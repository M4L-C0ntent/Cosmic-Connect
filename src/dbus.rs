// src/dbus.rs
use crate::models::Device;
use std::sync::Arc;
use tokio::sync::Mutex;
use zbus::Connection;

/// Media player information from the phone
#[derive(Debug, Clone, Default)]
pub struct MediaPlayerInfo {
    pub player: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub is_playing: bool,
    pub length: i64,
    pub position: i64,
    pub volume: i32,
    pub can_pause: bool,
    pub can_play: bool,
    pub can_go_next: bool,
    pub can_go_previous: bool,
    pub can_seek: bool,
}

// NEW: Connection pool for reuse and cleanup
lazy_static::lazy_static! {
    static ref CONNECTION_POOL: Arc<Mutex<Option<Connection>>> = Arc::new(Mutex::new(None));
}

// NEW: Get or create shared connection
async fn get_connection() -> Result<Connection, Box<dyn std::error::Error + Send + Sync>> {
    let mut pool = CONNECTION_POOL.lock().await;
    
    if let Some(conn) = pool.as_ref() {
        // Just return the existing connection - zbus 5.1 doesn't have is_closed()
        return Ok(conn.clone());
    }
    
    let conn = Connection::session().await?;
    *pool = Some(conn.clone());
    Ok(conn)
}

// NEW: Cleanup function for shutdown
pub async fn cleanup() {
    let mut pool = CONNECTION_POOL.lock().await;
    if let Some(conn) = pool.take() {
        drop(conn);
        eprintln!("D-Bus connection pool closed");
    }
}

pub async fn fetch_devices() -> Vec<Device> {
    let mut devices = Vec::new();

    // CHANGED: Use connection pool instead of creating new connection
    match get_connection().await {
        Ok(conn) => {
            let daemon_path = "/modules/kdeconnect";
            let daemon_interface = "org.kde.kdeconnect.daemon";
            
            let result = conn.call_method(
                Some("org.kde.kdeconnect"),
                daemon_path,
                Some(daemon_interface),
                "devices",
                &(false, false)
            ).await;
            
            match result {
                Ok(reply) => {
                    let body = reply.body();
                    match body.deserialize::<Vec<String>>() {
                        Ok(device_ids) => {
                            for device_id in device_ids {
                                let path = format!("/modules/kdeconnect/devices/{}", device_id);
                                
                                let name = get_device_property(&conn, &path, "name").await.unwrap_or_else(|_| "Unknown".to_string());
                                let is_reachable = get_device_property_bool(&conn, &path, "isReachable").await.unwrap_or(false);
                                let is_paired = get_device_property_bool(&conn, &path, "isPaired").await.unwrap_or(false);
                                let device_type = get_device_property(&conn, &path, "type").await.unwrap_or_else(|_| "phone".to_string());
                                
                                let has_battery = check_plugin(&conn, &path, "kdeconnect_battery").await;
                                let has_ping = check_plugin(&conn, &path, "kdeconnect_ping").await;
                                let has_share = check_plugin(&conn, &path, "kdeconnect_share").await;
                                let has_findmyphone = check_plugin(&conn, &path, "kdeconnect_findmyphone").await;
                                let has_sms = check_plugin(&conn, &path, "kdeconnect_sms").await;
                                let has_clipboard = check_plugin(&conn, &path, "kdeconnect_clipboard").await;
                                let has_contacts = check_plugin(&conn, &path, "kdeconnect_contacts").await;
                                let has_mpris = check_plugin(&conn, &path, "kdeconnect_mprisremote").await;
                                let has_remote_keyboard = check_plugin(&conn, &path, "kdeconnect_remotekeyboard").await;
                                let has_sftp = check_plugin(&conn, &path, "kdeconnect_sftp").await;
                                let has_presenter = check_plugin(&conn, &path, "kdeconnect_presenter").await;
                                let has_lockdevice = check_plugin(&conn, &path, "kdeconnect_lockdevice").await;
                                let has_virtualmonitor = check_plugin(&conn, &path, "kdeconnect_virtualmonitor").await;
                                
                                let (battery_level, is_charging) = if has_battery {
                                    let battery_path = format!("/modules/kdeconnect/devices/{}/battery", device_id);
                                    let level = get_plugin_property_int(&conn, &battery_path, "org.kde.kdeconnect.device.battery", "charge").await.ok();
                                    let charging = get_plugin_property_bool(&conn, &battery_path, "org.kde.kdeconnect.device.battery", "isCharging").await.ok();
                                    (level, charging)
                                } else {
                                    (None, None)
                                };

                                // Fetch connectivity/signal strength information
                                let has_connectivity = check_plugin(&conn, &path, "kdeconnect_connectivity_report").await;
                                let (signal_strength, network_type) = if has_connectivity {
                                    let connectivity_path = format!("/modules/kdeconnect/devices/{}/connectivity_report", device_id);
                                    let strength = get_plugin_property_int(&conn, &connectivity_path, "org.kde.kdeconnect.device.connectivity_report", "cellularNetworkStrength").await.ok();
                                    let net_type = get_plugin_property_string(&conn, &connectivity_path, "org.kde.kdeconnect.device.connectivity_report", "cellularNetworkType").await.ok();
                                    (strength, net_type)
                                } else {
                                    (None, None)
                                };

                                let pairing_requests = get_device_property_int(&conn, &path, "pairingRequestsCount").await.unwrap_or(0);

                                devices.push(Device {
                                    id: device_id,
                                    name,
                                    device_type,
                                    is_reachable,
                                    is_paired,
                                    battery_level,
                                    is_charging,
                                    has_battery,
                                    has_ping,
                                    has_share,
                                    has_findmyphone,
                                    has_sms,
                                    has_clipboard,
                                    has_contacts,
                                    has_mpris,
                                    has_remote_keyboard,
                                    has_sftp,
                                    has_presenter,
                                    has_lockdevice,
                                    has_virtualmonitor,
                                    pairing_requests,
                                    signal_strength,
                                    network_type,
                                    available_players: Vec::new(),
                                    current_player: None,
                                    media_info: None,
                                });
                            }
                        }
                        Err(_) => {}
                    }
                }
                Err(_) => {}
            }
        }
        Err(_) => {}
    }

    devices
}

