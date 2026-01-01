// src/settings.rs
//! Settings window for managing KDE Connect devices and permissions.
//!
//! This application provides a comprehensive interface for:
//! - Viewing and managing paired devices
//! - Pairing new devices
//! - Configuring plugin permissions (enable/disable features)
//! - Testing device features like ping and ring

use cosmic::app::{Core, Task};
use cosmic::iced::{Alignment, Length, Subscription};
use cosmic::widget::{self, segmented_button};
use cosmic::{Application, ApplicationExt, Element};
use std::collections::HashMap;
use zbus::Connection;

mod plugin_config;
use plugin_config::PluginConfigs;

#[derive(Debug, Clone)]
pub struct Device {
    id: String,
    name: String,
    device_type: String,
    is_reachable: bool,
    is_paired: bool,
    #[allow(dead_code)]
    is_trusted: bool,
    battery_level: Option<i32>,
    is_charging: Option<bool>,
    #[allow(dead_code)]
    has_battery: bool,
    has_ping: bool,
    #[allow(dead_code)]
    has_share: bool,
    has_findmyphone: bool,
    #[allow(dead_code)]
    has_sms: bool,
    #[allow(dead_code)]
    has_clipboard: bool,
    #[allow(dead_code)]
    has_contacts: bool,
    #[allow(dead_code)]
    has_mpris: bool,
    #[allow(dead_code)]
    has_remote_keyboard: bool,
    #[allow(dead_code)]
    has_notifications: bool,
    #[allow(dead_code)]
    has_sftp: bool,
    #[allow(dead_code)]
    has_presenter: bool,
    #[allow(dead_code)]
    has_lockdevice: bool,
    #[allow(dead_code)]
    has_virtualmonitor: bool,
    pairing_requests: i32,
}

/// Represents the permission/plugin states for a device.
#[derive(Debug, Clone)]
pub struct DevicePermissions {
    // Existing plugins
    pub sms: bool,
    pub share: bool,                     // Changed from filesystem - this is kdeconnect_share
    pub sftp: bool,                      // Added - this is kdeconnect_sftp  
    pub media_player: bool,
    pub volume_control: bool,
    pub connectivity_report: bool,
    pub remote_keypresses: bool,
    pub notifications: bool,
    pub pause_media_calls: bool,
    pub contacts_sync: bool,
    pub clipboard: bool,
    
    // Missing plugins from KDE Connect
    pub battery: bool,
    pub ping: bool,
    pub findmyphone: bool,
    pub presenter: bool,
    pub photo: bool,
    pub runcommand: bool,
    pub lockdevice: bool,
    pub telephony: bool,
    pub mpris_remote: bool,
    pub multimedia_receiver: bool,
    pub screensaver_inhibit: bool,
    pub virtualmonitor: bool,
    pub bigscreen: bool,
    pub mousepad: bool,              // Use phone as mouse/touchpad
    pub remotecontrol: bool,         // Remote control for media/presentations
    pub sendnotifications: bool,     // Send notifications TO phone
}

