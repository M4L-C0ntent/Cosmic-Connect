// src/plugins/sms/dbus.rs
//! D-Bus operations for KDE Connect SMS functionality.

use futures::StreamExt;
use std::collections::HashMap;
use zbus::{Connection, MatchRule};

use super::models::{ContactsMap, Conversation, Message, SignalEvent};
use super::messages::SmsMessage;
use super::utils::{now_millis, parse_vcard};

const KDECONNECT_SERVICE: &str = "org.kde.kdeconnect";
const CONVERSATIONS_INTERFACE: &str = "org.kde.kdeconnect.device.conversations";
const DEVICE_INTERFACE: &str = "org.kde.kdeconnect.device";
const CONTACTS_INTERFACE: &str = "org.kde.kdeconnect.device.contacts";

/// Fetches all conversations from the device.
pub async fn fetch_conversations(device_id: String) -> Vec<Conversation> {
    let mut conversations = Vec::new();
    
    eprintln!("=== Fetching Conversations ===");
    
    let conn = match Connection::session().await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("✗ D-Bus failed: {:?}", e);
            return conversations;
        }
    };

    let path = format!("/modules/kdeconnect/devices/{}", device_id);
    
    // Request all conversation threads
    let _ = conn.call_method(
        Some(KDECONNECT_SERVICE),
        path.as_str(),
        Some(CONVERSATIONS_INTERFACE),
        "requestAllConversationThreads",
        &()
    ).await;
    
    tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
    
    // Get active conversations
    match conn.call_method(
        Some(KDECONNECT_SERVICE),
        path.as_str(),
        Some(CONVERSATIONS_INTERFACE),
        "activeConversations",
        &()
    ).await {
        Ok(reply) => {
            let body = reply.body();
            
            if let Ok(conv_variants) = body.deserialize::<Vec<zbus::zvariant::Value>>() {
                eprintln!("✓ Found {} conversations", conv_variants.len());
                
                for variant in conv_variants {
                    if let Some(conv) = parse_conversation_variant(&variant) {
                        conversations.push(conv);
                    }
                }
            }
        }
        Err(e) => {
            eprintln!("✗ Failed: {:?}", e);
        }
    }

    if conversations.is_empty() {
        conversations.push(Conversation {
            thread_id: "info_1".to_string(),
            contact_name: "📱 KDE Connect SMS".to_string(),
            phone_number: "System".to_string(),
            last_message: "No conversations found. Make sure SMS plugin is enabled!".to_string(),
            timestamp: now_millis(),
            unread: false,
        });
    }

    conversations
}

fn parse_conversation_variant(variant: &zbus::zvariant::Value) -> Option<Conversation> {
    let zbus::zvariant::Value::Structure(fields) = variant else {
        return None;
    };
    
    let fields_vec: Vec<zbus::zvariant::Value> = fields.fields().to_vec();
    
    let message_text = extract_string(&fields_vec, 1).unwrap_or_default();
    let phone_number = extract_phone_from_array(&fields_vec, 2).unwrap_or_else(|| "Unknown".to_string());
    let timestamp = extract_i64(&fields_vec, 3).unwrap_or_else(now_millis);
    let thread_id = extract_i64(&fields_vec, 6)
        .map(|id| id.to_string())
        .unwrap_or_else(|| "unknown".to_string());

    Some(Conversation {
        thread_id,
        contact_name: phone_number.clone(),
        phone_number,
        last_message: message_text,
        timestamp,
        unread: false,
    })
}

/// Requests messages for a specific conversation thread.
pub async fn request_conversation_messages(device_id: String, thread_id: String) {
    eprintln!("=== Requesting Messages ===");
    eprintln!("Thread: {}", thread_id);
    
    let conn = match Connection::session().await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("✗ D-Bus failed: {:?}", e);
            return;
        }
    };

    let thread_id_i64 = match thread_id.parse::<i64>() {
        Ok(id) => id,
        Err(_) => {
            eprintln!("✗ Invalid thread ID");
            return;
        }
    };

    let path = format!("/modules/kdeconnect/devices/{}", device_id);
    
    match conn.call_method(
        Some(KDECONNECT_SERVICE),
        path.as_str(),
        Some(CONVERSATIONS_INTERFACE),
        "requestConversation",
        &(thread_id_i64, 0i32, 50i32)
    ).await {
        Ok(_) => {
            eprintln!("✓ Requested messages for thread {}", thread_id_i64);
            eprintln!("  Messages will arrive via conversationUpdated signals");
        }
        Err(e) => {
            eprintln!("✗ Failed: {:?}", e);
        }
    }
}