async fn get_device_property(conn: &Connection, path: &str, property: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let result = conn.call_method(
        Some("org.kde.kdeconnect"),
        path,
        Some("org.freedesktop.DBus.Properties"),
        "Get",
        &("org.kde.kdeconnect.device", property)
    ).await?;
    
    let body = result.body();
    let value: zbus::zvariant::Value = body.deserialize()?;
    
    if let zbus::zvariant::Value::Str(s) = value {
        Ok(s.to_string())
    } else {
        Err("Not a string".into())
    }
}

async fn get_device_property_bool(conn: &Connection, path: &str, property: &str) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    let result = conn.call_method(
        Some("org.kde.kdeconnect"),
        path,
        Some("org.freedesktop.DBus.Properties"),
        "Get",
        &("org.kde.kdeconnect.device", property)
    ).await?;
    
    let body = result.body();
    let value: zbus::zvariant::Value = body.deserialize()?;
    
    if let zbus::zvariant::Value::Bool(b) = value {
        Ok(b)
    } else {
        Err("Not a bool".into())
    }
}

async fn get_device_property_int(conn: &Connection, path: &str, property: &str) -> Result<i32, Box<dyn std::error::Error + Send + Sync>> {
    let result = conn.call_method(
        Some("org.kde.kdeconnect"),
        path,
        Some("org.freedesktop.DBus.Properties"),
        "Get",
        &("org.kde.kdeconnect.device", property)
    ).await?;
    
    let body = result.body();
    let value: zbus::zvariant::Value = body.deserialize()?;
    
    if let zbus::zvariant::Value::I32(i) = value {
        Ok(i)
    } else {
        Err("Not an int".into())
    }
}

async fn check_plugin(conn: &Connection, path: &str, plugin: &str) -> bool {
    let result = conn.call_method(
        Some("org.kde.kdeconnect"),
        path,
        Some("org.kde.kdeconnect.device"),
        "hasPlugin",
        &(plugin,)
    ).await;
    
    if let Ok(reply) = result {
        let body = reply.body();
        body.deserialize::<bool>().unwrap_or(false)
    } else {
        false
    }
}

// Device Actions
pub async fn ping_device(device_id: String) {
    eprintln!("=== Sending Ping ===");
    eprintln!("Device: {}", device_id);
    
    // CHANGED: Use connection pool
    if let Ok(conn) = get_connection().await {
        let path = format!("/modules/kdeconnect/devices/{}/ping", device_id);
        
        eprintln!("Path: {}", path);
        eprintln!("Interface: org.kde.kdeconnect.device.ping");
        
        match conn.call_method(
            Some("org.kde.kdeconnect"),
            path.as_str(),
            Some("org.kde.kdeconnect.device.ping"),
            "sendPing",
            &()
        ).await {
            Ok(_) => eprintln!("Ã¢Å“â€œ Ping sent successfully"),
            Err(e) => eprintln!("Ã¢Å“â€” Failed to send ping: {:?}", e),
        }
    }
}

pub async fn pair_device(device_id: String) {
    eprintln!("=== Pairing Device ===");
    eprintln!("Device: {}", device_id);
    
    if let Ok(conn) = get_connection().await {
        let path = format!("/modules/kdeconnect/devices/{}", device_id);
        
        match conn.call_method(
            Some("org.kde.kdeconnect"),
            path.as_str(),
            Some("org.kde.kdeconnect.device"),
            "requestPairing",
            &()
        ).await {
            Ok(_) => eprintln!("Ã¢Å“â€œ Pairing request sent"),
            Err(e) => eprintln!("Ã¢Å“â€” Failed to request pairing: {:?}", e),
        }
    }
}

pub async fn unpair_device(device_id: String) {
    eprintln!("=== Unpairing Device ===");
    eprintln!("Device: {}", device_id);
    
    if let Ok(conn) = get_connection().await {
        let path = format!("/modules/kdeconnect/devices/{}", device_id);
        
        match conn.call_method(
            Some("org.kde.kdeconnect"),
            path.as_str(),
            Some("org.kde.kdeconnect.device"),
            "unpair",
            &()
        ).await {
            Ok(_) => eprintln!("Ã¢Å“â€œ Device unpaired"),
            Err(e) => eprintln!("Ã¢Å“â€” Failed to unpair: {:?}", e),
        }
    }
}

