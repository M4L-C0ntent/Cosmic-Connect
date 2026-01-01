// src/messages.rs
#[derive(Debug, Clone)]
pub enum Message {
    // Popup control
    TogglePopup,
    
    // Device management
    RefreshDevices,
    DevicesUpdated(Vec<crate::models::Device>),
    PairDevice(String),
    UnpairDevice(String),
    AcceptPairing(String),
    RejectPairing(String),
    
    // Device menu toggle
    ToggleDeviceMenu(String),
    TogglePlayerMenu(String), // device_id
    
    // Communication
    PingDevice(String),
    SendSMS(String),
    ShareClipboard(String),
    
    // File operations
    SendFile(String),
    ShareUrl(String, String),
    BrowseDevice(String),
    
    // Remote control
    RemoteInput(String),
    RingDevice(String),
    LockDevice(String),
    
    // Media control
    MediaPlay(String),
    MediaPause(String),
    MediaNext(String),
    MediaPrevious(String),
    VolumeUp(String),
    VolumeDown(String),
    VolumeChanged(String, i32), // NEW: device_id, volume (0-100)
    MediaPlayerSelected(String, String), // device_id, player_name
    MediaInfoUpdated(String, Option<crate::dbus::MediaPlayerInfo>), // device_id, info
    RequestMediaInfo(String), // device_id
    RefreshMediaPlayers(String), // device_id
    MediaPlayersUpdated(String, Vec<String>), // device_id, players
    
    // Advanced features
    PresenterMode(String),
    UseAsMonitor(String),
    
    // Settings
    OpenSettings,
    
    // Pairing notifications
    PairingRequestReceived(String, String, String), // device_id, device_name, device_type
}