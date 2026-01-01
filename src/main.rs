// src/main.rs
mod models;
mod messages;
mod dbus;
mod ui;
mod plugins;
mod notifications;

use cosmic::app::Core;
use cosmic::iced::{window, Limits, Subscription};
use cosmic::iced::window::Id as SurfaceId;
use cosmic::iced::Task as Command;
use cosmic::{Element, Action};
use cosmic::widget;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

use models::Device;
use messages::Message;

const ICON_PHONE: &str = "phone-symbolic";

// NEW: Static receiver for pairing notifications - created once at startup
lazy_static::lazy_static! {
    static ref PAIRING_RECEIVER: Arc<Mutex<Option<tokio::sync::mpsc::Receiver<notifications::PairingNotification>>>> = 
        Arc::new(Mutex::new(None));
}

pub struct KdeConnectApplet {
    core: Core,
    devices: HashMap<String, Device>,
    popup: Option<window::Id>,
    expanded_device: Option<String>,
    expanded_player_menu: Option<String>,
}

impl cosmic::Application for KdeConnectApplet {
    type Executor = cosmic::executor::Default;
    type Flags = ();
    type Message = Message;
    const APP_ID: &str = "io.github.M4LC0ntent.CosmicKdeConnect";

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(
        core: Core,
        _flags: Self::Flags,
    ) -> (Self, Command<Action<Self::Message>>) {
        // NEW: Initialize notification listener ONCE in init
        tokio::spawn(async {
            let mut receiver_guard = PAIRING_RECEIVER.lock().await;
            
            if receiver_guard.is_none() {
                eprintln!("=== Initializing notification listener (ONCE) ===");
                
                let (tx, rx) = tokio::sync::mpsc::channel(100);
                *receiver_guard = Some(rx);
                drop(receiver_guard);
                
                // Start the listener
                notifications::start_notification_listener(tx, false);
            }
        });
        
        let applet = KdeConnectApplet {
            core,
            devices: HashMap::new(),
            popup: None,
            expanded_device: None,
            expanded_player_menu: None,
        };

        (applet, Command::perform(dbus::fetch_devices(), |devices| {
            Action::App(Message::DevicesUpdated(devices))
        }))
    }

    fn on_close_requested(&self, _id: SurfaceId) -> Option<Message> {
        Some(Message::TogglePopup)
    }

