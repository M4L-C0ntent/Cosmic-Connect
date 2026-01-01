// src/models.rs
#[derive(Debug, Clone)]
pub struct Device {
    pub id: String,
    pub name: String,
    pub device_type: String,
    pub is_reachable: bool,
    pub is_paired: bool,
    pub battery_level: Option<i32>,
    pub is_charging: Option<bool>,
    pub has_battery: bool,
    pub has_ping: bool,
    pub has_share: bool,
    pub has_findmyphone: bool,
    pub has_sms: bool,
    pub has_clipboard: bool,
    pub has_contacts: bool,
    pub has_mpris: bool,
    pub has_remote_keyboard: bool,
    pub has_sftp: bool,
    pub has_presenter: bool,
    pub has_lockdevice: bool,
    pub has_virtualmonitor: bool,
    pub pairing_requests: i32,
    // Connectivity information
    pub signal_strength: Option<i32>,  // 0-4 bars, or -1 for no signal
    pub network_type: Option<String>,  // "5G", "4G", "3G", "2G", etc.
    // Media player information
    pub available_players: Vec<String>,
    pub current_player: Option<String>,
    pub media_info: Option<crate::dbus::MediaPlayerInfo>,
}

impl Device {
    pub fn device_icon(&self) -> &'static str {
        match self.device_type.as_str() {
            "phone" => "phone-symbolic",
            "tablet" => "tablet-symbolic",
            "desktop" => "computer-symbolic",
            "laptop" => "computer-symbolic",
            _ => "phone-symbolic",
        }
    }

    pub fn battery_icon(&self) -> &'static str {
        if let (Some(level), Some(charging)) = (self.battery_level, self.is_charging) {
            if charging {
                return "battery-full-charging-symbolic";
            }
            match level {
                0..=20 => "battery-level-20-symbolic",
                21..=40 => "battery-level-40-symbolic",
                41..=60 => "battery-level-60-symbolic",
                61..=80 => "battery-level-80-symbolic",
                _ => "battery-level-100-symbolic",
            }
        } else {
            "battery-symbolic"
        }
    }

    pub fn signal_icon(&self) -> Option<&'static str> {
        self.signal_strength.map(|strength| {
            match strength {
                -1 => "network-cellular-offline-symbolic",  // No signal
                0 => "network-cellular-signal-none-symbolic",
                1 => "network-cellular-signal-weak-symbolic",
                2 => "network-cellular-signal-ok-symbolic",
                3 => "network-cellular-signal-good-symbolic",
                4 => "network-cellular-signal-excellent-symbolic",
                _ => "network-cellular-symbolic",
            }
        })
    }
}