pub async fn accept_pairing(device_id: String) {
    eprintln!("=== Accepting Pairing ===");
    eprintln!("Device: {}", device_id);
    
    if let Ok(conn) = get_connection().await {
        let path = format!("/modules/kdeconnect/devices/{}", device_id);
        
        match conn.call_method(
            Some("org.kde.kdeconnect"),
            path.as_str(),
            Some("org.kde.kdeconnect.device"),
            "acceptPairing",
            &()
        ).await {
            Ok(_) => eprintln!("Ã¢Å“â€œ Pairing accepted"),
            Err(e) => eprintln!("Ã¢Å“â€” Failed to accept pairing: {:?}", e),
        }
    }
}

pub async fn reject_pairing(device_id: String) {
    eprintln!("=== Rejecting Pairing ===");
    eprintln!("Device: {}", device_id);
    
    if let Ok(conn) = get_connection().await {
        let path = format!("/modules/kdeconnect/devices/{}", device_id);
        
        match conn.call_method(
            Some("org.kde.kdeconnect"),
            path.as_str(),
            Some("org.kde.kdeconnect.device"),
            "rejectPairing",
            &()
        ).await {
            Ok(_) => eprintln!("Ã¢Å“â€œ Pairing rejected"),
            Err(e) => eprintln!("Ã¢Å“â€” Failed to reject pairing: {:?}", e),
        }
    }
}

pub async fn find_my_phone(device_id: String) {
    eprintln!("=== Finding Phone ===");
    eprintln!("Device: {}", device_id);
    
    if let Ok(conn) = get_connection().await {
        let path = format!("/modules/kdeconnect/devices/{}/findmyphone", device_id);
        
        match conn.call_method(
            Some("org.kde.kdeconnect"),
            path.as_str(),
            Some("org.kde.kdeconnect.device.findmyphone"),
            "ring",
            &()
        ).await {
            Ok(_) => eprintln!("Ã¢Å“â€œ Phone is ringing"),
            Err(e) => eprintln!("Ã¢Å“â€” Failed to ring phone: {:?}", e),
        }
    }
}

// Alias for compatibility with main.rs
pub async fn ring_device(device_id: String) {
    find_my_phone(device_id).await;
}

pub async fn share_file(device_id: String, file_path: String) {
    eprintln!("=== Sharing File ===");
    eprintln!("Device: {}", device_id);
    eprintln!("File: {}", file_path);
    
    if let Ok(conn) = get_connection().await {
        let path = format!("/modules/kdeconnect/devices/{}/share", device_id);
        
        // Convert file path to file:// URL
        let file_url = if file_path.starts_with("file://") {
            file_path
        } else {
            format!("file://{}", file_path)
        };
        
        match conn.call_method(
            Some("org.kde.kdeconnect"),
            path.as_str(),
            Some("org.kde.kdeconnect.device.share"),
            "shareUrl",
            &(file_url,)
        ).await {
            Ok(_) => eprintln!("Ã¢Å“â€œ File shared successfully"),
            Err(e) => eprintln!("Ã¢Å“â€” Failed to share file: {:?}", e),
        }
    }
}

// Share multiple files
pub async fn share_files(device_id: String, file_paths: Vec<String>) {
    for file_path in file_paths {
        share_file(device_id.clone(), file_path).await;
    }
}

#[allow(dead_code)]
// Enhanced browse_files function with comprehensive debugging
// Replace the existing browse_files function in dbus.rs with this version

#[allow(dead_code)]
// src/dbus.rs - browse_files function
// Final version that handles stale SSHFS mounts

#[allow(dead_code)]
// src/dbus.rs - browse_files function
// Corrected version that opens file manager directly (not settings)