    fn update(
        &mut self,
        message: Self::Message,
    ) -> Command<Action<Self::Message>> {
        match message {
            Message::TogglePopup => {
                if let Some(popup_id) = self.popup.take() {
                    self.expanded_device = None;
                    return cosmic::iced::platform_specific::shell::commands::popup::destroy_popup(popup_id);
                }
                
                let new_id = window::Id::unique();
                self.popup = Some(new_id);
                
                let mut popup_settings = self.core.applet.get_popup_settings(
                    self.core.main_window_id().unwrap(),
                    new_id,
                    None,
                    None,
                    None,
                );
                
                popup_settings.positioner.size_limits = Limits::NONE
                    .min_width(400.0)
                    .max_width(400.0)
                    .min_height(200.0)
                    .max_height(700.0);
                
                let mpris_devices: Vec<String> = self.devices.values()
                    .filter(|d| d.has_mpris && d.is_reachable && d.is_paired)
                    .map(|d| d.id.clone())
                    .collect();
                
                let mut commands = vec![
                    cosmic::iced::platform_specific::shell::commands::popup::get_popup(popup_settings),
                    Command::perform(dbus::fetch_devices(), |devices| {
                        Action::App(Message::DevicesUpdated(devices))
                    })
                ];
                
                for device_id in mpris_devices {
                    let id_for_players = device_id.clone();
                    let id_for_info = device_id.clone();
                    
                    commands.push(Command::perform(
                        async move {
                            (id_for_players.clone(), dbus::get_media_player_list(id_for_players).await)
                        },
                        |(device_id, players)| Action::App(Message::MediaPlayersUpdated(device_id, players))
                    ));
                    
                    commands.push(Command::perform(
                        async move {
                            (id_for_info.clone(), dbus::get_media_player_info(id_for_info).await)
                        },
                        |(device_id, info)| Action::App(Message::MediaInfoUpdated(device_id, info))
                    ));
                }
                
                return Command::batch(commands);
            }
            Message::RefreshDevices => {
                return Command::perform(dbus::fetch_devices(), |devices| {
                    Action::App(Message::DevicesUpdated(devices))
                });
            }
            Message::DevicesUpdated(devices) => {
                let old_devices = std::mem::take(&mut self.devices);
                
                for mut device in devices {
                    if let Some(old_device) = old_devices.get(&device.id) {
                        device.available_players = old_device.available_players.clone();
                        device.current_player = old_device.current_player.clone();
                        device.media_info = old_device.media_info.clone();
                    }
                    self.devices.insert(device.id.clone(), device);
                }
            }
            Message::ToggleDeviceMenu(ref device_id) => {
                let should_load_media = if self.expanded_device.as_ref() == Some(device_id) {
                    self.expanded_device = None;
                    false
                } else {
                    self.expanded_device = Some(device_id.clone());
                    self.devices.get(device_id).map(|d| d.has_mpris).unwrap_or(false)
                };
                
                if should_load_media {
                    let id = device_id.clone();
                    return Command::perform(
                        async move {
                            Message::RefreshMediaPlayers(id)
                        },
                        |msg| Action::App(msg)
                    );
                }
            }
            Message::TogglePlayerMenu(ref device_id) => {
                if self.expanded_player_menu.as_ref() == Some(device_id) {
                    self.expanded_player_menu = None;
                } else {
                    self.expanded_player_menu = Some(device_id.clone());
                }
            }
            Message::PingDevice(ref device_id) => {
                let id = device_id.clone();
                return Command::perform(
                    async move {
                        dbus::ping_device(id).await;
                    },
                    |_| Action::App(Message::RefreshDevices)
                );
            }
            Message::RingDevice(ref device_id) => {
                let id = device_id.clone();
                return Command::perform(
                    async move {
                        dbus::ring_device(id).await;
                    },
                    |_| Action::App(Message::RefreshDevices)
                );
            }
            Message::LockDevice(ref device_id) => {
                let id = device_id.clone();
                return Command::perform(
                    async move {
                        dbus::lock_device(id).await;
                    },
                    |_| Action::App(Message::RefreshDevices)
                );
            }
            #[allow(unused_variables)]
            Message::BrowseDevice(ref device_id) => {
                let id = device_id.clone();
                return Command::perform(
                    async move {
                        dbus::browse_files(id).await;
                    },
                    |_| Action::App(Message::RefreshDevices)
                );
            }
            Message::SendFile(ref device_id) => {
                let id = device_id.clone();
                return Command::perform(
                    async move {
                        // Use xdg-desktop-portal for native COSMIC integration
                        let files = cosmic_connect_applet::portal::pick_files(
                            "Select files to send",
                            true,  // Allow multiple selection
                            None,  // No file filters
                        ).await;
                        
                        if !files.is_empty() {
                            dbus::share_files(id, files).await;
                        }
                    },
                    |_| Action::App(Message::RefreshDevices)
                );
            }
            Message::ShareClipboard(ref device_id) => {
                let id = device_id.clone();
                return Command::perform(
                    async move {
                        match std::process::Command::new("wl-paste")
                            .output()
                        {
                            Ok(output) if output.status.success() => {
                                if let Ok(content) = String::from_utf8(output.stdout) {
                                    dbus::send_clipboard(id, content).await;
                                }
                            }
                            _ => eprintln!("Failed to get clipboard content"),
                        }
                    },
                    |_| Action::App(Message::RefreshDevices)
                );
            }
            Message::PairDevice(ref device_id) => {
                let id = device_id.clone();
                return Command::perform(
                    async move {
                        dbus::pair_device(id).await;
                    },
                    |_| Action::App(Message::RefreshDevices)
                );
            }
            Message::UnpairDevice(ref device_id) => {
                let id = device_id.clone();
                return Command::perform(
                    async move {
                        dbus::unpair_device(id).await;
                    },
                    |_| Action::App(Message::RefreshDevices)
                );
            }
            Message::AcceptPairing(ref device_id) => {
                let id = device_id.clone();
                return Command::perform(
                    async move {
                        dbus::accept_pairing(id).await;
                    },
                    |_| Action::App(Message::RefreshDevices)
                );
            }
            Message::RejectPairing(ref device_id) => {
                let id = device_id.clone();
                return Command::perform(
                    async move {
                        dbus::reject_pairing(id).await;
                    },
                    |_| Action::App(Message::RefreshDevices)
                );
            }
            Message::SendSMS(ref device_id) => {
                let device_name = self.devices.get(device_id)
                    .map(|d| d.name.clone())
                    .unwrap_or_else(|| "Unknown Device".to_string());
                
                let id = device_id.clone();
                let name = device_name;
                
                std::thread::spawn(move || {
                    let _ = std::process::Command::new("cosmic-connect-sms")
                        .arg(&id)
                        .arg(&name)
                        .spawn();
                });
            }
            Message::MediaPlay(ref device_id) => {
                let id = device_id.clone();
                let id_for_refresh = device_id.clone();
                return Command::batch(vec![
                    Command::perform(
                        async move {
                            dbus::play_media(id).await;
                        },
                        |_| Action::App(Message::RefreshDevices)
                    ),
                    Command::perform(
                        async move {
                            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                            let info = dbus::get_media_player_info(id_for_refresh.clone()).await;
                            (id_for_refresh, info)
                        },
                        |(device_id, info)| Action::App(Message::MediaInfoUpdated(device_id, info))
                    )
                ]);
            }
            Message::MediaPause(ref device_id) => {
                let id = device_id.clone();
                let id_for_refresh = device_id.clone();
                return Command::batch(vec![
                    Command::perform(
                        async move {
                            dbus::pause_media(id).await;
                        },
                        |_| Action::App(Message::RefreshDevices)
                    ),
                    Command::perform(
                        async move {
                            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                            let info = dbus::get_media_player_info(id_for_refresh.clone()).await;
                            (id_for_refresh, info)
                        },
                        |(device_id, info)| Action::App(Message::MediaInfoUpdated(device_id, info))
                    )
                ]);
            }
            Message::MediaNext(ref device_id) => {
                let id = device_id.clone();
                let id_for_refresh = device_id.clone();
                return Command::batch(vec![
                    Command::perform(
                        async move {
                            dbus::media_next(id).await;
                        },
                        |_| Action::App(Message::RefreshDevices)
                    ),
                    Command::perform(
                        async move {
                            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                            let info = dbus::get_media_player_info(id_for_refresh.clone()).await;
                            (id_for_refresh, info)
                        },
                        |(device_id, info)| Action::App(Message::MediaInfoUpdated(device_id, info))
                    )
                ]);
            }
            Message::MediaPrevious(ref device_id) => {
                let id = device_id.clone();
                let id_for_refresh = device_id.clone();
                return Command::batch(vec![
                    Command::perform(
                        async move {
                            dbus::media_previous(id).await;
                        },
                        |_| Action::App(Message::RefreshDevices)
                    ),
                    Command::perform(
                        async move {
                            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                            let info = dbus::get_media_player_info(id_for_refresh.clone()).await;
                            (id_for_refresh, info)
                        },
                        |(device_id, info)| Action::App(Message::MediaInfoUpdated(device_id, info))
                    )
                ]);
            }
            Message::VolumeUp(ref device_id) => {
                let id = device_id.clone();
                return Command::perform(
                    async move {
                        if let Some(current_vol) = dbus::get_media_volume(id.clone()).await {
                            let new_vol = (current_vol + 10).min(100);
                            dbus::set_media_volume(id.clone(), new_vol).await;
                        }
                    },
                    |_| Action::App(Message::RefreshDevices)
                );
            }
            Message::VolumeDown(ref device_id) => {
                let id = device_id.clone();
                return Command::perform(
                    async move {
                        if let Some(current_vol) = dbus::get_media_volume(id.clone()).await {
                            let new_vol = (current_vol - 10).max(0);
                            dbus::set_media_volume(id.clone(), new_vol).await;
                        }
                    },
                    |_| Action::App(Message::RefreshDevices)
                );
            }
            Message::VolumeChanged(ref device_id, volume) => {
                let id = device_id.clone();
                let id_for_refresh = device_id.clone();
                return Command::batch(vec![
                    Command::perform(
                        async move {
                            dbus::set_media_volume(id, volume).await;
                        },
                        |_| Action::App(Message::RefreshDevices)
                    ),
                    Command::perform(
                        async move {
                            tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
                            let info = dbus::get_media_player_info(id_for_refresh.clone()).await;
                            (id_for_refresh, info)
                        },
                        |(device_id, info)| Action::App(Message::MediaInfoUpdated(device_id, info))
                    )
                ]);
            }
            Message::ShareUrl(ref device_id, ref url) => {
                let id = device_id.clone();
                let url_clone = url.clone();
                return Command::perform(
                    async move {
                        dbus::share_files(id, vec![url_clone]).await;
                    },
                    |_| Action::App(Message::RefreshDevices)
                );
            }
            Message::MediaPlayerSelected(ref device_id, ref player_name) => {
                let id = device_id.clone();
                let player = player_name.clone();
                let id_for_refresh = device_id.clone();
                return Command::batch(vec![
                    Command::perform(
                        async move {
                            dbus::set_media_player(id, player).await;
                        },
                        |_| Action::App(Message::RefreshDevices)
                    ),
                    Command::perform(
                        async move {
                            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                            let info = dbus::get_media_player_info(id_for_refresh.clone()).await;
                            (id_for_refresh, info)
                        },
                        |(device_id, info)| Action::App(Message::MediaInfoUpdated(device_id, info))
                    )
                ]);
            }
            Message::MediaInfoUpdated(ref device_id, ref info) => {
                if let Some(device) = self.devices.get_mut(device_id) {
                    device.media_info = info.clone();
                    if let Some(info) = info {
                        if !info.player.is_empty() {
                            device.current_player = Some(info.player.clone());
                        }
                    }
                }
            }
            Message::RequestMediaInfo(ref device_id) => {
                let id = device_id.clone();
                return Command::perform(
                    async move {
                        let info = dbus::get_media_player_info(id.clone()).await;
                        (id, info)
                    },
                    |(device_id, info)| Action::App(Message::MediaInfoUpdated(device_id, info))
                );
            }
            Message::RefreshMediaPlayers(ref device_id) => {
                let id = device_id.clone();
                let id_for_info = device_id.clone();
                return Command::batch(vec![
                    Command::perform(
                        async move {
                            let players = dbus::get_media_player_list(id.clone()).await;
                            (id, players)
                        },
                        |(device_id, players)| Action::App(Message::MediaPlayersUpdated(device_id, players))
                    ),
                    Command::perform(
                        async move {
                            let info = dbus::get_media_player_info(id_for_info.clone()).await;
                            (id_for_info, info)
                        },
                        |(device_id, info)| Action::App(Message::MediaInfoUpdated(device_id, info))
                    )
                ]);
            }
            Message::MediaPlayersUpdated(ref device_id, ref players) => {
                if let Some(device) = self.devices.get_mut(device_id) {
                    device.available_players = players.clone();
                }
            }
            Message::OpenSettings => {
                let _ = std::process::Command::new("cosmic-connect-settings")
                    .spawn();
            }
            Message::RemoteInput(ref device_id) => {
                eprintln!("Remote input requested for device: {}", device_id);
            }
            Message::PresenterMode(ref device_id) => {
                eprintln!("Presenter mode requested for device: {}", device_id);
            }
            Message::UseAsMonitor(ref device_id) => {
                eprintln!("Use as monitor requested for device: {}", device_id);
            }
            Message::PairingRequestReceived(device_id, device_name, device_type) => {
                eprintln!("=== Pairing Request in Main App ===");
                eprintln!("Device: {} ({})", device_name, device_id);
                eprintln!("Type: {}", device_type);
                
                tokio::spawn(async move {
                    if let Err(e) = notifications::show_pairing_notification(&device_name, &device_id).await {
                        eprintln!("Failed to show notification: {}", e);
                    }
                });
                
                return Command::perform(dbus::fetch_devices(), |devices| {
                    Action::App(Message::DevicesUpdated(devices))
                });
            }
        }
        Command::none()
    }