/// Sends an SMS message to the specified phone number.
pub async fn send_sms(device_id: String, phone_number: String, message: String) {
    let mut log = format!(
        "=== Sending SMS ===\nDevice: {}\nTo: {}\nMessage: {}\n",
        device_id, phone_number, message
    );
    
    eprintln!("{}", log);
    let _ = std::fs::write("/tmp/sms_send.log", &log);
    
    // Try kdeconnect-cli first
    if send_via_cli(&device_id, &phone_number, &message, &mut log).await {
        return;
    }
    
    // Fallback to D-Bus
    send_via_dbus(&device_id, &phone_number, &message, &mut log).await;
}

async fn send_via_cli(device_id: &str, phone: &str, message: &str, log: &mut String) -> bool {
    log.push_str("Using kdeconnect-cli...\n");
    let _ = std::fs::write("/tmp/sms_send.log", log.as_str());
    
    match tokio::process::Command::new("kdeconnect-cli")
        .args(&["--device", device_id, "--send-sms", message, "--destination", phone])
        .output()
        .await
    {
        Ok(output) if output.status.success() => {
            log.push_str("✓ SMS sent successfully via kdeconnect-cli!\n");
            eprintln!("✓ SMS sent successfully via kdeconnect-cli!");
            let _ = std::fs::write("/tmp/sms_send.log", log.as_str());
            true
        }
        Ok(output) => {
            log.push_str(&format!(
                "✗ kdeconnect-cli failed: {}\n",
                String::from_utf8_lossy(&output.stderr)
            ));
            let _ = std::fs::write("/tmp/sms_send.log", log.as_str());
            false
        }
        Err(e) => {
            log.push_str(&format!("✗ kdeconnect-cli not available: {:?}\n", e));
            let _ = std::fs::write("/tmp/sms_send.log", log.as_str());
            false
        }
    }
}

async fn send_via_dbus(device_id: &str, phone: &str, message: &str, log: &mut String) {
    log.push_str("\nTrying D-Bus methods as fallback...\n");
    let _ = std::fs::write("/tmp/sms_send.log", log.as_str());
    
    let conn = match Connection::session().await {
        Ok(c) => c,
        Err(e) => {
            log.push_str(&format!("✗ D-Bus connection failed: {:?}\n", e));
            eprintln!("✗ D-Bus connection failed: {:?}", e);
            let _ = std::fs::write("/tmp/sms_send.log", log.as_str());
            return;
        }
    };

    log.push_str("✓ D-Bus connection established\n");
    let sms_path = format!("/modules/kdeconnect/devices/{}/sms", device_id);
    
    let addresses = vec![phone];
    let attachments: Vec<String> = vec![];
    let sub_ids: Vec<i64> = vec![];
    
    match conn.call_method(
        Some(KDECONNECT_SERVICE),
        sms_path.as_str(),
        Some(CONVERSATIONS_INTERFACE),
        "sendWithoutConversation",
        &(addresses, message, attachments, sub_ids)
    ).await {
        Ok(_) => {
            log.push_str("✓ SMS sent successfully via D-Bus!\n");
            eprintln!("✓ SMS sent successfully via D-Bus!");
        }
        Err(e) => {
            log.push_str(&format!("✗ D-Bus sendWithoutConversation failed: {:?}\n", e));
            log.push_str("\n✗ All SMS sending methods failed!\n");
        }
    }
    
    let _ = std::fs::write("/tmp/sms_send.log", log.as_str());
}

/// Fetches contacts from the device.
pub async fn fetch_contacts(device_id: String) -> ContactsMap {
    let mut contacts = HashMap::new();
    
    eprintln!("=== Fetching Contacts ===");
    eprintln!("Device ID: {}", device_id);
    
    let conn = match Connection::session().await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("✗ D-Bus connection failed: {:?}", e);
            return contacts;
        }
    };

    let device_path = format!("/modules/kdeconnect/devices/{}", device_id);
    
    // Check if contacts plugin is enabled
    if !check_contacts_plugin(&conn, &device_path).await {
        return contacts;
    }

    // Trigger synchronization
    trigger_contacts_sync(&conn, &device_id).await;

    // Read contacts from filesystem
    if let Ok(home) = std::env::var("HOME") {
        read_kpeople_contacts(&home, &device_id, &mut contacts);
        read_fallback_contacts(&home, &device_id, &mut contacts);
    }
    
    if contacts.is_empty() {
        eprintln!("\n=== No contacts found ===");
        eprintln!("To use contact names:");
        eprintln!("1. Enable the Contacts plugin in KDE Connect settings");
        eprintln!("2. Grant contacts permission on your phone");
        eprintln!("3. Wait for contacts to sync");
        eprintln!("4. Reopen this SMS window");
    }
    
    eprintln!("Returning {} contacts", contacts.len());
    contacts
}