#[allow(dead_code)]
pub async fn browse_files(device_id: String) {
    eprintln!("\n╔════════════════════════════════════════════════════════════════╗");
    eprintln!("║              BROWSING DEVICE FILES                             ║");
    eprintln!("╚════════════════════════════════════════════════════════════════╝");
    eprintln!("Device ID: {}", device_id);
    
    if let Ok(conn) = get_connection().await {
        let path = format!("/modules/kdeconnect/devices/{}/sftp", device_id);
        
        // === STEP 1: Check if already mounted ===
        eprintln!("\n[1] Checking mount status...");
        let is_mounted = match conn.call_method(
            Some("org.kde.kdeconnect"),
            path.as_str(),
            Some("org.kde.kdeconnect.device.sftp"),
            "isMounted",
            &()
        ).await {
            Ok(reply) => {
                let mounted = reply.body().deserialize::<bool>().unwrap_or(false);
                eprintln!("  → isMounted: {}", mounted);
                mounted
            }
            Err(e) => {
                eprintln!("  ✗ Failed to check mount status: {:?}", e);
                false
            }
        };
        
        // === STEP 2: If mounted, check if it's stale ===
        if is_mounted {
            eprintln!("\n[2] Device reports as mounted, checking if accessible...");
            
            // Get mount point
            if let Ok(reply) = conn.call_method(
                Some("org.kde.kdeconnect"),
                path.as_str(),
                Some("org.kde.kdeconnect.device.sftp"),
                "mountPoint",
                &()
            ).await {
                if let Ok(mount_point) = reply.body().deserialize::<String>() {
                    if !mount_point.is_empty() {
                        eprintln!("  → Mount point: {}", mount_point);
                        
                        // Check if mount is accessible
                        if std::path::Path::new(&mount_point).exists() {
                            match std::fs::read_dir(&mount_point) {
                                Ok(entries) => {
                                    let count = entries.count();
                                    eprintln!("  ✓ Mount is accessible ({} entries)", count);
                                    eprintln!("\n[3] Opening file manager...");
                                    open_in_file_manager(&mount_point).await;
                                    eprintln!("\n✓ Done!");
                                    return;
                                }
                                Err(e) => {
                                    eprintln!("  ✗ Mount exists but not accessible: {:?}", e);
                                    eprintln!("  → This is a STALE MOUNT, needs to be fixed");
                                    
                                    // Unmount the stale mount
                                    eprintln!("\n[3] Unmounting stale mount...");
                                    let _ = conn.call_method(
                                        Some("org.kde.kdeconnect"),
                                        path.as_str(),
                                        Some("org.kde.kdeconnect.device.sftp"),
                                        "unmount",
                                        &()
                                    ).await;
                                    
                                    // Force unmount with fusermount
                                    eprintln!("  → Force unmounting with fusermount...");
                                    let _ = tokio::process::Command::new("fusermount")
                                        .args(&["-u", &mount_point])
                                        .output()
                                        .await;
                                    
                                    // Also try fusermount3
                                    let _ = tokio::process::Command::new("fusermount3")
                                        .args(&["-u", &mount_point])
                                        .output()
                                        .await;
                                    
                                    eprintln!("  → Waiting 2 seconds for unmount...");
                                    tokio::time::sleep(tokio::time::Duration::from_millis(2000)).await;
                                    
                                    // Fall through to remount below
                                }
                            }
                        } else {
                            eprintln!("  ✗ Mount point doesn't exist: {}", mount_point);
                            // Fall through to mount
                        }
                    }
                }
            }
        }
        
        // === STEP 3: Mount the device ===
        eprintln!("\n[3] Mounting device...");
        match conn.call_method(
            Some("org.kde.kdeconnect"),
            path.as_str(),
            Some("org.kde.kdeconnect.device.sftp"),
            "mount",
            &()
        ).await {
            Ok(_) => {
                eprintln!("  ✓ Mount command sent");
                eprintln!("  → Waiting 4 seconds for SSHFS to establish connection...");
                tokio::time::sleep(tokio::time::Duration::from_millis(4000)).await;
            }
            Err(e) => {
                eprintln!("  ✗ Mount command failed: {:?}", e);
                eprintln!("\n✗ Cannot mount device. Please check:");
                eprintln!("   - Phone is connected to same network");
                eprintln!("   - KDE Connect is running on phone");
                eprintln!("   - SFTP plugin is enabled in phone settings");
                return;
            }
        }
        
        // === STEP 4: Verify mount succeeded ===
        eprintln!("\n[4] Verifying mount...");
        let is_now_mounted = match conn.call_method(
            Some("org.kde.kdeconnect"),
            path.as_str(),
            Some("org.kde.kdeconnect.device.sftp"),
            "isMounted",
            &()
        ).await {
            Ok(reply) => reply.body().deserialize::<bool>().unwrap_or(false),
            Err(_) => false
        };
        
        if !is_now_mounted {
            eprintln!("  ✗ Mount verification failed");
            eprintln!("\n✗ Mount did not succeed. This could mean:");
            eprintln!("   - SSHFS is not installed on your PC");
            eprintln!("   - Network connectivity issue");
            eprintln!("   - Phone rejected the connection");
            return;
        }
        
        eprintln!("  ✓ Mount verified");
        
        // === STEP 5: Get mount point and verify accessibility ===
        eprintln!("\n[5] Getting mount point...");
        match conn.call_method(
            Some("org.kde.kdeconnect"),
            path.as_str(),
            Some("org.kde.kdeconnect.device.sftp"),
            "mountPoint",
            &()
        ).await {
            Ok(reply) => {
                if let Ok(mount_point) = reply.body().deserialize::<String>() {
                    if mount_point.is_empty() {
                        eprintln!("  ✗ Mount point is empty");
                        return;
                    }
                    
                    eprintln!("  → Mount point: {}", mount_point);
                    
                    // === STEP 6: Test accessibility with retries ===
                    eprintln!("\n[6] Testing mount accessibility...");
                    
                    let mut accessible = false;
                    
                    for attempt in 1..=5 {
                        eprintln!("  → Attempt {}/5...", attempt);
                        
                        // First check if path exists
                        if !std::path::Path::new(&mount_point).exists() {
                            eprintln!("    ✗ Mount point doesn't exist yet");
                            if attempt < 5 {
                                eprintln!("    → Waiting 1 second...");
                                tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
                            }
                            continue;
                        }
                        
                        // Try to read directory
                        match std::fs::read_dir(&mount_point) {
                            Ok(entries) => {
                                let entry_count = entries.count();
                                eprintln!("    ✓ Accessible ({} entries)", entry_count);
                                accessible = true;
                                
                                // Show first few files
                                if let Ok(entries) = std::fs::read_dir(&mount_point) {
                                    eprintln!("    → Contents:");
                                    for (i, entry) in entries.enumerate().take(5) {
                                        if let Ok(entry) = entry {
                                            eprintln!("      {}. {}", i + 1, entry.file_name().to_string_lossy());
                                        }
                                    }
                                    if entry_count > 5 {
                                        eprintln!("      ... and {} more", entry_count - 5);
                                    }
                                }
                                break;
                            }
                            Err(e) => {
                                eprintln!("    ✗ Not accessible: {} (kind: {:?})", e, e.kind());
                                
                                if attempt == 5 {
                                    // Last attempt - show diagnostics
                                    eprintln!("\n    [Diagnostics]");
                                    
                                    // Check metadata
                                    if let Ok(metadata) = std::fs::metadata(&mount_point) {
                                        #[cfg(unix)]
                                        {
                                            use std::os::unix::fs::PermissionsExt;
                                            let mode = metadata.permissions().mode();
                                            eprintln!("    → Permissions: {:o}", mode);
                                        }
                                    }
                                    
                                    // Check if mount is in mount table
                                    if let Ok(output) = tokio::process::Command::new("mount")
                                        .output()
                                        .await
                                    {
                                        let mount_output = String::from_utf8_lossy(&output.stdout);
                                        if let Some(line) = mount_output.lines().find(|l| l.contains(&device_id)) {
                                            eprintln!("    → System mount: {}", line);
                                        }
                                    }
                                } else {
                                    eprintln!("    → Waiting 1 second before retry...");
                                    tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
                                }
                            }
                        }
                    }
                    
                    if accessible {
                        eprintln!("\n[7] Opening file manager...");
                        eprintln!("  → Path: {}", mount_point);
                        open_in_file_manager(&mount_point).await;
                        eprintln!("\n✓ Done! File manager should now be open.");
                    } else {
                        eprintln!("\n✗ Mount exists but is not accessible after 5 attempts");
                        eprintln!("\nPossible issues:");
                        eprintln!("  1. SSHFS connection is broken (stale mount)");
                        eprintln!("  2. Network connectivity problem");
                        eprintln!("  3. Phone's SSH server not responding");
                        eprintln!("\nTry:");
                        eprintln!("  - Running: ./fix_stale_mount.sh");
                        eprintln!("  - Restarting KDE Connect on phone");
                        eprintln!("  - Checking phone is on same network");
                        eprintln!("  - Running: systemctl --user restart kdeconnect.service");
                    }
                } else {
                    eprintln!("  ✗ Failed to deserialize mount point");
                }
            }
            Err(e) => {
                eprintln!("  ✗ Failed to get mount point: {:?}", e);
            }
        }
    } else {
        eprintln!("✗ Failed to get D-Bus connection");
    }
}

