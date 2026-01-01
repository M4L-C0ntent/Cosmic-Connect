// src/notifications.rs
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use zbus::{Connection, MatchRule, MessageStream};
use futures::StreamExt; // For .next() on MessageStream
use std::error::Error as StdError;

/// Notification handler for KDE Connect pairing requests
pub struct NotificationHandler {
    conn: Connection,
    known_pairing_states: Arc<Mutex<HashMap<String, bool>>>,
}

impl NotificationHandler {
    /// Create a new notification handler
    pub async fn new() -> Result<Self, Box<dyn StdError + Send + Sync>> {
        let conn = Connection::session().await?;
        
        // Start the notification interceptor in background
        Self::start_notification_interceptor(conn.clone());
        
        Ok(Self {
            conn,
            known_pairing_states: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    /// Intercept and suppress KDE Connect's pairing notifications
    fn start_notification_interceptor(conn: Connection) {
        tokio::spawn(async move {
            eprintln!("=== Starting Notification Interceptor ===");
            
            // Listen for Notify method calls from kdeconnectd
            let rule = match MatchRule::builder()
                .msg_type(zbus::message::Type::MethodCall)
                .interface("org.freedesktop.Notifications")
                .and_then(|builder| builder.member("Notify"))
                .map(|builder| builder.build())
            {
                Ok(rule) => rule,
                Err(e) => {
                    eprintln!("Failed to create match rule: {}", e);
                    return;
                }
            };
            
            if let Ok(mut stream) = MessageStream::for_match_rule(rule, &conn, None).await {
                while let Some(msg) = stream.next().await {
                    if let Ok(message) = msg {
                        // Check if this is a pairing notification from kdeconnectd
                        let body = message.body();
                        
                        // Notify signature: (susssasa{sv}i)
                        // app_name, replaces_id, app_icon, summary, body, actions, hints, expire_timeout
                        if let Ok((app_name, _replaces_id, _app_icon, summary, _body, _actions, _hints, _expire_timeout)) 
                            = body.deserialize::<(String, u32, String, String, String, Vec<String>, HashMap<String, zbus::zvariant::OwnedValue>, i32)>() 
                        {
                            // Check if this is a KDE Connect pairing notification
                            if (app_name.to_lowercase().contains("kdeconnect") || 
                                app_name.to_lowercase().contains("kde connect")) &&
                               (summary.to_lowercase().contains("pair") || 
                                summary.to_lowercase().contains("wants to connect"))
                            {
                                eprintln!("🚫 Intercepted KDE Connect pairing notification: '{}'", summary);
                                eprintln!("   This notification will be suppressed (our app handles pairing)");
                                
                                // We can't actually block the notification here without becoming a notification daemon
                                // But we can log it for debugging
                                // The real solution is below in the pairing signal handler
                            }
                        }
                    }
                }
            }
        });
    }

    /// Start listening for pairing state changes via D-Bus signals
    pub async fn listen_for_pairing_signals(
        &self,
        tx: tokio::sync::mpsc::Sender<PairingNotification>,
    ) -> Result<(), Box<dyn StdError + Send + Sync>> {
        eprintln!("=== Starting Pairing Signal Listener ===");
        
        // Create match rule for pairStateChanged signals
        let rule = MatchRule::builder()
            .msg_type(zbus::message::Type::Signal)
            .interface("org.kde.kdeconnect.device")?
            .member("pairStateChanged")?
            .build();

        let mut stream = MessageStream::for_match_rule(
            rule,
            &self.conn,
            None,
        ).await?;

        eprintln!("Signal listener ready, waiting for pairing requests...");

        while let Some(msg) = stream.next().await {
            if let Ok(message) = msg {
                // Get the object path from the message header
                if let Some(path) = message.header().path() {
                    let device_id = Self::extract_device_id_from_path(path.as_str());
                    
                    // Get the pair state from message body
                    let body = message.body();
                    if let Ok(pair_state) = body.deserialize::<i32>() {
                        
                        eprintln!("=== Pairing Signal Received ===");
                        eprintln!("Device ID: {}", device_id);
                        eprintln!("Pair State: {} (0=NotPaired, 1=Requested, 2=RequestedByPeer, 3=Paired)", pair_state);
                        
                        // Check if this is a new pairing request from peer
                        if pair_state == 2 {
                            // Get device info
                            match self.get_device_info(&device_id).await {
                                Ok((name, device_type)) => {
                                    eprintln!("Pairing request from: {} ({})", name, device_type);
                                    
                                    let notification = PairingNotification {
                                        device_id: device_id.clone(),
                                        device_name: name,
                                        device_type,
                                    };
                                    
                                    // Send notification event
                                    if let Err(e) = tx.send(notification).await {
                                        eprintln!("Failed to send pairing notification: {}", e);
                                    }
                                }
                                Err(e) => {
                                    eprintln!("Failed to get device info: {}", e);
                                }
                            }
                        } else if pair_state == 3 {
                            eprintln!("Device paired successfully");
                        } else if pair_state == 0 {
                            eprintln!("Device unpaired");
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Poll for pairing requests (fallback method if signals don't work)
    pub async fn poll_for_pairing_requests(
        &self,
        tx: tokio::sync::mpsc::Sender<PairingNotification>,
    ) -> Result<(), Box<dyn StdError + Send + Sync>> {
        eprintln!("=== Starting Pairing Polling (fallback mode) ===");
        
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
            
            // Get all devices
            match self.get_all_devices().await {
                Ok(device_ids) => {
                    for device_id in device_ids {
                        // Check if device is requesting pairing
                        if let Ok(is_requesting) = self.check_pairing_request(&device_id).await {
                            let mut states = self.known_pairing_states.lock().await;
                            let was_requesting = states.get(&device_id).copied().unwrap_or(false);
                            
                            // New pairing request detected
                            if is_requesting && !was_requesting {
                                eprintln!("=== New Pairing Request Detected (polling) ===");
                                eprintln!("Device ID: {}", device_id);
                                
                                match self.get_device_info(&device_id).await {
                                    Ok((name, device_type)) => {
                                        eprintln!("Device: {} ({})", name, device_type);
                                        
                                        let notification = PairingNotification {
                                            device_id: device_id.clone(),
                                            device_name: name,
                                            device_type,
                                        };
                                        
                                        if let Err(e) = tx.send(notification).await {
                                            eprintln!("Failed to send pairing notification: {}", e);
                                        }
                                    }
                                    Err(e) => {
                                        eprintln!("Failed to get device info: {}", e);
                                    }
                                }
                            }
                            
                            states.insert(device_id.clone(), is_requesting);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Failed to get devices: {}", e);
                }
            }
        }
    }

    /// Get all device IDs from KDE Connect daemon
    async fn get_all_devices(&self) -> Result<Vec<String>, Box<dyn StdError + Send + Sync>> {
        let proxy = zbus::Proxy::new(
            &self.conn,
            "org.kde.kdeconnect",
            "/modules/kdeconnect",
            "org.kde.kdeconnect.daemon",
        ).await?;

        let devices: Vec<String> = proxy
            .call("devices", &(false, false))
            .await?;

        Ok(devices)
    }

    /// Check if a device is requesting pairing
    async fn check_pairing_request(&self, device_id: &str) -> Result<bool, Box<dyn StdError + Send + Sync>> {
        let path = format!("/modules/kdeconnect/devices/{}", device_id);
        
        let proxy = zbus::Proxy::new(
            &self.conn,
            "org.kde.kdeconnect",
            path.as_str(),
            "org.freedesktop.DBus.Properties",
        ).await?;

        let result: zbus::zvariant::OwnedValue = proxy
            .call(
                "Get",
                &("org.kde.kdeconnect.device", "isPairRequestedByPeer"),
            )
            .await?;

        // downcast_ref returns Result<&bool, Error>, so just use the reference
        let is_requesting: bool = match result.downcast_ref::<bool>() {
            Ok(val) => val,  // val is already &bool, just use it
            Err(_) => return Err("Failed to downcast isPairRequestedByPeer to bool".into()),
        };

        Ok(is_requesting)
    }

    /// Get device name and type
    async fn get_device_info(&self, device_id: &str) -> Result<(String, String), Box<dyn StdError + Send + Sync>> {
        let path = format!("/modules/kdeconnect/devices/{}", device_id);
        
        let proxy = zbus::Proxy::new(
            &self.conn,
            "org.kde.kdeconnect",
            path.as_str(),
            "org.freedesktop.DBus.Properties",
        ).await?;

        // Get device name
        let name_result: zbus::zvariant::OwnedValue = proxy
            .call("Get", &("org.kde.kdeconnect.device", "name"))
            .await?;
        
        let name = name_result
            .downcast_ref::<String>()
            .map(|s| s.to_string())
            .unwrap_or_else(|_| "Unknown Device".to_string());

        // Get device type
        let type_result: zbus::zvariant::OwnedValue = proxy
            .call("Get", &("org.kde.kdeconnect.device", "type"))
            .await?;
        
        let device_type = type_result
            .downcast_ref::<String>()
            .map(|s| s.to_string())
            .unwrap_or_else(|_| "unknown".to_string());

        Ok((name, device_type))
    }

    /// Extract device ID from D-Bus object path
    fn extract_device_id_from_path(path: &str) -> String {
        // Path format: /modules/kdeconnect/devices/{device_id}
        path.split('/')
            .last()
            .unwrap_or("")
            .to_string()
    }
}

/// Pairing notification event
#[derive(Debug, Clone)]
pub struct PairingNotification {
    pub device_id: String,
    pub device_name: String,
    pub device_type: String,
}

/// Start the notification listener in the background
pub fn start_notification_listener(
    tx: tokio::sync::mpsc::Sender<PairingNotification>,
    use_polling: bool,
) {
    tokio::spawn(async move {
        eprintln!("=== Notification Listener Starting ===");
        eprintln!("Mode: {}", if use_polling { "Polling" } else { "D-Bus Signals" });
        
        match NotificationHandler::new().await {
            Ok(handler) => {
                if use_polling {
                    if let Err(e) = handler.poll_for_pairing_requests(tx).await {
                        eprintln!("Polling listener error: {}", e);
                    }
                } else {
                    // Try signals first, fall back to polling if it fails
                    match handler.listen_for_pairing_signals(tx.clone()).await {
                        Ok(_) => {
                            eprintln!("Signal listener stopped");
                        }
                        Err(e) => {
                            eprintln!("Signal listener failed: {}, falling back to polling", e);
                            if let Err(e) = handler.poll_for_pairing_requests(tx).await {
                                eprintln!("Polling listener error: {}", e);
                            }
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("Failed to create notification handler: {}", e);
            }
        }
    });
}

/// Show a desktop notification for pairing request
pub async fn show_pairing_notification(
    device_name: &str,
    _device_id: &str,
) -> Result<(), Box<dyn StdError + Send + Sync>> {
    eprintln!("=== Showing Notification ===");
    eprintln!("Device: {} ({})", device_name, _device_id);
    
    // Small delay to let KDE Connect's notification appear first
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    
    // Try to close any existing KDE Connect pairing notifications
    close_kdeconnect_notifications().await;
    
    let summary = format!("{} wants to pair", device_name);
    let body = "Click to open settings and accept or reject";
    
    // Create the kdeconnect URL for our app
    let url = format!("kdeconnect://pair/{}", _device_id);
    let device_id_clone = _device_id.to_string();
    
    // Spawn a background task to handle the notification with action
    tokio::spawn(async move {
        match send_notification_with_action(&summary, &body, &url, &device_id_clone).await {
            Ok(_) => eprintln!("Notification handled successfully"),
            Err(e) => eprintln!("Failed to handle notification: {}", e),
        }
    });
    
    Ok(())
}

/// Close any existing KDE Connect pairing notifications
async fn close_kdeconnect_notifications() {
    // Connect to notification daemon and close notifications from kdeconnectd
    if let Ok(conn) = Connection::session().await {
        if let Ok(_proxy) = zbus::Proxy::new(
            &conn,
            "org.freedesktop.Notifications",
            "/org/freedesktop/Notifications",
            "org.freedesktop.Notifications",
        ).await {
            // Get list of current notifications (if the daemon supports it)
            // Note: Not all notification daemons support GetServerInformation
            // This is best-effort
            eprintln!("Attempting to close existing KDE Connect notifications...");
            
            // We can't reliably close other app's notifications without their ID
            // But we've added a small delay so ours appears after KDE Connect's
        }
    }
}

/// Send notification and listen for action invocation
async fn send_notification_with_action(
    summary: &str,
    body: &str,
    url: &str,
    _device_id: &str,
) -> Result<(), Box<dyn StdError + Send + Sync>> {
    use std::collections::HashMap;
    
    let conn = Connection::session().await?;
    
    // Create the notification proxy
    let proxy = zbus::Proxy::new(
        &conn,
        "org.freedesktop.Notifications",
        "/org/freedesktop/Notifications",
        "org.freedesktop.Notifications",
    ).await?;
    
    // Prepare notification parameters
    let app_name = "COSMIC KDE Connect";
    let replaces_id: u32 = 0;
    let app_icon = "phone";
    
    // Actions: pairs of [action_key, action_label]
    // "default" is triggered when clicking the notification body
    // "open" is an explicit button
    let actions: Vec<&str> = vec!["default", "Open Settings", "open", "Open"];
    
    // Hints for urgency - use HashMap which zbus can serialize
    let mut hints: HashMap<&str, zbus::zvariant::Value> = HashMap::new();
    hints.insert("urgency", zbus::zvariant::Value::U8(2)); // Critical urgency
    hints.insert("category", zbus::zvariant::Value::Str("device.added".into())); // Category hint
    
    let expire_timeout: i32 = 0; // Don't auto-expire
    
    eprintln!("Sending notification via D-Bus...");
    eprintln!("  App: {}", app_name);
    eprintln!("  Summary: {}", summary);
    eprintln!("  Actions: {:?}", actions);
    
    // Send the notification
    let notification_id: u32 = proxy.call(
        "Notify",
        &(app_name, replaces_id, app_icon, summary, body, actions, hints, expire_timeout)
    ).await?;
    
    eprintln!("✓ Notification sent with ID: {}", notification_id);
    eprintln!("Listening for user interaction...");
    
    // Clone url for the listener
    let url = url.to_string();
    
    // Listen for ActionInvoked signal for this notification
    let rule = MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .interface("org.freedesktop.Notifications")?
        .member("ActionInvoked")?
        .build();
    
    let mut stream = MessageStream::for_match_rule(rule, &conn, None).await?;
    
    // Also listen for NotificationClosed to stop listening when dismissed
    let close_rule = MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .interface("org.freedesktop.Notifications")?
        .member("NotificationClosed")?
        .build();
    
    let mut close_stream = MessageStream::for_match_rule(close_rule, &conn, None).await?;
    
    // Wait for either action invocation or notification close
    loop {
        tokio::select! {
            Some(msg) = stream.next() => {
                if let Ok(message) = msg {
                    let body = message.body();
                    
                    // ActionInvoked has signature (u, s) - notification_id and action_key
                    if let Ok((id, action)) = body.deserialize::<(u32, String)>() {
                        eprintln!("📣 Action received - Notification ID: {}, Action: '{}'", id, action);
                        
                        if id == notification_id && (action == "default" || action == "open") {
                            eprintln!("✓ User clicked notification! Opening settings...");
                            eprintln!("  URL: {}", url);
                            
                            // Open settings app with the device ID via xdg-open
                            match tokio::process::Command::new("xdg-open")
                                .arg(&url)
                                .spawn()
                            {
                                Ok(_) => eprintln!("✓ Settings app launched successfully"),
                                Err(e) => {
                                    eprintln!("✗ Failed to launch via xdg-open: {}", e);
                                    eprintln!("  Trying direct launch...");
                                    // Fallback: try direct launch
                                    let _ = tokio::process::Command::new("cosmic-kdeconnect-settings")
                                        .arg(&url)
                                        .spawn();
                                }
                            }
                            
                            break;
                        }
                    }
                }
            }
            
            Some(msg) = close_stream.next() => {
                if let Ok(message) = msg {
                    let body = message.body();
                    
                    // NotificationClosed has signature (u, u) - notification_id and reason
                    if let Ok((id, reason)) = body.deserialize::<(u32, u32)>() {
                        if id == notification_id {
                            eprintln!("Notification {} closed with reason: {}", id, reason);
                            // Reason: 1=expired, 2=dismissed by user, 3=closed by app, 4=undefined
                            break;
                        }
                    }
                }
            }
        }
    }
    
    eprintln!("Notification action handler stopped");
    
    Ok(())
}