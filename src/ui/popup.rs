// src/ui/popup.rs
use cosmic::iced::{Alignment, Length};
use cosmic::{widget, Element};
use std::collections::HashMap;
use crate::{models::Device, messages::Message};

pub fn create_popup_view<'a>(devices: &'a HashMap<String, Device>, expanded_device: Option<&'a String>, expanded_player_menu: Option<&'a String>) -> Element<'a, Message> {
    let spacing = cosmic::theme::active().cosmic().spacing;
    let mut content = widget::column().spacing(spacing.space_s).padding(spacing.space_s);

    // Header
    content = content.push(
        widget::row()
            .push(widget::text("Cosmic Connect").size(18).width(Length::Fill))
            .push(
                widget::button::standard("Settings")
                    .on_press(Message::OpenSettings)
            )
            .spacing(spacing.space_xs)
            .align_y(Alignment::Center)
    );

    content = content.push(widget::divider::horizontal::default());

    // Pairing requests - SORTED alphabetically
    let mut pairing_requests: Vec<_> = devices.values()
        .filter(|d| d.pairing_requests > 0 && !d.is_paired)
        .collect();
    
    // Sort pairing requests by device name
    pairing_requests.sort_by(|a, b| a.name.cmp(&b.name));

    if !pairing_requests.is_empty() {
        content = content.push(widget::text("Pairing Requests").size(14).font(cosmic::font::bold()));
        
        for device in pairing_requests {
            let device_id_accept = device.id.clone();
            let device_id_reject = device.id.clone();
            
            let request_card = widget::container(
                widget::column()
                    .push(
                        widget::row()
                            .push(widget::icon::from_name(device.device_icon()).size(24))
                            .push(widget::text(&device.name).size(14).width(Length::Fill))
                            .spacing(spacing.space_xs)
                            .align_y(Alignment::Center)
                    )
                    .push(widget::text("wants to pair").size(12))
                    .push(
                        widget::row()
                            .push(widget::button::suggested("Accept").on_press(Message::AcceptPairing(device_id_accept)))
                            .push(widget::button::destructive("Deny").on_press(Message::RejectPairing(device_id_reject)))
                            .spacing(spacing.space_xs)
                    )
                    .spacing(spacing.space_xxs)
            )
            .padding(spacing.space_xs)
            .class(cosmic::theme::Container::Card)
            .width(Length::Fill);
            
            content = content.push(request_card);
        }
        
        content = content.push(widget::divider::horizontal::default());
    }

    // Connected devices - SORTED alphabetically
    let mut paired_devices: Vec<_> = devices.values()
        .filter(|d| d.is_paired && d.is_reachable)
        .collect();
    
    // Sort paired devices by name for stable ordering
    paired_devices.sort_by(|a, b| a.name.cmp(&b.name));

    if paired_devices.is_empty() {
        content = content.push(
            widget::container(widget::text("No devices connected").size(14))
                .padding(spacing.space_m)
                .width(Length::Fill)
                .center_x(Length::Fill)
        );
    } else {
        for device in paired_devices {
            content = content.push(create_device_card(device, &spacing, expanded_device, expanded_player_menu));
        }
    }

    let popup_content = widget::container(widget::scrollable(content))
        .width(Length::Fixed(400.0))
        .max_height(700.0)
        .padding(spacing.space_xs)
        .class(cosmic::theme::Container::Dialog);

    cosmic::app::Core::default()
        .applet
        .popup_container(popup_content)
        .into()
}