/// Try using qdbus command-line tool to mount and browse
#[allow(dead_code)]
async fn try_qdbus_mount(device_id: &str) {
    eprintln!("=== Trying qdbus mount ===");
    
    // Try qdbus (Qt5) first, then qdbus-qt5 variant
    let mount_result = match tokio::process::Command::new("qdbus")
        .args(&[
            "org.kde.kdeconnect",
            &format!("/modules/kdeconnect/devices/{}/sftp", device_id),
            "org.kde.kdeconnect.device.sftp.mount"
        ])
        .output()
        .await
    {
        Ok(output) => Ok(output),
        Err(_) => {
            // Try qdbus-qt5 variant
            tokio::process::Command::new("qdbus-qt5")
                .args(&[
                    "org.kde.kdeconnect",
                    &format!("/modules/kdeconnect/devices/{}/sftp", device_id),
                    "org.kde.kdeconnect.device.sftp.mount"
                ])
                .output()
                .await
        }
    };
    
    match mount_result {
        Ok(output) if output.status.success() => {
            eprintln!("Ã¢Å“â€œ qdbus mount succeeded");
            tokio::time::sleep(tokio::time::Duration::from_millis(2000)).await;
            
            // Get mount point via qdbus
            let mountpoint_result = match tokio::process::Command::new("qdbus")
                .args(&[
                    "org.kde.kdeconnect",
                    &format!("/modules/kdeconnect/devices/{}/sftp", device_id),
                    "org.kde.kdeconnect.device.sftp.mountPoint"
                ])
                .output()
                .await
            {
                Ok(output) => Ok(output),
                Err(_) => {
                    tokio::process::Command::new("qdbus-qt5")
                        .args(&[
                            "org.kde.kdeconnect",
                            &format!("/modules/kdeconnect/devices/{}/sftp", device_id),
                            "org.kde.kdeconnect.device.sftp.mountPoint"
                        ])
                        .output()
                        .await
                }
            };
            
            if let Ok(mp_output) = mountpoint_result {
                if mp_output.status.success() {
                    let mount_point = String::from_utf8_lossy(&mp_output.stdout).trim().to_string();
                    eprintln!("Ã¢Å“â€œ Got mount point via qdbus: {}", mount_point);
                    
                    if !mount_point.is_empty() && std::path::Path::new(&mount_point).exists() {
                        open_in_file_manager(&mount_point).await;
                        return;
                    }
                }
            }
            
            eprintln!("Ã¢Å¡Â  Could not get valid mount point from qdbus");
        }
        Ok(output) => {
            eprintln!("Ã¢Å“â€” qdbus mount failed: {}", String::from_utf8_lossy(&output.stderr));
        }
        Err(e) => {
            eprintln!("Ã¢Å“â€” qdbus not available: {:?}", e);
        }
    }
    
    // Final fallback: try startBrowsing via qdbus
    eprintln!("Trying qdbus startBrowsing...");
    let _ = match tokio::process::Command::new("qdbus")
        .args(&[
            "org.kde.kdeconnect",
            &format!("/modules/kdeconnect/devices/{}/sftp", device_id),
            "org.kde.kdeconnect.device.sftp.startBrowsing"
        ])
        .spawn()
    {
        Ok(child) => Ok(child),
        Err(_) => {
            tokio::process::Command::new("qdbus-qt5")
                .args(&[
                    "org.kde.kdeconnect",
                    &format!("/modules/kdeconnect/devices/{}/sftp", device_id),
                    "org.kde.kdeconnect.device.sftp.startBrowsing"
                ])
                .spawn()
        }
    };
}