    fn view(&self) -> Element<'_, Self::Message> {
        self.core
            .applet
            .icon_button(ICON_PHONE)
            .on_press_down(Message::TogglePopup)
            .into()
    }

    fn view_window(&self, id: SurfaceId) -> Element<'_, Self::Message> {
        if !matches!(self.popup, Some(popup_id) if popup_id == id) {
            return widget::text("").into();
        }
        
        ui::popup::create_popup_view(&self.devices, self.expanded_device.as_ref(), self.expanded_player_menu.as_ref())
    }
    
    fn style(&self) -> Option<cosmic::iced_runtime::Appearance> {
        Some(cosmic::applet::style())
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        // Use unfold to read from static receiver created in init()
        let pairing_sub = Subscription::run_with_id(
            "pairing-notifications",
            futures::stream::unfold((), |_| async {
                // Lock the static receiver
                let mut receiver_guard = PAIRING_RECEIVER.lock().await;
                
                if let Some(rx) = receiver_guard.as_mut() {
                    // Try to receive a notification
                    if let Some(notification) = rx.recv().await {
                        return Some((notification, ()));
                    }
                }
                
                // Receiver not ready or closed, wait and retry
                drop(receiver_guard);
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                None
            })
        ).map(|notification| Message::PairingRequestReceived(
            notification.device_id,
            notification.device_name,
            notification.device_type,
        ));
        
        Subscription::batch(vec![
            cosmic::iced::time::every(std::time::Duration::from_secs(5))
                .map(|_| Message::RefreshDevices),
            pairing_sub,
        ])
    }
}

impl Drop for KdeConnectApplet {
    fn drop(&mut self) {
        eprintln!("=== KdeConnectApplet Drop called ===");
        eprintln!("Performing D-Bus cleanup...");
        
        if let Ok(rt) = tokio::runtime::Runtime::new() {
            rt.block_on(async {
                dbus::cleanup().await;
            });
        }
        
        eprintln!("D-Bus cleanup complete");
    }
}

fn main() -> cosmic::iced::Result {
    eprintln!("=== KDE Connect Applet Starting ===");
    
    // Setup signal handler
    ctrlc::set_handler(move || {
        eprintln!("=== Shutdown signal received (SIGTERM/SIGINT) ===");
        eprintln!("Cleaning up before exit...");
        
        if let Ok(rt) = tokio::runtime::Runtime::new() {
            rt.block_on(async {
                dbus::cleanup().await;
            });
        }
        
        eprintln!("Cleanup complete, exiting");
        std::process::exit(0);
    })
    .ok();
    
    cosmic::applet::run::<KdeConnectApplet>(())
}