async fn check_contacts_plugin(conn: &Connection, device_path: &str) -> bool {
    eprintln!("Checking if contacts plugin is enabled...");
    
    match conn.call_method(
        Some(KDECONNECT_SERVICE),
        device_path,
        Some(DEVICE_INTERFACE),
        "hasPlugin",
        &("kdeconnect_contacts",)
    ).await {
        Ok(reply) => {
            let has_plugin = reply.body().deserialize::<bool>().unwrap_or(false);
            eprintln!("Contacts plugin enabled: {}", has_plugin);
            if !has_plugin {
                eprintln!("✗ Contacts plugin is not enabled on this device!");
                eprintln!("  Enable it in KDE Connect settings to use contact names");
            }
            has_plugin
        }
        Err(e) => {
            eprintln!("✗ Failed to check plugin: {:?}", e);
            false
        }
    }
}

async fn trigger_contacts_sync(conn: &Connection, device_id: &str) {
    let path = format!("/modules/kdeconnect/devices/{}/contacts", device_id);
    
    eprintln!("Calling synchronizeRemoteWithLocal...");
    match conn.call_method(
        Some(KDECONNECT_SERVICE),
        path.as_str(),
        Some(CONTACTS_INTERFACE),
        "synchronizeRemoteWithLocal",
        &()
    ).await {
        Ok(_) => {
            eprintln!("✓ Sync triggered, waiting for contacts...");
            tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
        }
        Err(e) => {
            eprintln!("✗ Failed to trigger sync: {:?}", e);
        }
    }
}

fn read_kpeople_contacts(home: &str, device_id: &str, contacts: &mut ContactsMap) {
    let kpeople_base = format!("{}/.local/share/kpeoplevcard", home);
    eprintln!("Checking KPeople VCard directory: {}", kpeople_base);
    
    let Ok(entries) = std::fs::read_dir(&kpeople_base) else {
        eprintln!("✗ KPeople VCard directory not found: {}", kpeople_base);
        eprintln!("  Contacts may not be synced yet");
        return;
    };
    
    eprintln!("✓ KPeople VCard directory exists, listing subdirectories...");
    
    for entry in entries.flatten() {
        let entry_path = entry.path();
        if !entry_path.is_dir() {
            continue;
        }
        
        let dir_name = entry_path.file_name().unwrap().to_string_lossy();
        eprintln!("  Found directory: {}", dir_name);
        
        if !dir_name.contains(device_id) {
            continue;
        }
        
        eprintln!("  ✓ This directory matches our device ID!");
        read_vcards_from_dir(&entry_path, contacts);
    }
}

fn read_vcards_from_dir(dir_path: &std::path::Path, contacts: &mut ContactsMap) {
    let Ok(vcf_entries) = std::fs::read_dir(dir_path) else {
        return;
    };
    
    for vcf_entry in vcf_entries.flatten() {
        let vcf_path = vcf_entry.path();
        let ext = vcf_path.extension().and_then(|s| s.to_str());
        
        if ext != Some("vcf") && ext != Some("vcard") {
            continue;
        }
        
        eprintln!("    Reading VCard: {}", vcf_path.display());
        
        if let Ok(content) = std::fs::read_to_string(&vcf_path) {
            if let (Some(name), phones) = parse_vcard(&content) {
                eprintln!("      Name: {}", name);
                for phone in phones {
                    eprintln!("      Phone: {}", phone);
                    contacts.insert(phone, name.clone());
                }
            }
        }
    }
}

fn read_fallback_contacts(home: &str, device_id: &str, contacts: &mut ContactsMap) {
    let old_cache_path = format!("{}/.local/share/kdeconnect/{}/contacts", home, device_id);
    eprintln!("Checking fallback location: {}", old_cache_path);
    
    if !std::path::Path::new(&old_cache_path).exists() {
        return;
    }
    
    eprintln!("✓ Found old cache location, reading...");
    let Ok(content) = std::fs::read_to_string(&old_cache_path) else {
        return;
    };
    
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) else {
        return;
    };
    
    let Some(obj) = json.as_object() else {
        return;
    };
    
    for (_id, contact_data) in obj.iter() {
        let Some(contact_obj) = contact_data.as_object() else {
            continue;
        };
        
        let name = contact_obj.get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        
        if let Some(phone_array) = contact_obj.get("phoneNumber").and_then(|v| v.as_array()) {
            for phone_entry in phone_array {
                if let Some(phone_obj) = phone_entry.as_object() {
                    if let Some(number) = phone_obj.get("number").and_then(|v| v.as_str()) {
                        contacts.insert(number.to_string(), name.to_string());
                    }
                }
            }
        }
    }
}