/// Try the startBrowsing D-Bus method
#[allow(dead_code)]
async fn try_start_browsing(conn: &Connection, path: &str) {
    eprintln!("=== Trying startBrowsing ===");
    match conn.call_method(
        Some("org.kde.kdeconnect"),
        path,
        Some("org.kde.kdeconnect.device.sftp"),
        "startBrowsing",
        &()
    ).await {
        Ok(_) => eprintln!("Ã¢Å“â€œ startBrowsing called successfully"),
        Err(e) => eprintln!("Ã¢Å“â€” startBrowsing failed: {:?}", e),
    }
}

/// Open a path in the file manager with multiple fallbacks
#[allow(dead_code)]
async fn open_in_file_manager(path: &str) {
    eprintln!("Opening file manager at: {}", path);
    
    // Try xdg-open first (respects user's default)
    match std::process::Command::new("xdg-open")
        .arg(path)
        .spawn()
    {
        Ok(_) => {
            eprintln!("Ã¢Å“â€œ Opened with xdg-open");
            return;
        }
        Err(e) => {
            eprintln!("Ã¢Å¡Â  xdg-open failed: {:?}", e);
        }
    }
    
    // Try COSMIC Files
    if std::process::Command::new("cosmic-files")
        .arg(path)
        .spawn()
        .is_ok()
    {
        eprintln!("Ã¢Å“â€œ Opened with cosmic-files");
        return;
    }
    
    // Try Dolphin (KDE's file manager, most likely to handle KDE Connect mounts well)
    if std::process::Command::new("dolphin")
        .arg(path)
        .spawn()
        .is_ok()
    {
        eprintln!("Ã¢Å“â€œ Opened with dolphin");
        return;
    }
    
    // Try Nautilus (GNOME)
    if std::process::Command::new("nautilus")
        .arg(path)
        .spawn()
        .is_ok()
    {
        eprintln!("Ã¢Å“â€œ Opened with nautilus");
        return;
    }
    
    // Try Thunar (XFCE)
    if std::process::Command::new("thunar")
        .arg(path)
        .spawn()
        .is_ok()
    {
        eprintln!("Ã¢Å“â€œ Opened with thunar");
        return;
    }
    
    eprintln!("Ã¢Å“â€” All file manager attempts failed");
    eprintln!("Mount point is at: {}", path);
    eprintln!("You can manually open it in your file manager");
}

pub async fn send_clipboard(device_id: String, content: String) {
    eprintln!("=== Sending Clipboard ===");
    eprintln!("Device: {}", device_id);
    
    if let Ok(conn) = get_connection().await {
        let path = format!("/modules/kdeconnect/devices/{}/clipboard", device_id);
        
        match conn.call_method(
            Some("org.kde.kdeconnect"),
            path.as_str(),
            Some("org.kde.kdeconnect.device.clipboard"),
            "sendClipboard",
            &(content,)
        ).await {
            Ok(_) => eprintln!("Ã¢Å“â€œ Clipboard sent"),
            Err(e) => eprintln!("Ã¢Å“â€” Failed to send clipboard: {:?}", e),
        }
    }
}

pub async fn lock_device(device_id: String) {
    eprintln!("=== Locking Device ===");
    eprintln!("Device: {}", device_id);
    
    if let Ok(conn) = get_connection().await {
        let path = format!("/modules/kdeconnect/devices/{}/lockdevice", device_id);
        
        match conn.call_method(
            Some("org.kde.kdeconnect"),
            path.as_str(),
            Some("org.kde.kdeconnect.device.lockdevice"),
            "lock",
            &()
        ).await {
            Ok(_) => eprintln!("Ã¢Å“â€œ Device locked"),
            Err(e) => eprintln!("Ã¢Å“â€” Failed to lock device: {:?}", e),
        }
    }
}