impl Default for DevicePermissions {
    fn default() -> Self {
        Self {
            // Usually enabled by default
            sms: false,
            share: true,                 // Share plugin enabled by default
            sftp: false,                 // SFTP typically needs explicit enable
            media_player: true,
            volume_control: true,
            connectivity_report: true,
            remote_keypresses: false,
            notifications: true,
            pause_media_calls: true,
            contacts_sync: false,
            clipboard: false,
            
            // Additional plugins
            battery: true,
            ping: true,
            findmyphone: true,
            presenter: true,
            photo: false,
            runcommand: true,
            lockdevice: true,
            telephony: true,
            mpris_remote: true,
            multimedia_receiver: true,
            screensaver_inhibit: false,
            virtualmonitor: false,
            bigscreen: false,
            mousepad: false,              // Disabled by default (security/UX)
            remotecontrol: false,         // Disabled by default
            sendnotifications: false,     // Disabled by default
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Page {
    PairedDevices,
    AvailableDevices,
}

pub struct KdeConnectSettings {
    core: Core,
    devices: HashMap<String, Device>,
    permissions: DevicePermissions,
    plugin_configs: PluginConfigs,
    selected_device: Option<String>,
    current_page: segmented_button::SingleSelectModel,
    page_id_paired: segmented_button::Entity,
    page_id_available: segmented_button::Entity,
    is_loading: bool,
    last_interaction: std::time::Instant,
    // Track which plugin configuration is currently expanded/visible
    expanded_plugin_config: Option<PermissionType>,
}

#[derive(Debug, Clone)]
pub enum Message {
    RefreshDevices,
    DevicesUpdated(Vec<Device>),
    SelectDevice(String),
    DeselectDevice,
    PairDevice(String),
    UnpairDevice(String),
    AcceptPairing(String),
    RejectPairing(String),
    PingDevice(String),
    RingDevice(String),
    PageSelected(segmented_button::Entity),
    TogglePermission(PermissionType),
    PermissionsLoaded(DevicePermissions),
    // Plugin configuration messages
    TogglePluginConfig(PermissionType),  // Toggle configuration visibility for a plugin
    UpdateShareDestination(String),
    SavePluginConfig(PermissionType),    // Save configuration for a specific plugin
    BrowseShareDestination,              // Open file picker for share plugin
    FolderSelected(Option<String>),      // Result from folder picker
    // Clipboard configuration messages
    ToggleClipboardAutoShare(bool),      // Toggle auto-share clipboard
    ToggleClipboardSendPassword(bool),   // Toggle send password content
    // RunCommand configuration messages
    AddRunCommand,                       // Add new command
    DeleteRunCommand(usize),             // Delete command by index
    UpdateRunCommandName(usize, String), // Update command name
    UpdateRunCommandCommand(usize, String), // Update command string
    // PauseMusic configuration messages
    TogglePauseMusicOnRinging(bool),     // Toggle pause on ringing
    TogglePauseMusicOnlyOnTalking(bool), // Toggle pause only while talking
    TogglePauseMusicPauseMedia(bool),    // Toggle pause media players
    TogglePauseMusicMuteSystem(bool),    // Toggle mute system sound
    TogglePauseMusicResumeAfter(bool),   // Toggle auto-resume after call
    // FindMyPhone configuration messages
    UpdateFindMyPhoneRingtone(String),   // Update ringtone path
    BrowseFindMyPhoneRingtone,           // Open file picker for ringtone
    RingtoneSelected(Option<String>),    // Result from ringtone picker
    // SendNotifications configuration messages
    ToggleSendNotificationsPersistentOnly(bool),  // Toggle persistent only
    ToggleSendNotificationsIncludeBody(bool),     // Toggle include body
    ToggleSendNotificationsSyncIcons(bool),       // Toggle sync icons
    UpdateSendNotificationsMinUrgency(i32),       // Update min urgency level
    ToggleSendNotificationsBlocklistMode(bool),   // Toggle blocklist vs allowlist
    AddSendNotificationsApp,                      // Add app to list
    RemoveSendNotificationsApp(usize),            // Remove app by index
    UpdateSendNotificationsAppName(usize, String), // Update app name
    ToggleSendNotificationsAppEnabled(usize, bool), // Toggle app enabled
    PluginConfigsLoaded(PluginConfigs),
}

#[derive(Debug, Clone, PartialEq)]
pub enum PermissionType {
    // Original plugins
    SMS,
    Share,                  // Changed from Filesystem - kdeconnect_share
    Sftp,                   // Added - kdeconnect_sftp (remote filesystem browser)
    MediaPlayer,
    VolumeControl,
    ConnectivityReport,
    RemoteKeypresses,
    Notifications,
    PauseMediaCalls,
    ContactsSync,
    Clipboard,
    
    // Additional plugins
    Battery,
    Ping,
    FindMyPhone,
    Presenter,
    Photo,
    RunCommand,
    LockDevice,
    Telephony,
    MprisRemote,
    MultimediaReceiver,
    ScreensaverInhibit,
    VirtualMonitor,
    Bigscreen,
    Mousepad,             // Use phone as mouse/touchpad
    RemoteControl,        // Remote control for media/presentations
    SendNotifications,    // Send notifications to phone
}

impl Application for KdeConnectSettings {
    type Executor = cosmic::executor::Default;
    type Flags = Option<String>; // kdeconnect:// URL from notifications
    type Message = Message;
    const APP_ID: &str = "io.github.M4LC0ntent.CosmicKdeConnectSettings";

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(core: Core, flags: Self::Flags) -> (Self, Task<Self::Message>) {
        let mut current_page = segmented_button::ModelBuilder::default();
        
        // Extract device ID from kdeconnect:// URL if present
        let (show_available, target_device_id) = if let Some(ref url) = flags {
            eprintln!("Settings launched with URL: {}", url);
            if url.starts_with("kdeconnect://pair/") {
                // Extract device ID from URL: kdeconnect://pair/{device_id}
                let device_id = url.trim_start_matches("kdeconnect://pair/").to_string();
                eprintln!("Extracted device ID from URL: {}", device_id);
                (true, Some(device_id))
            } else {
                (false, None)
            }
        } else {
            (false, None)
        };
        
        // Build tabs - activate Available Devices if launched from pairing
        current_page = current_page.insert(|b| {
            if show_available {
                b.text("Paired Devices")
            } else {
                b.text("Paired Devices").activate()
            }
        });
        
        current_page = current_page.insert(|b| {
            if show_available {
                b.text("Available Devices").activate()
            } else {
                b.text("Available Devices")
            }
        });
        
        let mut model = current_page.build();
        let page_id_paired = model.entity_at(0).unwrap();
        let page_id_available = model.entity_at(1).unwrap();

        let mut app = KdeConnectSettings {
            core,
            devices: HashMap::new(),
            permissions: DevicePermissions::default(),
            plugin_configs: PluginConfigs::load(""),  // Will be loaded when device is selected
            selected_device: target_device_id.clone(),
            current_page: model,
            page_id_paired,
            page_id_available,
            is_loading: true,
            last_interaction: std::time::Instant::now(),
            expanded_plugin_config: None,
        };

        let title_task = app.set_window_title("KDE Connect Settings".to_string(), app.core.main_window_id().unwrap());

        // If we have a target device from the URL, we'll select it after devices load
        let tasks = vec![
            title_task,
            cosmic::task::future(async move {
                Message::DevicesUpdated(fetch_devices().await)
            }),
        ];

        (app, Task::batch(tasks))
    }

    fn update(&mut self, message: Self::Message) -> Task<Self::Message> {
        // Update last interaction time for any user action
        self.last_interaction = std::time::Instant::now();
        
        match message {
            Message::RefreshDevices => {
                // Reduced from 5 to 1 second to allow refresh after pairing actions
                if self.last_interaction.elapsed().as_secs() < 1 {
                    return Task::none();
                }
                self.is_loading = true;
                return cosmic::task::future(async move {
                    Message::DevicesUpdated(fetch_devices().await)
                });
            }
            Message::DevicesUpdated(devices) => {
                self.devices.clear();
                for device in devices {
                    self.devices.insert(device.id.clone(), device);
                }
                self.is_loading = false;
            }
            Message::SelectDevice(device_id) => {
                eprintln!("=== Device Selected: {} ===", device_id);
                self.selected_device = Some(device_id.clone());
                
                // Load the actual plugin states for this device
                eprintln!("Loading permissions for device: {}", device_id);
                return cosmic::task::future(async move {
                    let permissions = load_device_permissions(device_id).await;
                    Message::PermissionsLoaded(permissions)
                });
            }
            Message::DeselectDevice => {
                self.selected_device = None;
            }
            Message::PairDevice(device_id) => {
                return cosmic::task::future(async move {
                    pair_device(device_id).await;
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                    Message::DevicesUpdated(fetch_devices().await)
                });
            }
            Message::UnpairDevice(device_id) => {
                self.selected_device = None;
                return cosmic::task::future(async move {
                    unpair_device(device_id).await;
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                    Message::DevicesUpdated(fetch_devices().await)
                });
            }
            Message::AcceptPairing(device_id) => {
                return cosmic::task::future(async move {
                    accept_pairing(device_id).await;
                    // Give pairing state time to propagate through D-Bus
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                    Message::DevicesUpdated(fetch_devices().await)
                });
            }
            Message::RejectPairing(device_id) => {
                return cosmic::task::future(async move {
                    reject_pairing(device_id).await;
                    // Give pairing state time to propagate through D-Bus
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                    Message::DevicesUpdated(fetch_devices().await)
                });
            }
            Message::PingDevice(device_id) => {
                return cosmic::task::future(async move {
                    ping_device(device_id).await;
                    Message::RefreshDevices
                });
            }
            Message::RingDevice(device_id) => {
                return cosmic::task::future(async move {
                    ring_device(device_id).await;
                    Message::RefreshDevices
                });
            }
            Message::PageSelected(entity) => {
                self.current_page.activate(entity);
                // Clear selected device when going to Available Devices
                if entity == self.page_id_available {
                    self.selected_device = None;
                }
            }
            Message::TogglePermission(perm) => {
                // Update local state immediately for UI responsiveness
                match perm {
                    PermissionType::SMS => self.permissions.sms = !self.permissions.sms,
                    PermissionType::Share => self.permissions.share = !self.permissions.share,
                    PermissionType::Sftp => self.permissions.sftp = !self.permissions.sftp,
                    PermissionType::MediaPlayer => self.permissions.media_player = !self.permissions.media_player,
                    PermissionType::VolumeControl => self.permissions.volume_control = !self.permissions.volume_control,
                    PermissionType::ConnectivityReport => self.permissions.connectivity_report = !self.permissions.connectivity_report,
                    PermissionType::RemoteKeypresses => self.permissions.remote_keypresses = !self.permissions.remote_keypresses,
                    PermissionType::Notifications => self.permissions.notifications = !self.permissions.notifications,
                    PermissionType::PauseMediaCalls => self.permissions.pause_media_calls = !self.permissions.pause_media_calls,
                    PermissionType::ContactsSync => self.permissions.contacts_sync = !self.permissions.contacts_sync,
                    PermissionType::Clipboard => self.permissions.clipboard = !self.permissions.clipboard,
                    PermissionType::Battery => self.permissions.battery = !self.permissions.battery,
                    PermissionType::Ping => self.permissions.ping = !self.permissions.ping,
                    PermissionType::FindMyPhone => self.permissions.findmyphone = !self.permissions.findmyphone,
                    PermissionType::Presenter => self.permissions.presenter = !self.permissions.presenter,
                    PermissionType::Photo => self.permissions.photo = !self.permissions.photo,
                    PermissionType::RunCommand => self.permissions.runcommand = !self.permissions.runcommand,
                    PermissionType::LockDevice => self.permissions.lockdevice = !self.permissions.lockdevice,
                    PermissionType::Telephony => self.permissions.telephony = !self.permissions.telephony,
                    PermissionType::MprisRemote => self.permissions.mpris_remote = !self.permissions.mpris_remote,
                    PermissionType::MultimediaReceiver => self.permissions.multimedia_receiver = !self.permissions.multimedia_receiver,
                    PermissionType::ScreensaverInhibit => self.permissions.screensaver_inhibit = !self.permissions.screensaver_inhibit,
                    PermissionType::VirtualMonitor => self.permissions.virtualmonitor = !self.permissions.virtualmonitor,
                    PermissionType::Bigscreen => self.permissions.bigscreen = !self.permissions.bigscreen,
                    PermissionType::Mousepad => self.permissions.mousepad = !self.permissions.mousepad,
                    PermissionType::RemoteControl => self.permissions.remotecontrol = !self.permissions.remotecontrol,
                    PermissionType::SendNotifications => self.permissions.sendnotifications = !self.permissions.sendnotifications,
                }

                // Apply the change via D-Bus if a device is selected
                if let Some(device_id) = &self.selected_device {
                    let device_id_clone = device_id.clone();
                    let plugin_name = permission_to_plugin_name(&perm);
                    let enabled = get_permission_state(&self.permissions, &perm);
                    
                    eprintln!("Toggling plugin: {} -> {}", plugin_name, enabled);
                    
                    return cosmic::task::future(async move {
                        match set_plugin_enabled_internal(device_id_clone.clone(), plugin_name, enabled).await {
                            Ok(_) => {
                                eprintln!("ÃƒÆ’Ã‚Â¢Ãƒâ€¦Ã¢â‚¬Å“ÃƒÂ¢Ã¢â€šÂ¬Ã…â€œ Plugin state changed successfully");
                                // Wait a moment for the device to process the change
                                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                                Message::RefreshDevices
                            }
                            Err(e) => {
                                eprintln!("ÃƒÆ’Ã‚Â¢Ãƒâ€¦Ã¢â‚¬Å“ÃƒÂ¢Ã¢â€šÂ¬Ã¢â‚¬Â Failed to change plugin state: {}", e);
                                Message::RefreshDevices
                            }
                        }
                    });
                }
            }
            Message::PermissionsLoaded(permissions) => {
                eprintln!("Ã¢Å“â€œ Loaded permissions for selected device");
                self.permissions = permissions;
                
                // Also load plugin configs for the selected device
                if let Some(device_id) = &self.selected_device {
                    let device_id = device_id.clone();
                    return cosmic::task::future(async move {
                        let configs = PluginConfigs::load(&device_id);
                        Message::PluginConfigsLoaded(configs)
                    });
                }
            }
            Message::PluginConfigsLoaded(configs) => {
                eprintln!("Ã¢Å“â€œ Loaded plugin configurations for selected device");
                self.plugin_configs = configs;
            }
            Message::TogglePluginConfig(plugin_type) => {
                // Toggle the expanded state for this plugin's configuration
                if self.expanded_plugin_config.as_ref() == Some(&plugin_type) {
                    self.expanded_plugin_config = None;
                } else {
                    self.expanded_plugin_config = Some(plugin_type);
                }
            }
            Message::UpdateShareDestination(path) => {
                self.plugin_configs.share.destination_path = path;
            }
            Message::BrowseShareDestination => {
                // Open folder picker using xdg-desktop-portal for native COSMIC integration
                return cosmic::task::future(async move {
                    eprintln!("Opening portal folder picker");
                    
                    if let Some(folder) = cosmic_connect_applet::portal::pick_folder("Select Download Folder").await {
                        eprintln!("âœ“ Folder selected: {}", folder);
                        Message::FolderSelected(Some(folder))
                    } else {
                        eprintln!("âœ— Folder selection cancelled");
                        Message::FolderSelected(None)
                    }
                });
            }
            Message::FolderSelected(path_opt) => {
                // Update the destination path if a folder was selected
                if let Some(path) = path_opt {
                    eprintln!("Ã¢Å“â€œ Folder selected: {}", path);
                    self.plugin_configs.share.destination_path = path;
                } else {
                    eprintln!("Ã¢Å“â€” Folder selection cancelled");
                }
            }
            Message::ToggleClipboardAutoShare(enabled) => {
                self.plugin_configs.clipboard.auto_share = enabled;
                eprintln!("Clipboard auto-share: {}", enabled);
            }
            Message::ToggleClipboardSendPassword(enabled) => {
                self.plugin_configs.clipboard.send_password = enabled;
                eprintln!("Clipboard send password: {}", enabled);
            }
            Message::AddRunCommand => {
                use crate::plugin_config::RemoteCommand;
                let new_command = RemoteCommand {
                    id: format!("command_{}", self.plugin_configs.runcommand.commands.len()),
                    name: "New Command".to_string(),
                    command: "echo 'Hello'".to_string(),
                };
                self.plugin_configs.runcommand.commands.push(new_command);
                eprintln!("Ã¢Å“â€œ Added new run command");
            }
            Message::DeleteRunCommand(index) => {
                if index < self.plugin_configs.runcommand.commands.len() {
                    self.plugin_configs.runcommand.commands.remove(index);
                    eprintln!("Ã¢Å“â€œ Deleted run command at index {}", index);
                }
            }
            Message::UpdateRunCommandName(index, name) => {
                if let Some(cmd) = self.plugin_configs.runcommand.commands.get_mut(index) {
                    cmd.name = name;
                }
            }
            Message::UpdateRunCommandCommand(index, command) => {
                if let Some(cmd) = self.plugin_configs.runcommand.commands.get_mut(index) {
                    cmd.command = command;
                }
            }
            Message::TogglePauseMusicOnRinging(enabled) => {
                self.plugin_configs.pausemusic.pause_on_ringing = enabled;
                // If enabling on-ringing, disable only-on-talking
                if enabled {
                    self.plugin_configs.pausemusic.pause_only_on_talking = false;
                }
                eprintln!("Pause on ringing: {}", enabled);
            }
            Message::TogglePauseMusicOnlyOnTalking(enabled) => {
                self.plugin_configs.pausemusic.pause_only_on_talking = enabled;
                // If enabling only-on-talking, disable on-ringing
                if enabled {
                    self.plugin_configs.pausemusic.pause_on_ringing = false;
                }
                eprintln!("Pause only on talking: {}", enabled);
            }
            Message::TogglePauseMusicPauseMedia(enabled) => {
                self.plugin_configs.pausemusic.pause_media = enabled;
                eprintln!("Pause media: {}", enabled);
            }
            Message::TogglePauseMusicMuteSystem(enabled) => {
                self.plugin_configs.pausemusic.mute_system_sound = enabled;
                eprintln!("Mute system: {}", enabled);
            }
            Message::TogglePauseMusicResumeAfter(enabled) => {
                self.plugin_configs.pausemusic.resume_after_call = enabled;
                eprintln!("Resume after call: {}", enabled);
            }
            Message::UpdateFindMyPhoneRingtone(path) => {
                self.plugin_configs.findmyphone.ringtone_path = path;
            }
            Message::BrowseFindMyPhoneRingtone => {
                return cosmic::task::future(async move {
                    eprintln!("Opening portal file picker for ringtone");
                    
                    // Create audio file filters
                    let filters = vec![
                        cosmic_connect_applet::portal::FileFilter::new("Audio files")
                            .patterns(vec![
                                "*.mp3".to_string(),
                                "*.ogg".to_string(),
                                "*.oga".to_string(),
                                "*.wav".to_string(),
                                "*.flac".to_string(),
                                "*.m4a".to_string(),
                            ]),
                        cosmic_connect_applet::portal::FileFilter::new("All files")
                            .pattern("*"),
                    ];
                    
                    let files = cosmic_connect_applet::portal::pick_files(
                        "Select Ringtone Sound File",
                        false,  // Single selection only
                        Some(filters),
                    ).await;
                    
                    if let Some(path) = files.first() {
                        eprintln!("âœ“ Ringtone selected: {}", path);
                        Message::RingtoneSelected(Some(path.clone()))
                    } else {
                        eprintln!("âœ— Ringtone selection cancelled");
                        Message::RingtoneSelected(None)
                    }
                });
            }
            Message::RingtoneSelected(path_opt) => {
                // Update the ringtone path if a file was selected
                if let Some(path) = path_opt {
                    eprintln!("Ã¢Å“â€œ Ringtone file selected: {}", path);
                    self.plugin_configs.findmyphone.ringtone_path = path;
                } else {
                    eprintln!("Ã¢Å“â€” Ringtone selection cancelled");
                }
            }
            Message::ToggleSendNotificationsPersistentOnly(enabled) => {
                self.plugin_configs.sendnotifications.persistent_only = enabled;
                eprintln!("Send notifications - persistent only: {}", enabled);
            }
            Message::ToggleSendNotificationsIncludeBody(enabled) => {
                self.plugin_configs.sendnotifications.include_body = enabled;
                eprintln!("Send notifications - include body: {}", enabled);
            }
            Message::ToggleSendNotificationsSyncIcons(enabled) => {
                self.plugin_configs.sendnotifications.sync_icons = enabled;
                eprintln!("Send notifications - sync icons: {}", enabled);
            }
            Message::UpdateSendNotificationsMinUrgency(level) => {
                use crate::plugin_config::UrgencyLevel;
                self.plugin_configs.sendnotifications.min_urgency = UrgencyLevel::from_i32(level);
                eprintln!("Send notifications - min urgency: {:?}", self.plugin_configs.sendnotifications.min_urgency);
            }
            Message::ToggleSendNotificationsBlocklistMode(is_blocklist) => {
                self.plugin_configs.sendnotifications.use_blocklist = is_blocklist;
                eprintln!("Send notifications - use blocklist: {}", is_blocklist);
            }
            Message::AddSendNotificationsApp => {
                use crate::plugin_config::AppNotificationSetting;
                let new_app = AppNotificationSetting {
                    app_name: "App Name".to_string(),
                    enabled: true,
                };
                self.plugin_configs.sendnotifications.app_settings.push(new_app);
                eprintln!("Ã¢Å“â€œ Added new app to notification settings");
            }
            Message::RemoveSendNotificationsApp(index) => {
                if index < self.plugin_configs.sendnotifications.app_settings.len() {
                    self.plugin_configs.sendnotifications.app_settings.remove(index);
                    eprintln!("Ã¢Å“â€œ Removed app at index {}", index);
                }
            }
            Message::UpdateSendNotificationsAppName(index, name) => {
                if let Some(app) = self.plugin_configs.sendnotifications.app_settings.get_mut(index) {
                    app.app_name = name;
                }
            }
            Message::ToggleSendNotificationsAppEnabled(index, enabled) => {
                if let Some(app) = self.plugin_configs.sendnotifications.app_settings.get_mut(index) {
                    app.enabled = enabled;
                }
            }
            Message::SavePluginConfig(plugin_type) => {
                if let Some(device_id) = &self.selected_device {
                    let device_id = device_id.clone();
                    let configs = self.plugin_configs.clone();
                    
                    return cosmic::task::future(async move {
                        match plugin_type {
                            PermissionType::Share => {
                                match configs.share.save(&device_id) {
                                    Ok(_) => {
                                        eprintln!("Ã¢Å“â€œ Saved share plugin configuration");
                                    }
                                    Err(e) => {
                                        eprintln!("Ã¢Å“â€” Failed to save share plugin configuration: {:?}", e);
                                    }
                                }
                            }
                            PermissionType::Clipboard => {
                                match configs.clipboard.save(&device_id) {
                                    Ok(_) => {
                                        eprintln!("Ã¢Å“â€œ Saved clipboard plugin configuration");
                                    }
                                    Err(e) => {
                                        eprintln!("Ã¢Å“â€” Failed to save clipboard plugin configuration: {:?}", e);
                                    }
                                }
                            }
                            PermissionType::RunCommand => {
                                match configs.runcommand.save(&device_id) {
                                    Ok(_) => {
                                        eprintln!("Ã¢Å“â€œ Saved runcommand plugin configuration");
                                    }
                                    Err(e) => {
                                        eprintln!("Ã¢Å“â€” Failed to save runcommand plugin configuration: {:?}", e);
                                    }
                                }
                            }
                            PermissionType::PauseMediaCalls => {
                                match configs.pausemusic.save(&device_id) {
                                    Ok(_) => {
                                        eprintln!("Ã¢Å“â€œ Saved pausemusic plugin configuration");
                                    }
                                    Err(e) => {
                                        eprintln!("Ã¢Å“â€” Failed to save pausemusic plugin configuration: {:?}", e);
                                    }
                                }
                            }
                            PermissionType::FindMyPhone => {
                                match configs.findmyphone.save(&device_id) {
                                    Ok(_) => {
                                        eprintln!("Ã¢Å“â€œ Saved findmyphone plugin configuration");
                                    }
                                    Err(e) => {
                                        eprintln!("Ã¢Å“â€” Failed to save findmyphone plugin configuration: {:?}", e);
                                    }
                                }
                            }
                            PermissionType::SendNotifications => {
                                match configs.sendnotifications.save(&device_id) {
                                    Ok(_) => {
                                        eprintln!("Ã¢Å“â€œ Saved sendnotifications plugin configuration");
                                    }
                                    Err(e) => {
                                        eprintln!("Ã¢Å“â€” Failed to save sendnotifications plugin configuration: {:?}", e);
                                    }
                                }
                            }
                            _ => {
                                eprintln!("Configuration save not implemented for {:?}", plugin_type);
                            }
                        }
                        Message::RefreshDevices
                    });
                }
            }
        }
        Task::none()
    }
    fn view(&self) -> Element<'_, Self::Message> {
        let spacing = cosmic::theme::active().cosmic().spacing;

        // Header with page selector
        let header = widget::row()
            .push(
                widget::segmented_button::horizontal(&self.current_page)
                    .on_activate(Message::PageSelected)
            )
            .push(widget::horizontal_space())
            .push(
                widget::button::standard("Refresh")
                    .on_press(Message::RefreshDevices)
            )
            .spacing(spacing.space_s)
            .padding([0, spacing.space_m]);

        let content: Element<Message> = if self.is_loading {
            widget::container(
                widget::text("Loading devices...")
                    .size(16)
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
        } else {
            let active_page = if self.current_page.active() == self.page_id_paired {
                Page::PairedDevices
            } else {
                Page::AvailableDevices
            };

            match active_page {
                Page::PairedDevices => self.view_paired_devices().into(),
                Page::AvailableDevices => self.view_available_devices().into(),
            }
        };

        widget::column()
            .push(header)
            .push(widget::divider::horizontal::default())
            .push(content)
            .spacing(spacing.space_s)
            .into()
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        // Refresh every 10 seconds instead of 3 to reduce interruptions
        cosmic::iced::time::every(std::time::Duration::from_secs(10))
            .map(|_| Message::RefreshDevices)
    }
}

impl KdeConnectSettings {
    fn view_paired_devices(&self) -> Element<'_, Message> {
        let spacing = cosmic::theme::active().cosmic().spacing;
        
        let paired_devices: Vec<_> = self
            .devices
            .values()
            .filter(|d| d.is_paired)
            .collect();

        if paired_devices.is_empty() {
            return widget::container(
                widget::text("No paired devices.\nGo to 'Available Devices' to pair a new device.")
                    .size(14)
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .padding(spacing.space_xl)
            .into();
        }

        let left_panel = {
            let mut list = widget::column().spacing(spacing.space_xs);
            
            for device in &paired_devices {
                let is_selected = self.selected_device.as_ref() == Some(&device.id);
                let device_id = device.id.clone();
                
                let button = widget::button::custom(
                    widget::row()
                        .push(widget::icon::from_name(device_icon(&device.device_type)).size(24))
                        .push(
                            widget::column()
                                .push(widget::text(&device.name).size(14))
                                .push(widget::text(if device.is_reachable { "Connected" } else { "Disconnected" }).size(12))
                                .spacing(2)
                        )
                        .spacing(spacing.space_s)
                        .align_y(Alignment::Center)
                        .padding(spacing.space_s)
                )
                .padding(0)
                .width(Length::Fill)
                .on_press(Message::SelectDevice(device_id));

                let item = if is_selected {
                    widget::container(button)
                        .class(cosmic::theme::Container::Primary)
                        .width(Length::Fill)
                } else {
                    widget::container(button).width(Length::Fill)
                };

                list = list.push(item);
            }

            widget::scrollable(list)
                .height(Length::Fill)
                .width(Length::Fixed(250.0))
        };

        let right_panel: Element<Message> = if let Some(device_id) = &self.selected_device {
            if let Some(device) = self.devices.get(device_id) {
                self.view_device_details(device)
            } else {
                widget::container(widget::text("Device not found"))
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .center_x(Length::Fill)
                    .center_y(Length::Fill)
                    .into()
            }
        } else {
            widget::container(widget::text("Select a device to view details").size(14))
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .into()
        };

        widget::container(
            widget::row()
                .push(left_panel)
                .push(widget::divider::vertical::default())
                .push(right_panel)
                .spacing(spacing.space_s)
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(spacing.space_m)
        .into()
    }

    fn view_available_devices(&self) -> Element<'_, Message> {
        let spacing = cosmic::theme::active().cosmic().spacing;
        
        let available_devices: Vec<_> = self
            .devices
            .values()
            .filter(|d| !d.is_paired && d.is_reachable)
            .collect();

        if available_devices.is_empty() {
            return widget::container(
                widget::column()
                    .push(widget::text("No available devices found").size(16))
                    .push(widget::text("Make sure KDE Connect is running on your device and it's on the same network.").size(12))
                    .spacing(spacing.space_s)
                    .align_x(Alignment::Center)
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .padding(spacing.space_xl)
            .into();
        }

        let mut list = widget::column().spacing(spacing.space_m);
        
        list = list.push(
            widget::text("Available devices on your network")
                .size(14)
        );

        for device in available_devices {
            let device_id = device.id.clone();
            let device_id_pair = device.id.clone();
            let device_id_reject = device.id.clone();

            let mut device_row = widget::row()
                .push(widget::icon::from_name(device_icon(&device.device_type)).size(32))
                .push(
                    widget::column()
                        .push(widget::text(&device.name).size(16))
                        .push(widget::text(&device.device_type).size(12))
                        .spacing(2)
                        .width(Length::Fill)
                )
                .spacing(spacing.space_m)
                .align_y(Alignment::Center);

            if device.pairing_requests > 0 {
                device_row = device_row.push(
                    widget::column()
                        .push(widget::text("Wants to pair").size(12))
                        .push(
                            widget::row()
                                .push(widget::button::suggested("Accept").on_press(Message::AcceptPairing(device_id_pair)))
                                .push(widget::button::destructive("Reject").on_press(Message::RejectPairing(device_id_reject)))
                                .spacing(spacing.space_xs)
                        )
                        .spacing(spacing.space_xxs)
                );
            } else {
                device_row = device_row.push(
                    widget::button::suggested("Pair").on_press(Message::PairDevice(device_id))
                );
            }

            let card = widget::container(device_row)
                .padding(spacing.space_m)
                .class(cosmic::theme::Container::Card)
                .width(Length::Fill);

            list = list.push(card);
        }

        widget::container(widget::scrollable(list))
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(spacing.space_m)
            .into()
    }


    fn view_device_details<'a>(&'a self, device: &'a Device) -> Element<'a, Message> {
        let spacing = cosmic::theme::active().cosmic().spacing;
        let device_id_unpair = device.id.clone();
        let device_id_ping = device.id.clone();
        let device_id_ring = device.id.clone();

        let mut details = widget::column().spacing(spacing.space_m);

        // Device header
        details = details.push(
            widget::row()
                .push(widget::icon::from_name(device_icon(&device.device_type)).size(48))
                .push(
                    widget::column()
                        .push(widget::text(&device.name).size(20))
                        .push(widget::text(&device.device_type).size(14))
                        .spacing(spacing.space_xxs)
                )
                .spacing(spacing.space_m)
                .align_y(Alignment::Center)
        );

        details = details.push(widget::divider::horizontal::default());

        // Status section
        let status_text = if device.is_reachable {
            "Connected and reachable"
        } else {
            "Paired but not reachable"
        };
        
        details = details.push(
            widget::column()
                .push(widget::text("Status").size(12))
                .push(widget::text(status_text).size(14))
                .spacing(spacing.space_xxs)
        );

        // Battery info
        if let (Some(level), Some(charging)) = (device.battery_level, device.is_charging) {
            let battery_text = if charging {
                format!("{}% (Charging)", level)
            } else {
                format!("{}%", level)
            };
            
            details = details.push(
                widget::column()
                    .push(widget::text("Battery").size(12))
                    .push(
                        widget::row()
                            .push(widget::icon::from_name(battery_icon(level, charging)).size(16))
                            .push(widget::text(battery_text).size(14))
                            .spacing(spacing.space_xs)
                            .align_y(Alignment::Center)
                    )
                    .spacing(spacing.space_xxs)
            );
        }


        details = details.push(widget::divider::horizontal::default());

        // Available Plugins section (KDE Connect style)
        details = details.push(widget::text("Available Plugins").size(14).font(cosmic::font::bold()));
        details = details.push(widget::text("Enable or disable plugins and configure their settings").size(12));

        // Define plugins with their properties: (name, description, enabled_state, permission_type, has_config)
        let plugins = vec![
            // Sorted alphabetically to match KDE Connect
            ("Battery monitor", "Show your phone battery next to your computer battery", self.permissions.battery, PermissionType::Battery, false),
            ("Bigscreen voice control", "Send voice commands to your TV running Plasma Bigscreen", self.permissions.bigscreen, PermissionType::Bigscreen, false),
            ("Clipboard", "Share the clipboard between devices", self.permissions.clipboard, PermissionType::Clipboard, true),
            ("Connectivity monitor", "Show your phone's network signal strength", self.permissions.connectivity_report, PermissionType::ConnectivityReport, false),
            ("Contacts", "Synchronize Contacts from the Connected Device to the Desktop", self.permissions.contacts_sync, PermissionType::ContactsSync, false),
            ("Find this device", "Find this device by making it play an alarm sound", self.permissions.findmyphone, PermissionType::FindMyPhone, true),
            ("Host remote commands", "Trigger commands predefined on the remote device", self.permissions.runcommand, PermissionType::RunCommand, true),
            ("Inhibit screensaver", "Inhibit the screensaver when the device is connected", self.permissions.screensaver_inhibit, PermissionType::ScreensaverInhibit, false),
            ("LockDevice", "Locks your systems", self.permissions.lockdevice, PermissionType::LockDevice, false),
            ("ModemManager Telephony integration", "Show notifications for incoming calls", self.permissions.telephony, PermissionType::Telephony, false),
            ("MprisRemote", "Control MPRIS services", self.permissions.mpris_remote, PermissionType::MprisRemote, false),
            ("Multimedia control receiver", "Remote control your music and videos", self.permissions.multimedia_receiver, PermissionType::MultimediaReceiver, false),
            ("Mousepad", "Use your phone as a wireless mouse and keyboard", self.permissions.mousepad, PermissionType::Mousepad, false),
            ("Pause media during calls", "Pause music/videos during a phone call", self.permissions.pause_media_calls, PermissionType::PauseMediaCalls, true),
            ("Photo", "Use a connected device to take a photo", self.permissions.photo, PermissionType::Photo, false),
            ("Ping", "Send and receive pings", self.permissions.ping, PermissionType::Ping, false),
            ("Presenter", "Use your mobile device to point to things on the screen", self.permissions.presenter, PermissionType::Presenter, false),
            ("Receive notifications", "Show device's notifications on this computer and keep them in sync", self.permissions.notifications, PermissionType::Notifications, false),
            ("Remote control", "Control system volume and multimedia players remotely", self.permissions.remotecontrol, PermissionType::RemoteControl, false),
            ("Remote filesystem browser", "Browse files on the device remotely using SFTP", self.permissions.sftp, PermissionType::Sftp, false),
            ("Remote keypresses", "Receive remote keyboard input", self.permissions.remote_keypresses, PermissionType::RemoteKeypresses, false),
            ("Share", "Send and receive files", self.permissions.share, PermissionType::Share, true),
            ("Send notifications", "Send notifications to your phone from this computer", self.permissions.sendnotifications, PermissionType::SendNotifications, true),
            ("SMS Messages", "Send and receive SMS messages", self.permissions.sms, PermissionType::SMS, false),
            ("Virtual monitor", "Use your phone as a virtual monitor", self.permissions.virtualmonitor, PermissionType::VirtualMonitor, false),
            ("Volume control", "Control device volume remotely", self.permissions.volume_control, PermissionType::VolumeControl, false),
        ];


        for (label, description, enabled, perm_type, has_config) in plugins {
            let perm_clone = perm_type.clone();
            
            // Main plugin row
            let toggle = widget::toggler(enabled)
                .on_toggle(move |_| Message::TogglePermission(perm_clone.clone()));
            
            // Create description column with optional note
            let mut desc_column = widget::column()
                .spacing(spacing.space_xxs)
                .push(widget::text(label).size(13))
                .push(widget::text(description).size(11));
            
            // Add special note for SFTP plugin
            if matches!(perm_type, PermissionType::Sftp) {
                desc_column = desc_column.push(
                    widget::text("Note: Requires allowing permissions to device's filesystem if available")
                        .size(10)
                );
            }
            
            let mut plugin_row = widget::row()
                .push(desc_column.width(Length::Fill))
                .spacing(spacing.space_m)
                .align_y(Alignment::Center);
            
            // Add configure button if plugin has configuration options
            // Configure button goes BEFORE toggle (between description and toggle)
            if has_config && enabled {
                let perm_clone3 = perm_type.clone();
                plugin_row = plugin_row.push(
                    widget::button::text("Configure")
                        .on_press(Message::TogglePluginConfig(perm_clone3))
                        .class(cosmic::theme::Button::Text)
                );
            }
            
            // Toggle always on the right edge
            plugin_row = plugin_row.push(toggle);
            
            let plugin_container = widget::container(plugin_row)
                .padding([spacing.space_xs, spacing.space_m, spacing.space_xs, 0]);
            
            details = details.push(plugin_container);
            
            // Show configuration UI if this plugin is expanded
            if self.expanded_plugin_config.as_ref() == Some(&perm_type) {
                let config_ui = self.view_plugin_config(&perm_type);
                details = details.push(config_ui);
            }
            
            details = details.push(widget::divider::horizontal::light());
        }

        details = details.push(widget::vertical_space().height(spacing.space_m));
        details = details.push(widget::vertical_space().height(spacing.space_m));

        // Actions
        if device.is_reachable {
            let mut actions = widget::row().spacing(spacing.space_xs);
            
            if device.has_ping {
                actions = actions.push(
                    widget::button::standard("Send Ping")
                        .on_press(Message::PingDevice(device_id_ping))
                );
            }
            
            if device.has_findmyphone {
                actions = actions.push(
                    widget::button::standard("Ring Device")
                        .on_press(Message::RingDevice(device_id_ring))
                );
            }
            
            details = details.push(actions);
        }

        details = details.push(
            widget::button::destructive("Unpair Device")
                .on_press(Message::UnpairDevice(device_id_unpair))
                .width(Length::Fill)
        );

        widget::container(widget::scrollable(details))
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(spacing.space_m)
            .into()
    }

    /// View configuration UI for a specific plugin type
    fn view_plugin_config<'a>(&'a self, plugin_type: &PermissionType) -> Element<'a, Message> {
        let spacing = cosmic::theme::active().cosmic().spacing;
        
        match plugin_type {
            PermissionType::Share => {
                // Share plugin configuration (file transfer)
                widget::container(
                    widget::column()
                        .spacing(spacing.space_xs)
                        .push(
                            widget::text("Download Location").size(12).font(cosmic::font::bold())
                        )
                        .push(
                            widget::text("Choose where files received from this device are saved").size(11)
                        )
                        .push(
                            widget::row()
                                .push(
                                    widget::text_input("Path to download folder", &self.plugin_configs.share.destination_path)
                                        .on_input(Message::UpdateShareDestination)
                                        .width(Length::Fill)
                                )
                                .push(
                                    widget::button::standard("Browse")
                                        .on_press(Message::BrowseShareDestination)
                                )
                                .push(
                                    widget::button::suggested("Save")
                                        .on_press(Message::SavePluginConfig(PermissionType::Share))
                                )
                                .spacing(spacing.space_xs)
                        )
                        .padding([spacing.space_s, spacing.space_m])
                )
                .class(cosmic::theme::Container::Card)
                .width(Length::Fill)
                .into()
            }
            PermissionType::Clipboard => {
                // Clipboard plugin configuration
                widget::container(
                    widget::column()
                        .spacing(spacing.space_xs)
                        .push(
                            widget::text("Clipboard Sync Options").size(12).font(cosmic::font::bold())
                        )
                        .push(
                            widget::text("Configure how clipboard content is shared with this device").size(11)
                        )
                        .push(widget::Space::with_height(Length::Fixed(spacing.space_xs as f32)))
                        .push(
                            widget::row()
                                .push(
                                    widget::column()
                                        .push(widget::text("Automatically share clipboard").size(12))
                                        .push(widget::text("Sync clipboard content between devices automatically").size(10))
                                        .spacing(spacing.space_xxxs)
                                        .width(Length::Fill)
                                )
                                .push(
                                    widget::toggler(self.plugin_configs.clipboard.auto_share)
                                        .on_toggle(Message::ToggleClipboardAutoShare)
                                )
                                .spacing(spacing.space_m)
                                .align_y(cosmic::iced::Alignment::Center)
                        )
                        .push(widget::Space::with_height(Length::Fixed(spacing.space_xs as f32)))
                        .push(
                            widget::row()
                                .push(
                                    widget::column()
                                        .push(widget::text("Include password content").size(12))
                                        .push(widget::text("Share clipboard content marked as passwords by password managers").size(10))
                                        .spacing(spacing.space_xxxs)
                                        .width(Length::Fill)
                                )
                                .push(
                                    widget::toggler(self.plugin_configs.clipboard.send_password)
                                        .on_toggle(Message::ToggleClipboardSendPassword)
                                )
                                .spacing(spacing.space_m)
                                .align_y(cosmic::iced::Alignment::Center)
                        )
                        .push(widget::Space::with_height(Length::Fixed(spacing.space_s as f32)))
                        .push(
                            widget::row()
                                .push(widget::Space::with_width(Length::Fill))
                                .push(
                                    widget::button::suggested("Save")
                                        .on_press(Message::SavePluginConfig(PermissionType::Clipboard))
                                )
                        )
                        .padding([spacing.space_s, spacing.space_m])
                )
                .class(cosmic::theme::Container::Card)
                .width(Length::Fill)
                .into()
            }
            PermissionType::RunCommand => {
                // RunCommand plugin configuration (host remote commands)
                let mut column = widget::column()
                    .spacing(spacing.space_xs)
                    .push(
                        widget::text("Remote Commands").size(12).font(cosmic::font::bold())
                    )
                    .push(
                        widget::text("Define commands that can be triggered from your phone").size(11)
                    )
                    .push(widget::Space::with_height(Length::Fixed(spacing.space_xs as f32)));
                
                // List existing commands
                for (index, cmd) in self.plugin_configs.runcommand.commands.iter().enumerate() {
                    column = column.push(
                        widget::container(
                            widget::column()
                                .spacing(spacing.space_xxs)
                                .push(
                                    widget::row()
                                        .push(
                                            widget::text(format!("Command {}", index + 1))
                                                .size(11)
                                                .font(cosmic::font::bold())
                                        )
                                        .push(widget::Space::with_width(Length::Fill))
                                        .push(
                                            widget::button::destructive("Delete")
                                                .on_press(Message::DeleteRunCommand(index))
                                        )
                                        .spacing(spacing.space_xs)
                                        .align_y(cosmic::iced::Alignment::Center)
                                )
                                .push(
                                    widget::row()
                                        .push(widget::text("Name:").size(10).width(Length::Fixed(80.0)))
                                        .push(
                                            widget::text_input("Command name (shown on phone)", &cmd.name)
                                                .on_input(move |s| Message::UpdateRunCommandName(index, s))
                                                .width(Length::Fill)
                                        )
                                        .spacing(spacing.space_xs)
                                        .align_y(cosmic::iced::Alignment::Center)
                                )
                                .push(
                                    widget::row()
                                        .push(widget::text("Command:").size(10).width(Length::Fixed(80.0)))
                                        .push(
                                            widget::text_input("Shell command to execute", &cmd.command)
                                                .on_input(move |s| Message::UpdateRunCommandCommand(index, s))
                                                .width(Length::Fill)
                                        )
                                        .spacing(spacing.space_xs)
                                        .align_y(cosmic::iced::Alignment::Center)
                                )
                                .padding(spacing.space_xs)
                        )
                        .class(cosmic::theme::Container::Background)
                        .width(Length::Fill)
                    );
                    
                    column = column.push(widget::Space::with_height(Length::Fixed(spacing.space_xs as f32)));
                }
                
                // Add command button and save button
                column = column.push(
                    widget::row()
                        .push(
                            widget::button::standard("Add Command")
                                .on_press(Message::AddRunCommand)
                        )
                        .push(widget::Space::with_width(Length::Fill))
                        .push(
                            widget::button::suggested("Save")
                                .on_press(Message::SavePluginConfig(PermissionType::RunCommand))
                        )
                        .spacing(spacing.space_xs)
                );
                
                widget::container(column.padding([spacing.space_s, spacing.space_m]))
                    .class(cosmic::theme::Container::Card)
                    .width(Length::Fill)
                    .into()
            }
            PermissionType::PauseMediaCalls => {
                // PauseMusic plugin configuration (pause media during calls)
                widget::container(
                    widget::column()
                        .spacing(spacing.space_xs)
                        .push(
                            widget::text("Pause Media During Calls").size(12).font(cosmic::font::bold())
                        )
                        .push(
                            widget::text("Configure when and how media is paused during phone calls").size(11)
                        )
                        .push(widget::Space::with_height(Length::Fixed(spacing.space_s as f32)))
                        
                        // Condition section
                        .push(
                            widget::text("Condition").size(11).font(cosmic::font::bold())
                        )
                        .push(widget::Space::with_height(Length::Fixed(spacing.space_xxs as f32)))
                        .push(
                            widget::row()
                                .push(
                                    widget::column()
                                        .push(widget::text("Pause as soon as phone rings").size(12))
                                        .push(widget::text("Pause immediately when a call comes in").size(10))
                                        .spacing(spacing.space_xxxs)
                                        .width(Length::Fill)
                                )
                                .push(
                                    widget::toggler(self.plugin_configs.pausemusic.pause_on_ringing)
                                        .on_toggle(Message::TogglePauseMusicOnRinging)
                                )
                                .spacing(spacing.space_m)
                                .align_y(cosmic::iced::Alignment::Center)
                        )
                        .push(widget::Space::with_height(Length::Fixed(spacing.space_xs as f32)))
                        .push(
                            widget::row()
                                .push(
                                    widget::column()
                                        .push(widget::text("Pause only while talking").size(12))
                                        .push(widget::text("Pause only when call is answered").size(10))
                                        .spacing(spacing.space_xxxs)
                                        .width(Length::Fill)
                                )
                                .push(
                                    widget::toggler(self.plugin_configs.pausemusic.pause_only_on_talking)
                                        .on_toggle(Message::TogglePauseMusicOnlyOnTalking)
                                )
                                .spacing(spacing.space_m)
                                .align_y(cosmic::iced::Alignment::Center)
                        )
                        
                        .push(widget::Space::with_height(Length::Fixed(spacing.space_s as f32)))
                        
                        // Actions section
                        .push(
                            widget::text("Actions").size(11).font(cosmic::font::bold())
                        )
                        .push(widget::Space::with_height(Length::Fixed(spacing.space_xxs as f32)))
                        .push(
                            widget::row()
                                .push(
                                    widget::column()
                                        .push(widget::text("Pause media players").size(12))
                                        .push(widget::text("Pause music and video playback").size(10))
                                        .spacing(spacing.space_xxxs)
                                        .width(Length::Fill)
                                )
                                .push(
                                    widget::toggler(self.plugin_configs.pausemusic.pause_media)
                                        .on_toggle(Message::TogglePauseMusicPauseMedia)
                                )
                                .spacing(spacing.space_m)
                                .align_y(cosmic::iced::Alignment::Center)
                        )
                        .push(widget::Space::with_height(Length::Fixed(spacing.space_xs as f32)))
                        .push(
                            widget::row()
                                .push(
                                    widget::column()
                                        .push(widget::text("Mute system sound").size(12))
                                        .push(widget::text("Mute all system audio output").size(10))
                                        .spacing(spacing.space_xxxs)
                                        .width(Length::Fill)
                                )
                                .push(
                                    widget::toggler(self.plugin_configs.pausemusic.mute_system_sound)
                                        .on_toggle(Message::TogglePauseMusicMuteSystem)
                                )
                                .spacing(spacing.space_m)
                                .align_y(cosmic::iced::Alignment::Center)
                        )
                        .push(widget::Space::with_height(Length::Fixed(spacing.space_xs as f32)))
                        .push(
                            widget::row()
                                .push(
                                    widget::column()
                                        .push(widget::text("Automatically resume media when call has finished").size(12))
                                        .push(widget::text("Resume playback after call ends").size(10))
                                        .spacing(spacing.space_xxxs)
                                        .width(Length::Fill)
                                )
                                .push(
                                    widget::toggler(self.plugin_configs.pausemusic.resume_after_call)
                                        .on_toggle(Message::TogglePauseMusicResumeAfter)
                                )
                                .spacing(spacing.space_m)
                                .align_y(cosmic::iced::Alignment::Center)
                        )
                        
                        .push(widget::Space::with_height(Length::Fixed(spacing.space_s as f32)))
                        .push(
                            widget::row()
                                .push(widget::Space::with_width(Length::Fill))
                                .push(
                                    widget::button::suggested("Save")
                                        .on_press(Message::SavePluginConfig(PermissionType::PauseMediaCalls))
                                )
                        )
                        .padding([spacing.space_s, spacing.space_m])
                )
                .class(cosmic::theme::Container::Card)
                .width(Length::Fill)
                .into()
            }
            PermissionType::FindMyPhone => {
                // FindMyPhone plugin configuration (find this device)
                widget::container(
                    widget::column()
                        .spacing(spacing.space_xs)
                        .push(
                            widget::text("Find This Device Sound").size(12).font(cosmic::font::bold())
                        )
                        .push(
                            widget::text("Choose the sound file that plays when you trigger 'Find my device'").size(11)
                        )
                        .push(widget::Space::with_height(Length::Fixed(spacing.space_xs as f32)))
                        .push(
                            widget::text("Ringtone File").size(11).font(cosmic::font::bold())
                        )
                        .push(
                            widget::text("Select an audio file to play when finding this device").size(10)
                        )
                        .push(widget::Space::with_height(Length::Fixed(spacing.space_xxs as f32)))
                        .push(
                            widget::row()
                                .push(
                                    widget::text_input(
                                        "Path to sound file (e.g., /usr/share/sounds/...", 
                                        &self.plugin_configs.findmyphone.ringtone_path
                                    )
                                        .on_input(Message::UpdateFindMyPhoneRingtone)
                                        .width(Length::Fill)
                                )
                                .push(
                                    widget::button::standard("Browse")
                                        .on_press(Message::BrowseFindMyPhoneRingtone)
                                )
                                .spacing(spacing.space_xs)
                        )
                        .push(widget::Space::with_height(Length::Fixed(spacing.space_xs as f32)))
                        .push(
                            widget::text("Supported formats: MP3, OGG, WAV, FLAC, M4A").size(10)
                        )
                        .push(
                            widget::text("Tip: Choose a loud, distinctive sound that's easy to hear").size(10)
                        )
                        .push(widget::Space::with_height(Length::Fixed(spacing.space_s as f32)))
                        .push(
                            widget::row()
                                .push(widget::Space::with_width(Length::Fill))
                                .push(
                                    widget::button::suggested("Save")
                                        .on_press(Message::SavePluginConfig(PermissionType::FindMyPhone))
                                )
                        )
                        .padding([spacing.space_s, spacing.space_m])
                )
                .class(cosmic::theme::Container::Card)
                .width(Length::Fill)
                .into()
            }
            PermissionType::SendNotifications => {
                // SendNotifications plugin configuration
                let mut column = widget::column()
                    .spacing(spacing.space_xs)
                    .push(
                        widget::text("Send Notifications to Phone").size(12).font(cosmic::font::bold())
                    )
                    .push(
                        widget::text("Configure which computer notifications are sent to your phone").size(11)
                    )
                    .push(widget::Space::with_height(Length::Fixed(spacing.space_s as f32)))
                    
                    // General settings
                    .push(
                        widget::text("General Settings").size(11).font(cosmic::font::bold())
                    )
                    .push(widget::Space::with_height(Length::Fixed(spacing.space_xxs as f32)))
                    .push(
                        widget::row()
                            .push(
                                widget::column()
                                    .push(widget::text("Persistent notifications only").size(12))
                                    .push(widget::text("Only send notifications that stay visible").size(10))
                                    .spacing(spacing.space_xxxs)
                                    .width(Length::Fill)
                            )
                            .push(
                                widget::toggler(self.plugin_configs.sendnotifications.persistent_only)
                                    .on_toggle(Message::ToggleSendNotificationsPersistentOnly)
                            )
                            .spacing(spacing.space_m)
                            .align_y(cosmic::iced::Alignment::Center)
                    )
                    .push(widget::Space::with_height(Length::Fixed(spacing.space_xs as f32)))
                    .push(
                        widget::row()
                            .push(
                                widget::column()
                                    .push(widget::text("Include body").size(12))
                                    .push(widget::text("Send notification body text, not just title").size(10))
                                    .spacing(spacing.space_xxxs)
                                    .width(Length::Fill)
                            )
                            .push(
                                widget::toggler(self.plugin_configs.sendnotifications.include_body)
                                    .on_toggle(Message::ToggleSendNotificationsIncludeBody)
                            )
                            .spacing(spacing.space_m)
                            .align_y(cosmic::iced::Alignment::Center)
                    )
                    .push(widget::Space::with_height(Length::Fixed(spacing.space_xs as f32)))
                    .push(
                        widget::row()
                            .push(
                                widget::column()
                                    .push(widget::text("Sync icons").size(12))
                                    .push(widget::text("Send notification icons to phone").size(10))
                                    .spacing(spacing.space_xxxs)
                                    .width(Length::Fill)
                            )
                            .push(
                                widget::toggler(self.plugin_configs.sendnotifications.sync_icons)
                                    .on_toggle(Message::ToggleSendNotificationsSyncIcons)
                            )
                            .spacing(spacing.space_m)
                            .align_y(cosmic::iced::Alignment::Center)
                    )
                    
                    .push(widget::Space::with_height(Length::Fixed(spacing.space_s as f32)))
                    
                    // Minimum urgency level
                    .push(
                        widget::text("Minimum Urgency Level").size(11).font(cosmic::font::bold())
                    )
                    .push(
                        widget::text("Only send notifications at or above this urgency").size(10)
                    )
                    .push(widget::Space::with_height(Length::Fixed(spacing.space_xxs as f32)))
                    .push(
                        widget::row()
                            .push(
                                widget::radio(
                                    "Low (all notifications)",
                                    0,
                                    Some(self.plugin_configs.sendnotifications.min_urgency as i32),
                                    Message::UpdateSendNotificationsMinUrgency,
                                )
                            )
                            .push(
                                widget::radio(
                                    "Normal (default)",
                                    1,
                                    Some(self.plugin_configs.sendnotifications.min_urgency as i32),
                                    Message::UpdateSendNotificationsMinUrgency,
                                )
                            )
                            .push(
                                widget::radio(
                                    "Critical (urgent only)",
                                    2,
                                    Some(self.plugin_configs.sendnotifications.min_urgency as i32),
                                    Message::UpdateSendNotificationsMinUrgency,
                                )
                            )
                            .spacing(spacing.space_m)
                    )
                    
                    .push(widget::Space::with_height(Length::Fixed(spacing.space_s as f32)))
                    
                    // App-specific settings
                    .push(
                        widget::text("App-Specific Settings").size(11).font(cosmic::font::bold())
                    )
                    .push(widget::Space::with_height(Length::Fixed(spacing.space_xxs as f32)))
                    .push(
                        widget::row()
                            .push(
                                widget::column()
                                    .push(widget::text("Blocklist mode").size(12))
                                    .push(widget::text("If enabled, block listed apps. If disabled, only allow listed apps.").size(10))
                                    .spacing(spacing.space_xxxs)
                                    .width(Length::Fill)
                            )
                            .push(
                                widget::toggler(self.plugin_configs.sendnotifications.use_blocklist)
                                    .on_toggle(Message::ToggleSendNotificationsBlocklistMode)
                            )
                            .spacing(spacing.space_m)
                            .align_y(cosmic::iced::Alignment::Center)
                    );
                
                // List apps
                if !self.plugin_configs.sendnotifications.app_settings.is_empty() {
                    column = column.push(widget::Space::with_height(Length::Fixed(spacing.space_xs as f32)));
                    
                    let mode_text = if self.plugin_configs.sendnotifications.use_blocklist {
                        "Blocked Apps:"
                    } else {
                        "Allowed Apps:"
                    };
                    column = column.push(widget::text(mode_text).size(10));
                    
                    for (index, app) in self.plugin_configs.sendnotifications.app_settings.iter().enumerate() {
                        column = column.push(
                            widget::container(
                                widget::row()
                                    .push(
                                        widget::text_input("App name", &app.app_name)
                                            .on_input(move |s| Message::UpdateSendNotificationsAppName(index, s))
                                            .width(Length::Fill)
                                    )
                                    .push(
                                        widget::toggler(app.enabled)
                                            .on_toggle(move |e| Message::ToggleSendNotificationsAppEnabled(index, e))
                                    )
                                    .push(
                                        widget::button::destructive("Remove")
                                            .on_press(Message::RemoveSendNotificationsApp(index))
                                    )
                                    .spacing(spacing.space_xs)
                                    .align_y(cosmic::iced::Alignment::Center)
                                    .padding(spacing.space_xs)
                            )
                            .class(cosmic::theme::Container::Background)
                            .width(Length::Fill)
                        );
                        
                        column = column.push(widget::Space::with_height(Length::Fixed(spacing.space_xxs as f32)));
                    }
                }
                
                // Add app button
                column = column.push(widget::Space::with_height(Length::Fixed(spacing.space_xs as f32)));
                column = column.push(
                    widget::row()
                        .push(
                            widget::button::standard("Add App")
                                .on_press(Message::AddSendNotificationsApp)
                        )
                        .push(widget::Space::with_width(Length::Fill))
                        .push(
                            widget::button::suggested("Save")
                                .on_press(Message::SavePluginConfig(PermissionType::SendNotifications))
                        )
                        .spacing(spacing.space_xs)
                );
                
                widget::container(column.padding([spacing.space_s, spacing.space_m]))
                    .class(cosmic::theme::Container::Card)
                    .width(Length::Fill)
                    .into()
            }
            _ => {
                // Placeholder for other plugin configurations
                widget::container(
                    widget::text("Configuration options coming soon")
                        .size(11)
                )
                .padding(spacing.space_m)
                .into()
            }
        }
    }
}

fn device_icon(device_type: &str) -> &'static str {
    match device_type {
        "phone" => "phone-symbolic",
        "tablet" => "tablet-symbolic",
        "desktop" => "computer-symbolic",
        "laptop" => "computer-symbolic",
        _ => "phone-symbolic",
    }
}

fn battery_icon(level: i32, charging: bool) -> &'static str {
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
}