/// Creates a stream that listens for SMS signal events from D-Bus.
pub fn listen_for_sms_signals_stream(device_id: String) -> impl futures::Stream<Item = SmsMessage> {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    
    tokio::spawn(async move {
        eprintln!("🔊 Starting persistent signal listener for device: {}", device_id);
        
        let conn = match Connection::session().await {
            Ok(c) => c,
            Err(e) => {
                eprintln!("✗ Failed to connect to D-Bus: {:?}", e);
                return;
            }
        };

        let conversations_rule = match MatchRule::builder()
            .msg_type(zbus::message::Type::Signal)
            .sender(KDECONNECT_SERVICE)
            .and_then(|b| b.interface(CONVERSATIONS_INTERFACE))
        {
            Ok(builder) => builder.build(),
            Err(e) => {
                eprintln!("✗ Failed to build match rule: {:?}", e);
                return;
            }
        };
        
        let mut stream = match zbus::MessageStream::for_match_rule(conversations_rule, &conn, None).await {
            Ok(s) => s,
            Err(e) => {
                eprintln!("✗ Failed to create signal stream: {:?}", e);
                return;
            }
        };
        
        eprintln!("✓ Signal stream created, listening for all signals...");
        
        while let Some(msg_result) = stream.next().await {
            let Ok(msg) = msg_result else {
                continue;
            };
            
            let header = msg.header();
            let path = header.path().map(|p| p.as_str()).unwrap_or("");
            
            if !path.contains(&device_id) {
                continue;
            }
            
            let member = header.member().map(|m| m.as_str()).unwrap_or("unknown");
            
            if member == "conversationUpdated" {
                if let Some(message) = parse_conversation_updated_signal(&msg) {
                    eprintln!("📨 Message #{}: type={}, {}", 
                        message.thread_id, 
                        message.type_, 
                        message.body.chars().take(40).collect::<String>()
                    );
                    
                    if tx.send(SmsMessage::SignalReceived(SignalEvent::MessageReceived(message))).is_err() {
                        eprintln!("⚠️  Receiver dropped, stopping signal listener");
                        break;
                    }
                }
            }
        }
        
        eprintln!("Signal listener task ended");
    });
    
    tokio_stream::wrappers::UnboundedReceiverStream::new(rx)
}

fn parse_conversation_updated_signal(msg: &zbus::Message) -> Option<Message> {
    let body = msg.body();
    let zbus::zvariant::Value::Structure(fields) = body.deserialize::<zbus::zvariant::Value>().ok()? else {
        return None;
    };
    
    let fields_vec: Vec<zbus::zvariant::Value> = fields.fields().to_vec();
    
    let message_type = extract_i32(&fields_vec, 4).unwrap_or(1);
    let message_body = extract_string(&fields_vec, 1).unwrap_or_default();
    let phone_number = extract_phone_from_array(&fields_vec, 2).unwrap_or_else(|| "Unknown".to_string());
    let timestamp = extract_i64(&fields_vec, 3).unwrap_or_else(now_millis);
    let thread_id = extract_i64(&fields_vec, 6)
        .map(|id| id.to_string())
        .unwrap_or_else(|| "unknown".to_string());
    
    let message_id = format!("{}_{}", thread_id, timestamp);
    
    Some(Message {
        id: message_id,
        thread_id,
        body: message_body,
        address: phone_number,
        date: timestamp,
        type_: message_type,
        read: true,
    })
}

// Helper functions for extracting values from D-Bus variants

fn extract_string(fields: &[zbus::zvariant::Value], index: usize) -> Option<String> {
    match fields.get(index) {
        Some(zbus::zvariant::Value::Str(s)) => Some(s.to_string()),
        _ => None,
    }
}

fn extract_i32(fields: &[zbus::zvariant::Value], index: usize) -> Option<i32> {
    match fields.get(index) {
        Some(zbus::zvariant::Value::I32(n)) => Some(*n),
        _ => None,
    }
}

fn extract_i64(fields: &[zbus::zvariant::Value], index: usize) -> Option<i64> {
    match fields.get(index) {
        Some(zbus::zvariant::Value::I64(n)) => Some(*n),
        _ => None,
    }
}

fn extract_phone_from_array(fields: &[zbus::zvariant::Value], index: usize) -> Option<String> {
    let zbus::zvariant::Value::Array(arr) = fields.get(index)? else {
        return None;
    };
    
    let zbus::zvariant::Value::Structure(phone_struct) = arr.iter().next()? else {
        return None;
    };
    
    let phone_fields: Vec<_> = phone_struct.fields().to_vec();
    extract_string(&phone_fields, 0)
}