/// Helper to get bool property from plugin
async fn get_plugin_property_bool(conn: &Connection, path: &str, interface: &str, property: &str) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    let result = conn.call_method(
        Some("org.kde.kdeconnect"),
        path,
        Some("org.freedesktop.DBus.Properties"),
        "Get",
        &(interface, property)
    ).await?;
    
    let body = result.body();
    let value: zbus::zvariant::Value = body.deserialize()?;
    
    if let zbus::zvariant::Value::Bool(b) = value {
        Ok(b)
    } else {
        Ok(false)
    }
}

/// Helper to get i64 property from plugin
async fn get_plugin_property_i64(conn: &Connection, path: &str, interface: &str, property: &str) -> Result<i64, Box<dyn std::error::Error + Send + Sync>> {
    let result = conn.call_method(
        Some("org.kde.kdeconnect"),
        path,
        Some("org.freedesktop.DBus.Properties"),
        "Get",
        &(interface, property)
    ).await?;
    
    let body = result.body();
    let value: zbus::zvariant::Value = body.deserialize()?;
    
    if let zbus::zvariant::Value::I64(i) = value {
        Ok(i)
    } else {
        Ok(0)
    }
}

/// Helper to get int property from plugin
async fn get_plugin_property_int(conn: &Connection, path: &str, interface: &str, property: &str) -> Result<i32, Box<dyn std::error::Error + Send + Sync>> {
    let result = conn.call_method(
        Some("org.kde.kdeconnect"),
        path,
        Some("org.freedesktop.DBus.Properties"),
        "Get",
        &(interface, property)
    ).await?;
    
    let body = result.body();
    let value: zbus::zvariant::Value = body.deserialize()?;
    
    if let zbus::zvariant::Value::I32(i) = value {
        Ok(i)
    } else {
        Ok(0)
    }
}

/// Helper to get string property from plugin
async fn get_plugin_property_string(conn: &Connection, path: &str, interface: &str, property: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let result = conn.call_method(
        Some("org.kde.kdeconnect"),
        path,
        Some("org.freedesktop.DBus.Properties"),
        "Get",
        &(interface, property)
    ).await?;
    
    let body = result.body();
    let value: zbus::zvariant::Value = body.deserialize()?;
    
    if let zbus::zvariant::Value::Str(s) = value {
        Ok(s.to_string())
    } else {
        Err("Not a string".into())
    }
}

/// Get list of available media players on the phone
pub async fn get_media_player_list(device_id: String) -> Vec<String> {
    eprintln!("=== Getting Media Player List ===");
    eprintln!("Device: {}", device_id);
    
    if let Ok(conn) = get_connection().await {
        let path = format!("/modules/kdeconnect/devices/{}/mprisremote", device_id);
        
        let result = conn.call_method(
            Some("org.kde.kdeconnect"),
            path.as_str(),
            Some("org.freedesktop.DBus.Properties"),
            "Get",
            &("org.kde.kdeconnect.device.mprisremote", "playerList")
        ).await;
        
        match result {
            Ok(reply) => {
                let body = reply.body();
                if let Ok(value) = body.deserialize::<zbus::zvariant::Value>() {
                    if let zbus::zvariant::Value::Array(arr) = value {
                        let players: Vec<String> = arr.iter()
                            .filter_map(|v| {
                                if let zbus::zvariant::Value::Str(s) = v {
                                    Some(s.to_string())
                                } else {
                                    None
                                }
                            })
                            .collect();
                        eprintln!("Found {} players: {:?}", players.len(), players);
                        return players;
                    }
                }
            }
            Err(e) => {
                eprintln!("Failed to get player list: {:?}", e);
            }
        }
    }
    
    Vec::new()
}

/// Get current media player information from the phone
pub async fn get_media_player_info(device_id: String) -> Option<MediaPlayerInfo> {
    eprintln!("=== Getting Media Player Info ===");
    eprintln!("Device: {}", device_id);
    
    let conn = get_connection().await.ok()?;
    let path = format!("/modules/kdeconnect/devices/{}/mprisremote", device_id);
    let interface = "org.kde.kdeconnect.device.mprisremote";
    
    let player = get_plugin_property_string(&conn, &path, interface, "player").await.unwrap_or_default();
    let title = get_plugin_property_string(&conn, &path, interface, "title").await.unwrap_or_default();
    let artist = get_plugin_property_string(&conn, &path, interface, "artist").await.unwrap_or_default();
    let album = get_plugin_property_string(&conn, &path, interface, "album").await.unwrap_or_default();
    let is_playing = get_plugin_property_bool(&conn, &path, interface, "isPlaying").await.unwrap_or(false);
    let length = get_plugin_property_i64(&conn, &path, interface, "length").await.unwrap_or(0);
    let position = get_plugin_property_i64(&conn, &path, interface, "position").await.unwrap_or(0);
    let volume = get_plugin_property_int(&conn, &path, interface, "volume").await.unwrap_or(50);
    let can_pause = get_plugin_property_bool(&conn, &path, interface, "canPause").await.unwrap_or(true);
    let can_play = get_plugin_property_bool(&conn, &path, interface, "canPlay").await.unwrap_or(true);
    let can_go_next = get_plugin_property_bool(&conn, &path, interface, "canGoNext").await.unwrap_or(true);
    let can_go_previous = get_plugin_property_bool(&conn, &path, interface, "canGoPrevious").await.unwrap_or(true);
    let can_seek = get_plugin_property_bool(&conn, &path, interface, "canSeek").await.unwrap_or(false);
    
    eprintln!("Player: {}, Title: {}, Artist: {}, Playing: {}", player, title, artist, is_playing);
    
    Some(MediaPlayerInfo {
        player,
        title,
        artist,
        album,
        is_playing,
        length,
        position,
        volume,
        can_pause,
        can_play,
        can_go_next,
        can_go_previous,
        can_seek,
    })
}