// D-Bus helper functions

async fn fetch_devices() -> Vec<Device> {
    let mut devices = Vec::new();

    match Connection::session().await {
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
                                let is_trusted = get_device_property_bool(&conn, &path, "isTrusted").await.unwrap_or(false);
                                let device_type = get_device_property(&conn, &path, "type").await.unwrap_or_else(|_| "phone".to_string());
                                
                                let has_battery = check_plugin(&conn, &path, "kdeconnect_battery").await;
                                let has_ping = check_plugin(&conn, &path, "kdeconnect_ping").await;
                                let has_share = check_plugin(&conn, &path, "kdeconnect_share").await;
                                let has_findmyphone = check_plugin(&conn, &path, "kdeconnect_findmyphone").await;
                                let has_sms = check_plugin(&conn, &path, "kdeconnect_sms").await;
                                let has_clipboard = check_plugin(&conn, &path, "kdeconnect_clipboard").await;
                                let has_contacts = check_plugin(&conn, &path, "kdeconnect_contacts").await;
                                let has_mpris = check_plugin(&conn, &path, "kdeconnect_mpriscontrol").await;
                                let has_remote_keyboard = check_plugin(&conn, &path, "kdeconnect_remotekeyboard").await;
                                let has_notifications = check_plugin(&conn, &path, "kdeconnect_notifications").await;
                                let has_sftp = check_plugin(&conn, &path, "kdeconnect_sftp").await;
                                let has_presenter = check_plugin(&conn, &path, "kdeconnect_presenter").await;
                                let has_lockdevice = check_plugin(&conn, &path, "kdeconnect_lockdevice").await;
                                let has_virtualmonitor = check_plugin(&conn, &path, "kdeconnect_virtualmonitor").await;
                                
                                let (battery_level, is_charging) = if has_battery {
                                    let level = get_device_property_int(&conn, &path, "charge").await.ok();
                                    let charging = get_device_property_bool(&conn, &path, "isCharging").await.ok();
                                    (level, charging)
                                } else {
                                    (None, None)
                                };

                                // Check BOTH pairing directions:
                                // - isPairRequestedByPeer: Phone is requesting to pair with PC
                                // - isPairRequested: PC is requesting to pair with Phone
                                let pairing_requested_by_peer = get_device_property_bool(&conn, &path, "isPairRequestedByPeer").await.unwrap_or(false);
                                let pairing_requested_by_us = get_device_property_bool(&conn, &path, "isPairRequested").await.unwrap_or(false);
                                
                                // Show Accept/Reject buttons if EITHER direction has a pending request
                                let pairing_requested = pairing_requested_by_peer || pairing_requested_by_us;
                                let pairing_requests = if pairing_requested { 1 } else { 0 };
                                
                                if pairing_requested {
                                    eprintln!("=== Pairing Request Detected ===");
                                    eprintln!("Device: {} ({})", name, device_id);
                                    if pairing_requested_by_peer {
                                        eprintln!("Direction: Phone ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â€šÂ¬Ã‚Â ÃƒÂ¢Ã¢â€šÂ¬Ã¢â€žÂ¢ PC (isPairRequestedByPeer: true)");
                                    }
                                    if pairing_requested_by_us {
                                        eprintln!("Direction: PC ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â€šÂ¬Ã‚Â ÃƒÂ¢Ã¢â€šÂ¬Ã¢â€žÂ¢ Phone (isPairRequested: true)");
                                    }
                                }

                                devices.push(Device {
                                    id: device_id,
                                    name,
                                    device_type,
                                    is_reachable,
                                    is_paired,
                                    is_trusted,
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
                                    has_notifications,
                                    has_sftp,
                                    has_presenter,
                                    has_lockdevice,
                                    has_virtualmonitor,
                                    pairing_requests,
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

async fn get_device_property(conn: &Connection, path: &str, property: &str) -> Result<String, Box<dyn std::error::Error>> {
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

async fn get_device_property_bool(conn: &Connection, path: &str, property: &str) -> Result<bool, Box<dyn std::error::Error>> {
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

async fn get_device_property_int(conn: &Connection, path: &str, property: &str) -> Result<i32, Box<dyn std::error::Error>> {
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

async fn pair_device(device_id: String) {
    eprintln!("=== Requesting Pairing ===");
    eprintln!("Device: {}", device_id);
    
    if let Ok(conn) = Connection::session().await {
        let path = format!("/modules/kdeconnect/devices/{}", device_id);
        
        let result = conn.call_method(
            Some("org.kde.kdeconnect"),
            path.as_str(),
            Some("org.kde.kdeconnect.device"),
            "requestPair",
            &()
        ).await;
        
        match result {
            Ok(_) => eprintln!("ÃƒÆ’Ã‚Â¢Ãƒâ€¦Ã¢â‚¬Å“ÃƒÂ¢Ã¢â€šÂ¬Ã…â€œ Pairing request sent successfully"),
            Err(e) => eprintln!("ÃƒÆ’Ã‚Â¢Ãƒâ€¦Ã¢â‚¬Å“ÃƒÂ¢Ã¢â€šÂ¬Ã¢â‚¬Â Failed to send pairing request: {:?}", e),
        }
    } else {
        eprintln!("ÃƒÆ’Ã‚Â¢Ãƒâ€¦Ã¢â‚¬Å“ÃƒÂ¢Ã¢â€šÂ¬Ã¢â‚¬Â Failed to connect to D-Bus");
    }
}

async fn unpair_device(device_id: String) {
    eprintln!("=== Unpairing Device ===");
    eprintln!("Device: {}", device_id);
    
    if let Ok(conn) = Connection::session().await {
        let path = format!("/modules/kdeconnect/devices/{}", device_id);
        
        let result = conn.call_method(
            Some("org.kde.kdeconnect"),
            path.as_str(),
            Some("org.kde.kdeconnect.device"),
            "unpair",
            &()
        ).await;
        
        match result {
            Ok(_) => eprintln!("ÃƒÆ’Ã‚Â¢Ãƒâ€¦Ã¢â‚¬Å“ÃƒÂ¢Ã¢â€šÂ¬Ã…â€œ Device unpaired successfully"),
            Err(e) => eprintln!("ÃƒÆ’Ã‚Â¢Ãƒâ€¦Ã¢â‚¬Å“ÃƒÂ¢Ã¢â€šÂ¬Ã¢â‚¬Â Failed to unpair device: {:?}", e),
        }
    }
}

async fn accept_pairing(device_id: String) {
    eprintln!("=== Accepting Pairing ===");
    eprintln!("Device: {}", device_id);
    
    if let Ok(conn) = Connection::session().await {
        let path = format!("/modules/kdeconnect/devices/{}", device_id);
        
        let result = conn.call_method(
            Some("org.kde.kdeconnect"),
            path.as_str(),
            Some("org.kde.kdeconnect.device"),
            "acceptPairing",
            &()
        ).await;
        
        match result {
            Ok(_) => eprintln!("ÃƒÆ’Ã‚Â¢Ãƒâ€¦Ã¢â‚¬Å“ÃƒÂ¢Ã¢â€šÂ¬Ã…â€œ Pairing accepted successfully"),
            Err(e) => eprintln!("ÃƒÆ’Ã‚Â¢Ãƒâ€¦Ã¢â‚¬Å“ÃƒÂ¢Ã¢â€šÂ¬Ã¢â‚¬Â Failed to accept pairing: {:?}", e),
        }
    }
}

async fn reject_pairing(device_id: String) {
    eprintln!("=== Rejecting Pairing ===");
    eprintln!("Device: {}", device_id);
    
    if let Ok(conn) = Connection::session().await {
        let path = format!("/modules/kdeconnect/devices/{}", device_id);
        
        let result = conn.call_method(
            Some("org.kde.kdeconnect"),
            path.as_str(),
            Some("org.kde.kdeconnect.device"),
            "rejectPairing",
            &()
        ).await;
        
        match result {
            Ok(_) => eprintln!("ÃƒÆ’Ã‚Â¢Ãƒâ€¦Ã¢â‚¬Å“ÃƒÂ¢Ã¢â€šÂ¬Ã…â€œ Pairing rejected successfully"),
            Err(e) => eprintln!("ÃƒÆ’Ã‚Â¢Ãƒâ€¦Ã¢â‚¬Å“ÃƒÂ¢Ã¢â€šÂ¬Ã¢â‚¬Â Failed to reject pairing: {:?}", e),
        }
    }
}

async fn ping_device(device_id: String) {
    if let Ok(conn) = Connection::session().await {
        let path = format!("/modules/kdeconnect/devices/{}", device_id);
        let _ = conn.call_method(
            Some("org.kde.kdeconnect"),
            path.as_str(),
            Some("org.kde.kdeconnect.device.ping"),
            "sendPing",
            &()
        ).await;
    }
}

async fn ring_device(device_id: String) {
    if let Ok(conn) = Connection::session().await {
        let path = format!("/modules/kdeconnect/devices/{}", device_id);
        let _ = conn.call_method(
            Some("org.kde.kdeconnect"),
            path.as_str(),
            Some("org.kde.kdeconnect.device.findmyphone"),
            "ring",
            &()
        ).await;
    }
}

/// Maps a PermissionType to its corresponding KDE Connect plugin name.
fn permission_to_plugin_name(perm: &PermissionType) -> String {
    match perm {
        PermissionType::SMS => "kdeconnect_sms",
        PermissionType::Share => "kdeconnect_share",           // File sharing (send/receive)
        PermissionType::Sftp => "kdeconnect_sftp",             // Remote filesystem browser
        PermissionType::MediaPlayer => "kdeconnect_mpriscontrol",
        PermissionType::VolumeControl => "kdeconnect_systemvolume",
        PermissionType::ConnectivityReport => "kdeconnect_connectivity_report",
        PermissionType::RemoteKeypresses => "kdeconnect_remotekeyboard",
        PermissionType::Notifications => "kdeconnect_notifications",
        PermissionType::PauseMediaCalls => "kdeconnect_pausemusic",
        PermissionType::ContactsSync => "kdeconnect_contacts",
        PermissionType::Clipboard => "kdeconnect_clipboard",
        PermissionType::Battery => "kdeconnect_battery",
        PermissionType::Ping => "kdeconnect_ping",
        PermissionType::FindMyPhone => "kdeconnect_findmyphone",
        PermissionType::Presenter => "kdeconnect_presenter",
        PermissionType::Photo => "kdeconnect_photo",
        PermissionType::RunCommand => "kdeconnect_runcommand",
        PermissionType::LockDevice => "kdeconnect_lockdevice",
        PermissionType::Telephony => "kdeconnect_telephony",
        PermissionType::MprisRemote => "kdeconnect_mprisremote",
        PermissionType::MultimediaReceiver => "kdeconnect_mpriscontrol",
        PermissionType::ScreensaverInhibit => "kdeconnect_screensaver_inhibit",
        PermissionType::VirtualMonitor => "kdeconnect_virtualmonitor",
        PermissionType::Bigscreen => "kdeconnect_bigscreen",
        PermissionType::Mousepad => "kdeconnect_mousepad",
        PermissionType::RemoteControl => "kdeconnect_remotecontrol",
        PermissionType::SendNotifications => "kdeconnect_sendnotifications",
    }.to_string()
}

/// Gets the current state of a permission from DevicePermissions.
fn get_permission_state(permissions: &DevicePermissions, perm: &PermissionType) -> bool {
    match perm {
        PermissionType::SMS => permissions.sms,
        PermissionType::Share => permissions.share,
        PermissionType::Sftp => permissions.sftp,
        PermissionType::MediaPlayer => permissions.media_player,
        PermissionType::VolumeControl => permissions.volume_control,
        PermissionType::ConnectivityReport => permissions.connectivity_report,
        PermissionType::RemoteKeypresses => permissions.remote_keypresses,
        PermissionType::Notifications => permissions.notifications,
        PermissionType::PauseMediaCalls => permissions.pause_media_calls,
        PermissionType::ContactsSync => permissions.contacts_sync,
        PermissionType::Clipboard => permissions.clipboard,
        PermissionType::Battery => permissions.battery,
        PermissionType::Ping => permissions.ping,
        PermissionType::FindMyPhone => permissions.findmyphone,
        PermissionType::Presenter => permissions.presenter,
        PermissionType::Photo => permissions.photo,
        PermissionType::RunCommand => permissions.runcommand,
        PermissionType::LockDevice => permissions.lockdevice,
        PermissionType::Telephony => permissions.telephony,
        PermissionType::MprisRemote => permissions.mpris_remote,
        PermissionType::MultimediaReceiver => permissions.multimedia_receiver,
        PermissionType::ScreensaverInhibit => permissions.screensaver_inhibit,
        PermissionType::VirtualMonitor => permissions.virtualmonitor,
        PermissionType::Bigscreen => permissions.bigscreen,
        PermissionType::Mousepad => permissions.mousepad,
        PermissionType::RemoteControl => permissions.remotecontrol,
        PermissionType::SendNotifications => permissions.sendnotifications,
    }
}

/// Internal function to set plugin enabled state via D-Bus.
async fn set_plugin_enabled_internal(device_id: String, plugin_name: String, enabled: bool) -> Result<(), String> {
    eprintln!("=== Setting Plugin State ===");
    eprintln!("Device: {}", device_id);
    eprintln!("Plugin: {}", plugin_name);
    eprintln!("Enabled: {}", enabled);
    
    let conn = Connection::session().await
        .map_err(|e| format!("D-Bus connection failed: {:?}", e))?;
    
    let path = format!("/modules/kdeconnect/devices/{}", device_id);
    
    let result = conn.call_method(
        Some("org.kde.kdeconnect"),
        path.as_str(),
        Some("org.kde.kdeconnect.device"),
        "setPluginEnabled",
        &(plugin_name.as_str(), enabled)
    ).await;
    
    match result {
        Ok(_) => {
            eprintln!("ÃƒÆ’Ã‚Â¢Ãƒâ€¦Ã¢â‚¬Å“ÃƒÂ¢Ã¢â€šÂ¬Ã…â€œ Plugin {} {} successfully", 
                plugin_name, 
                if enabled { "enabled" } else { "disabled" }
            );
            Ok(())
        }
        Err(e) => {
            let err_msg = format!("Failed to set plugin state: {:?}", e);
            eprintln!("ÃƒÆ’Ã‚Â¢Ãƒâ€¦Ã¢â‚¬Å“ÃƒÂ¢Ã¢â€šÂ¬Ã¢â‚¬Â {}", err_msg);
            Err(err_msg)
        }
    }
}

/// Loads the actual plugin states for a device from KDE Connect.
async fn load_device_permissions(device_id: String) -> DevicePermissions {
    eprintln!("=== Loading Device Permissions ===");
    eprintln!("Device: {}", device_id);
    
    let conn = match Connection::session().await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("ÃƒÆ’Ã‚Â¢Ãƒâ€¦Ã¢â‚¬Å“ÃƒÂ¢Ã¢â€šÂ¬Ã¢â‚¬Â D-Bus connection failed: {:?}", e);
            return DevicePermissions::default();
        }
    };
    
    let path = format!("/modules/kdeconnect/devices/{}", device_id);
    
    // Helper function to check if a plugin is enabled
    async fn check_plugin_enabled(conn: &Connection, path: &str, plugin_name: &str) -> bool {
        match conn.call_method(
            Some("org.kde.kdeconnect"),
            path,
            Some("org.kde.kdeconnect.device"),
            "isPluginEnabled",
            &(plugin_name,)
        ).await {
            Ok(reply) => {
                let body = reply.body();
                body.deserialize::<bool>().unwrap_or(false)
            }
            Err(e) => {
                eprintln!("ÃƒÆ’Ã‚Â¢Ãƒâ€¦Ã¢â‚¬Å“ÃƒÂ¢Ã¢â€šÂ¬Ã¢â‚¬Â Failed to check plugin {}: {:?}", plugin_name, e);
                false
            }
        }
    }
    
    DevicePermissions {
        sms: check_plugin_enabled(&conn, &path, "kdeconnect_sms").await,
        share: check_plugin_enabled(&conn, &path, "kdeconnect_share").await,        // File sharing
        sftp: check_plugin_enabled(&conn, &path, "kdeconnect_sftp").await,          // Remote filesystem
        media_player: check_plugin_enabled(&conn, &path, "kdeconnect_mpriscontrol").await,
        volume_control: check_plugin_enabled(&conn, &path, "kdeconnect_systemvolume").await,
        connectivity_report: check_plugin_enabled(&conn, &path, "kdeconnect_connectivity_report").await,
        remote_keypresses: check_plugin_enabled(&conn, &path, "kdeconnect_remotekeyboard").await,
        notifications: check_plugin_enabled(&conn, &path, "kdeconnect_notifications").await,
        pause_media_calls: check_plugin_enabled(&conn, &path, "kdeconnect_pausemusic").await,
        contacts_sync: check_plugin_enabled(&conn, &path, "kdeconnect_contacts").await,
        clipboard: check_plugin_enabled(&conn, &path, "kdeconnect_clipboard").await,
        
        battery: check_plugin_enabled(&conn, &path, "kdeconnect_battery").await,
        ping: check_plugin_enabled(&conn, &path, "kdeconnect_ping").await,
        findmyphone: check_plugin_enabled(&conn, &path, "kdeconnect_findmyphone").await,
        presenter: check_plugin_enabled(&conn, &path, "kdeconnect_presenter").await,
        photo: check_plugin_enabled(&conn, &path, "kdeconnect_photo").await,
        runcommand: check_plugin_enabled(&conn, &path, "kdeconnect_runcommand").await,
        lockdevice: check_plugin_enabled(&conn, &path, "kdeconnect_lockdevice").await,
        telephony: check_plugin_enabled(&conn, &path, "kdeconnect_telephony").await,
        mpris_remote: check_plugin_enabled(&conn, &path, "kdeconnect_mprisremote").await,
        multimedia_receiver: check_plugin_enabled(&conn, &path, "kdeconnect_mpriscontrol").await,
        screensaver_inhibit: check_plugin_enabled(&conn, &path, "kdeconnect_screensaver_inhibit").await,
        virtualmonitor: check_plugin_enabled(&conn, &path, "kdeconnect_virtualmonitor").await,
        bigscreen: check_plugin_enabled(&conn, &path, "kdeconnect_bigscreen").await,
        mousepad: check_plugin_enabled(&conn, &path, "kdeconnect_mousepad").await,
        remotecontrol: check_plugin_enabled(&conn, &path, "kdeconnect_remotecontrol").await,
        sendnotifications: check_plugin_enabled(&conn, &path, "kdeconnect_sendnotifications").await,
    }
}

fn main() -> cosmic::iced::Result {
    // Parse command line arguments for kdeconnect:// URL handling
    let args: Vec<String> = std::env::args().collect();
    
    let url = if args.len() > 1 {
        let arg = &args[1];
        if arg.starts_with("kdeconnect://") {
            eprintln!("=== Launched with URL ===");
            eprintln!("URL: {}", arg);
            
            if arg.starts_with("kdeconnect://pair/") {
                let device_id = arg.strip_prefix("kdeconnect://pair/").unwrap_or("");
                eprintln!("Pairing request for device: {}", device_id);
            }
            
            Some(arg.clone())
        } else {
            None
        }
    } else {
        None
    };
    
    let settings = cosmic::app::Settings::default()
        .size_limits(cosmic::iced::Limits::NONE.min_width(700.0).min_height(500.0))
        .size(cosmic::iced::Size::new(900.0, 600.0));
    
    cosmic::app::run::<KdeConnectSettings>(settings, url)
}