fn create_device_card<'a>(device: &'a Device, spacing: &cosmic::cosmic_theme::Spacing, expanded_device: Option<&'a String>, expanded_player_menu: Option<&'a String>) -> Element<'a, Message> {
    let is_expanded = expanded_device == Some(&device.id);
    let is_player_menu_expanded = expanded_player_menu == Some(&device.id);
    
    let mut info_col = widget::column().spacing(4);
    
    // Device name and icon with menu toggle
    let mut name_row = widget::row()
        .push(widget::icon::from_name(device.device_icon()).size(20))
        .push(widget::text(&device.name).size(14).width(Length::Fill))
        .spacing(spacing.space_xs)
        .align_y(Alignment::Center);

    // Add battery icon and percentage next to device name if available
    if let Some(level) = device.battery_level {
        let battery_icon = if let Some(charging) = device.is_charging {
            if charging {
                "battery-full-charging-symbolic"
            } else {
                device.battery_icon()
            }
        } else {
            device.battery_icon()
        };
        
        // Add signal strength icon before battery if available
        if let Some(signal_icon) = device.signal_icon() {
            name_row = name_row.push(widget::icon::from_name(signal_icon).size(16));
        }
        
        name_row = name_row.push(widget::icon::from_name(battery_icon).size(16));
        name_row = name_row.push(widget::text(format!("{}%", level)).size(12));
    } else {
        // No battery, but show signal strength if available
        if let Some(signal_icon) = device.signal_icon() {
            name_row = name_row.push(widget::icon::from_name(signal_icon).size(16));
        }
    }

    name_row = name_row.push(
        widget::button::icon(
            widget::icon::from_name(if is_expanded { "go-up-symbolic" } else { "go-down-symbolic" }).size(16)
        )
        .on_press(Message::ToggleDeviceMenu(device.id.clone()))
    );
    
    info_col = info_col.push(name_row);

    // Media controls - always visible if available
    if device.has_mpris {
        let mut media_section = widget::column().spacing(spacing.space_xxs);
        
        // Title and artist (if available)
        if let Some(ref media_info) = device.media_info {
            if !media_info.title.is_empty() || !media_info.artist.is_empty() {
                let title_text = if !media_info.title.is_empty() {
                    media_info.title.clone()
                } else {
                    "No title".to_string()
                };
                
                media_section = media_section.push(
                    widget::text(title_text)
                        .size(12)
                        .font(cosmic::font::semibold())
                );
                
                if !media_info.artist.is_empty() {
                    media_section = media_section.push(
                        widget::text(&media_info.artist).size(11)
                    );
                }
            }
        }
        
        // Media control buttons
        let play_icon = if let Some(ref info) = device.media_info {
            if info.is_playing {
                "media-playback-pause-symbolic"
            } else {
                "media-playback-start-symbolic"
            }
        } else {
            "media-playback-start-symbolic"
        };
        
        // Get current volume for the slider
        let current_volume = device.media_info.as_ref()
            .map(|info| info.volume)
            .unwrap_or(50) as f32;
        
        // Media playback controls row
        let playback_controls = widget::row()
            .push(widget::text("Media").size(11))
            .push(widget::horizontal_space())
            .push(
                widget::button::icon(widget::icon::from_name("media-skip-backward-symbolic").size(14))
                    .on_press(Message::MediaPrevious(device.id.clone()))
            )
            .push(
                widget::button::icon(widget::icon::from_name(play_icon).size(14))
                    .on_press(Message::MediaPlay(device.id.clone()))
            )
            .push(
                widget::button::icon(widget::icon::from_name("media-skip-forward-symbolic").size(14))
                    .on_press(Message::MediaNext(device.id.clone()))
            )
            .push(
                widget::button::icon(widget::icon::from_name("view-refresh-symbolic").size(14))
                    .on_press(Message::RefreshMediaPlayers(device.id.clone()))
            )
            .spacing(spacing.space_xxs)
            .align_y(Alignment::Center);
        
        // Volume control row
        let volume_controls = widget::row()
            .push(widget::text("Volume").size(11))
            .push(widget::horizontal_space())
            .push(widget::icon::from_name("audio-volume-low-symbolic").size(14))
            .push(
                widget::slider(0.0..=100.0, current_volume, {
                    let device_id = device.id.clone();
                    move |value| Message::VolumeChanged(device_id.clone(), value as i32)
                })
                .width(Length::Fixed(100.0))
            )
            .push(widget::icon::from_name("audio-volume-high-symbolic").size(14))
            .push(widget::text(format!("{}%", current_volume as i32)).size(11))
            .spacing(spacing.space_xxs)
            .align_y(Alignment::Center);
        
        media_section = media_section.push(playback_controls);
        media_section = media_section.push(volume_controls);
        
        // Collapsible player selector - only show if multiple players
        if !device.available_players.is_empty() && device.available_players.len() > 1 {
            let current_player = device.current_player.as_ref()
                .and_then(|p| device.available_players.iter().find(|&x| x == p))
                .unwrap_or(&device.available_players[0]);
            
            // Compact button showing current player
            let player_button = widget::button::text(format!("♪ {} ▾", current_player))
                .width(Length::Fill)
                .class(cosmic::theme::Button::Text)
                .on_press(Message::TogglePlayerMenu(device.id.clone()));
            
            media_section = media_section.push(player_button);
            
            // Show player list if expanded
            if is_player_menu_expanded {
                let mut player_list = widget::column().spacing(2).padding([0, spacing.space_xs, 0, spacing.space_xs]);
                
                for player in &device.available_players {
                    let is_current = device.current_player.as_ref() == Some(player);
                    let player_name = player.clone();
                    let device_id = device.id.clone();
                    
                    let button = if is_current {
                        widget::button::text(format!("✓ {}", player))
                            .width(Length::Fill)
                            .class(cosmic::theme::Button::Text)
                    } else {
                        widget::button::text(format!("  {}", player))
                            .width(Length::Fill)
                            .class(cosmic::theme::Button::Text)
                    };
                    
                    player_list = player_list.push(
                        button.on_press(Message::MediaPlayerSelected(device_id, player_name))
                    );
                }
                
                media_section = media_section.push(player_list);
            }
        }
        
        info_col = info_col.push(media_section);
    }

    let mut device_content = widget::column()
        .push(info_col)
        .spacing(spacing.space_xs);

    // Only show menu items if expanded
    if is_expanded {
        let mut menu_items = widget::column().spacing(spacing.space_xxs);

        // Communication section
        if device.has_ping || device.has_findmyphone || device.has_sms || device.has_clipboard {
            menu_items = menu_items.push(widget::text("Communication").size(12).font(cosmic::font::bold()));
            
            if device.has_ping {
                menu_items = menu_items.push(
                    widget::button::text("Ping")
                        .on_press(Message::PingDevice(device.id.clone()))
                        .width(Length::Fill)
                        .class(cosmic::theme::Button::Text)
                );
            }
            
            if device.has_findmyphone {
                menu_items = menu_items.push(
                    widget::button::text("Ring device")
                        .on_press(Message::RingDevice(device.id.clone()))
                        .width(Length::Fill)
                        .class(cosmic::theme::Button::Text)
                );
            }
            
            if device.has_sms {
                menu_items = menu_items.push(
                    widget::button::text("SMS Chat")
                        .on_press(Message::SendSMS(device.id.clone()))
                        .width(Length::Fill)
                        .class(cosmic::theme::Button::Text)
                );
            }
            
            if device.has_clipboard {
                menu_items = menu_items.push(
                    widget::button::text("Share clipboard")
                        .on_press(Message::ShareClipboard(device.id.clone()))
                        .width(Length::Fill)
                        .class(cosmic::theme::Button::Text)
                );
            }
        }

        // File operations section
        if device.has_share || device.has_sftp {
            menu_items = menu_items.push(widget::divider::horizontal::light());
            menu_items = menu_items.push(widget::text("Files").size(12).font(cosmic::font::bold()));
            
            if device.has_share {
                menu_items = menu_items.push(
                    widget::button::text("Send file")
                        .on_press(Message::SendFile(device.id.clone()))
                        .width(Length::Fill)
                        .class(cosmic::theme::Button::Text)
                );
            }
            
            // Browse device functionality
            if device.has_sftp {
                menu_items = menu_items.push(
                    widget::button::text("Browse this device")
                        .on_press(Message::BrowseDevice(device.id.clone()))
                        .width(Length::Fill)
                        .class(cosmic::theme::Button::Text)
                );
            }
        }

        // Security & Display section (Lock device and Use as monitor only)
        if device.has_lockdevice || device.has_virtualmonitor {
            menu_items = menu_items.push(widget::divider::horizontal::light());
            menu_items = menu_items.push(widget::text("Security & Display").size(12).font(cosmic::font::bold()));
            
            if device.has_lockdevice {
                menu_items = menu_items.push(
                    widget::button::text("Lock device")
                        .on_press(Message::LockDevice(device.id.clone()))
                        .width(Length::Fill)
                        .class(cosmic::theme::Button::Text)
                );
            }
            
            if device.has_virtualmonitor {
                menu_items = menu_items.push(
                    widget::button::text("Use as monitor")
                        .on_press(Message::UseAsMonitor(device.id.clone()))
                        .width(Length::Fill)
                        .class(cosmic::theme::Button::Text)
                );
            }
        }

        device_content = device_content.push(menu_items);
    }

    widget::container(device_content.padding(spacing.space_xs))
        .class(cosmic::theme::Container::Card)
        .width(Length::Fill)
        .into()
}