/// Set which media player on the phone to control
pub async fn set_media_player(device_id: String, player: String) {
    eprintln!("=== Setting Media Player ===");
    eprintln!("Device: {}, Player: {}", device_id, player);
    
    if let Ok(conn) = get_connection().await {
        let path = format!("/modules/kdeconnect/devices/{}/mprisremote", device_id);
        
        let result = conn.call_method(
            Some("org.kde.kdeconnect"),
            path.as_str(),
            Some("org.freedesktop.DBus.Properties"),
            "Set",
            &("org.kde.kdeconnect.device.mprisremote", "player", zbus::zvariant::Value::new(player.as_str()))
        ).await;
        
        match result {
            Ok(_) => eprintln!("Ã¢Å“â€œ Player switched to '{}'", player),
            Err(e) => eprintln!("Ã¢Å“â€” Failed to switch player: {:?}", e),
        }
    }
}

/// Send a media action to the phone's media player
async fn send_media_action(device_id: String, action: &str) {
    eprintln!("=== Sending Media Action ===");
    eprintln!("Device: {}, Action: {}", device_id, action);
    
    if let Ok(conn) = get_connection().await {
        let path = format!("/modules/kdeconnect/devices/{}/mprisremote", device_id);
        
        let result = conn.call_method(
            Some("org.kde.kdeconnect"),
            path.as_str(),
            Some("org.kde.kdeconnect.device.mprisremote"),
            "sendAction",
            &(action,)
        ).await;
        
        match result {
            Ok(_) => eprintln!("Ã¢Å“â€œ Action '{}' sent successfully", action),
            Err(e) => eprintln!("Ã¢Å“â€” Failed to send action '{}': {:?}", action, e),
        }
    }
}

/// Set volume on the phone's media player (0-100)
pub async fn set_media_volume(device_id: String, volume: i32) {
    eprintln!("=== Setting Media Volume ===");
    eprintln!("Device: {}, Volume: {}", device_id, volume);
    
    let volume = volume.clamp(0, 100);
    
    if let Ok(conn) = get_connection().await {
        let path = format!("/modules/kdeconnect/devices/{}/mprisremote", device_id);
        
        let result = conn.call_method(
            Some("org.kde.kdeconnect"),
            path.as_str(),
            Some("org.freedesktop.DBus.Properties"),
            "Set",
            &("org.kde.kdeconnect.device.mprisremote", "volume", zbus::zvariant::Value::new(volume))
        ).await;
        
        match result {
            Ok(_) => eprintln!("Ã¢Å“â€œ Volume set to {}", volume),
            Err(e) => eprintln!("Ã¢Å“â€” Failed to set volume: {:?}", e),
        }
    }
}

/// Get current volume from the phone's media player
pub async fn get_media_volume(device_id: String) -> Option<i32> {
    if let Ok(conn) = get_connection().await {
        let path = format!("/modules/kdeconnect/devices/{}/mprisremote", device_id);
        let interface = "org.kde.kdeconnect.device.mprisremote";
        
        get_plugin_property_int(&conn, &path, interface, "volume").await.ok()
    } else {
        None
    }
}

// Media control convenience functions
pub async fn play_media(device_id: String) {
    send_media_action(device_id, "Play").await;
}

pub async fn pause_media(device_id: String) {
    send_media_action(device_id, "Pause").await;
}

#[allow(dead_code)]
pub async fn play_pause_media(device_id: String) {
    send_media_action(device_id, "PlayPause").await;
}

// Alias for main.rs compatibility
#[allow(dead_code)]
pub async fn media_play_pause(device_id: String) {
    play_pause_media(device_id).await;
}

pub async fn next_media(device_id: String) {
    send_media_action(device_id, "Next").await;
}

// Alias for main.rs compatibility
pub async fn media_next(device_id: String) {
    next_media(device_id).await;
}

pub async fn previous_media(device_id: String) {
    send_media_action(device_id, "Previous").await;
}

// Alias for main.rs compatibility
pub async fn media_previous(device_id: String) {
    previous_media(device_id).await;
}

#[allow(dead_code)]
pub async fn stop_media(device_id: String) {
    send_media_action(device_id, "Stop").await;
}