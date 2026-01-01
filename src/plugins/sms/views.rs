// src/plugins/sms/views.rs
//! UI view implementations for the SMS window.

use cosmic::iced::{Alignment, Length};
use cosmic::widget;
use cosmic::Element;

use super::app::SmsWindow;
use super::emoji::EmojiCategory;
use super::messages::SmsMessage;
use super::utils::{format_timestamp, normalize_phone_number, phone_numbers_match, truncate_message};

impl SmsWindow {
    /// Renders the new chat dialog.
    pub fn view_new_chat_dialog(&self, spacing: &cosmic::cosmic_theme::Spacing) -> Element<'_, SmsMessage> {
        let mut dialog_content = widget::column()
            .spacing(spacing.space_m)
            .padding(spacing.space_l);

        // Title
        dialog_content = dialog_content.push(
            widget::text("Start New Chat")
                .size(20)
                .font(cosmic::font::bold())
        );

        // Phone number / contact name search input
        dialog_content = dialog_content.push(
            widget::column()
                .spacing(spacing.space_xs)
                .push(widget::text("Enter phone number or contact name:").size(14))
                .push(
                    widget::text_input("e.g., +1-555-123-4567 or John Doe", &self.new_chat_phone_input)
                        .on_input(SmsMessage::UpdateNewChatPhone)
                        .width(Length::Fill)
                )
        );

        // Action buttons
        dialog_content = dialog_content.push(self.view_new_chat_actions(spacing));

        // Divider and contacts section
        dialog_content = dialog_content.push(widget::divider::horizontal::default());
        dialog_content = dialog_content.push(widget::text("Or select from contacts:").size(14));

        // Contacts list
        dialog_content = dialog_content.push(self.view_contacts_list(spacing));

        widget::container(dialog_content)
            .class(cosmic::theme::Container::Card)
            .width(Length::Fixed(500.0))
            .max_height(600.0)
            .into()
    }

    fn view_new_chat_actions(&self, spacing: &cosmic::cosmic_theme::Spacing) -> Element<'_, SmsMessage> {
        let start_button_action = if !self.new_chat_phone_input.trim().is_empty() {
            let input_lower = self.new_chat_phone_input.trim().to_lowercase();
            
            let matching_contact = self.contacts.iter()
                .find(|(_, name)| name.to_lowercase() == input_lower)
                .or_else(|| {
                    self.contacts.iter()
                        .find(|(_, name)| name.to_lowercase().contains(&input_lower))
                });
            
            if let Some((phone, _)) = matching_contact {
                Some(SmsMessage::StartChatWithNumber(phone.clone()))
            } else {
                Some(SmsMessage::StartChatWithNumber(self.new_chat_phone_input.clone()))
            }
        } else {
            None
        };

        widget::row()
            .spacing(spacing.space_xs)
            .push(widget::button::standard("Cancel").on_press(SmsMessage::CloseNewChatDialog))
            .push(widget::horizontal_space())
            .push(widget::button::suggested("Start Chat").on_press_maybe(start_button_action))
            .into()
    }

    fn view_contacts_list(&self, spacing: &cosmic::cosmic_theme::Spacing) -> Element<'_, SmsMessage> {
        if self.contacts.is_empty() {
            return widget::text("No contacts available").size(12).into();
        }

        let mut contacts_list = widget::column().spacing(spacing.space_xxs);
        
        let filtered_contacts = self.get_filtered_contacts();
        
        if filtered_contacts.is_empty() {
            contacts_list = contacts_list.push(widget::text("No matching contacts").size(12));
        } else {
            for (phone, name) in filtered_contacts.iter() {
                let contact_btn = widget::button::text(format!("{} ({})", name, phone))
                    .on_press(SmsMessage::SelectContactForNewChat(phone.to_string(), name.to_string()))
                    .width(Length::Fill);
                
                contacts_list = contacts_list.push(contact_btn);
            }
            
            contacts_list = contacts_list.push(
                widget::container(
                    widget::text(format!(
                        "Showing {} contact{}",
                        filtered_contacts.len(),
                        if filtered_contacts.len() == 1 { "" } else { "s" }
                    ))
                    .size(11)
                )
                .padding([spacing.space_xs, 0, 0, 0])
            );
        }
        
        widget::scrollable(contacts_list)
            .height(Length::Fixed(400.0))
            .into()
    }

    fn get_filtered_contacts(&self) -> Vec<(&String, &String)> {
        let mut sorted_contacts: Vec<_> = self.contacts.iter().collect();
        sorted_contacts.sort_by(|a, b| a.1.cmp(b.1));
        
        let search_term = self.new_chat_phone_input.trim().to_lowercase();
        
        if search_term.is_empty() {
            sorted_contacts
        } else {
            sorted_contacts.into_iter()
                .filter(|(phone, name)| {
                    name.to_lowercase().contains(&search_term)
                        || phone.contains(&search_term)
                        || normalize_phone_number(phone).contains(&normalize_phone_number(&search_term))
                })
                .collect()
        }
    }

    /// Renders the conversations list panel.
    pub fn view_conversations_list(&self, spacing: &cosmic::cosmic_theme::Spacing) -> Element<'_, SmsMessage> {
        let mut content = widget::column().spacing(spacing.space_xs);

        // Start Chat button
        content = content.push(
            widget::container(
                widget::button::suggested("Start Chat")
                    .on_press(SmsMessage::OpenNewChatDialog)
                    .width(Length::Fill)
            )
            .padding(spacing.space_s)
        );

        // Search input
        content = content.push(
            widget::text_input("Search conversations...", &self.search_query)
                .on_input(SmsMessage::UpdateSearch)
                .padding(spacing.space_s)
        );
        content = content.push(widget::divider::horizontal::default());

        // Filter conversations
        let mut filtered: Vec<_> = self.conversations
            .iter()
            .filter(|c| self.conversation_matches_search(c))
            .collect();
        
        // Ensure conversations are sorted by timestamp (most recent first)
        filtered.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        if filtered.is_empty() {
            let msg = if self.search_query.is_empty() {
                "No conversations"
            } else {
                "No matching conversations"
            };
            
            content = content.push(
                widget::container(widget::text(msg).size(14))
                    .width(Length::Fill)
                    .padding(spacing.space_xl)
                    .center_x(Length::Fill)
            );
        } else {
            let mut list = widget::column().spacing(0);

            for conv in filtered {
                list = list.push(self.view_conversation_item(conv, spacing));
                list = list.push(widget::divider::horizontal::light());
            }

            content = content.push(widget::scrollable(list).height(Length::Fill));
        }

        widget::container(content)
            .width(Length::Fixed(300.0))
            .height(Length::Fill)
            .into()
    }

    fn conversation_matches_search(&self, conv: &super::models::Conversation) -> bool {
        if self.search_query.is_empty() {
            return true;
        }
        
        let query = self.search_query.to_lowercase();
        conv.contact_name.to_lowercase().contains(&query)
            || conv.phone_number.contains(&self.search_query)
            || conv.last_message.to_lowercase().contains(&query)
    }

    fn view_conversation_item<'a>(
        &'a self,
        conv: &'a super::models::Conversation,
        spacing: &cosmic::cosmic_theme::Spacing,
    ) -> Element<'a, SmsMessage> {
        let is_selected = self.selected_thread.as_ref() == Some(&conv.thread_id);
        
        let display_name = self.get_contact_name(&conv.phone_number)
            .unwrap_or_else(|| conv.contact_name.clone());
        
        let preview_text = truncate_message(&conv.last_message, 40);
        
        let button = widget::button::custom(
            widget::container(
                widget::column()
                    .push(
                        widget::row()
                            .push(widget::text(display_name).size(14).font(cosmic::font::bold()))
                            .push(widget::horizontal_space())
                            .push(widget::text(format_timestamp(conv.timestamp)).size(11))
                            .spacing(spacing.space_xs)
                    )
                    .push(widget::text(preview_text).size(12))
                    .spacing(spacing.space_xxs)
                    .padding(spacing.space_s)
            )
            .width(Length::Fill)
        )
        .on_press(SmsMessage::SelectThread(conv.thread_id.clone()))
        .width(Length::Fill)
        .class(cosmic::theme::Button::Text);
        
        if is_selected {
            widget::container(button)
                .class(cosmic::theme::Container::Primary)
                .width(Length::Fill)
                .into()
        } else {
            button.into()
        }
    }

    /// Renders the message thread panel.
    pub fn view_message_thread(&self, spacing: &cosmic::cosmic_theme::Spacing) -> Element<'_, SmsMessage> {
        let Some(thread_id) = &self.selected_thread else {
            return widget::container(
                widget::text("Select a conversation to view messages").size(16)
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into();
        };

        let mut content = widget::column().spacing(0);

        // Header
        if let Some(conv) = self.conversations.iter().find(|c| c.thread_id == *thread_id) {
            content = content.push(self.view_thread_header(conv, spacing));
            content = content.push(widget::divider::horizontal::default());
        }

        // Messages
        content = content.push(self.view_messages_list(spacing));
        content = content.push(widget::divider::horizontal::default());
        
        // Input
        content = content.push(self.view_message_input(spacing));

        widget::container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn view_thread_header<'a>(
        &'a self,
        conv: &'a super::models::Conversation,
        spacing: &cosmic::cosmic_theme::Spacing,
    ) -> Element<'a, SmsMessage> {
        let display_name = self.get_contact_name(&conv.phone_number)
            .unwrap_or_else(|| conv.contact_name.clone());
        
        widget::container(
            widget::column()
                .push(widget::text(display_name).size(16).font(cosmic::font::bold()))
                .push(widget::text(&conv.phone_number).size(12))
                .spacing(spacing.space_xxs)
                .padding(spacing.space_s)
        )
        .class(cosmic::theme::Container::Card)
        .width(Length::Fill)
        .into()
    }

    fn view_messages_list(&self, spacing: &cosmic::cosmic_theme::Spacing) -> Element<'_, SmsMessage> {
        let mut messages_column = widget::column()
            .spacing(spacing.space_m)
            .padding(spacing.space_m);
        
        if self.messages.is_empty() {
            messages_column = messages_column.push(
                widget::container(
                    widget::column()
                        .push(widget::text("Waiting for messages...").size(14))
                        .push(widget::text("Messages will appear as they arrive from your phone").size(12))
                        .spacing(spacing.space_xs)
                        .align_x(Alignment::Center)
                )
                .width(Length::Fill)
                .center_x(Length::Fill)
                .padding(spacing.space_xl)
            );
        } else {
            for msg in &self.messages {
                messages_column = messages_column.push(self.view_message_bubble(msg, spacing));
            }
        }

        widget::scrollable(messages_column)
            .height(Length::Fill)
            .anchor_bottom()
            .into()
    }

    fn view_message_bubble<'a>(
        &'a self,
        msg: &'a super::models::Message,
        spacing: &cosmic::cosmic_theme::Spacing,
    ) -> Element<'a, SmsMessage> {
        let is_sent = msg.is_sent();
        let mut message_content = widget::column().spacing(spacing.space_xxs);
        
        // Show sender label only for received messages
        if !is_sent {
            let phone_number = self.get_current_conversation_phone()
                .unwrap_or_else(|| msg.address.clone());
            
            let sender_label = self.get_contact_name(&phone_number)
                .unwrap_or_else(|| phone_number.clone());
            
            message_content = message_content.push(
                widget::text(sender_label)
                    .size(11)
                    .font(cosmic::font::bold())
            );
        }
        
        message_content = message_content
            .push(widget::text(&msg.body).size(14))
            .push(widget::text(format_timestamp(msg.date)).size(11))
            .padding(spacing.space_s);
        
        let message_bubble = widget::container(message_content)
            .class(if is_sent {
                cosmic::theme::Container::Primary
            } else {
                cosmic::theme::Container::Card
            })
            .max_width(500.0);

        if is_sent {
            widget::row()
                .push(widget::horizontal_space())
                .push(message_bubble)
                .width(Length::Fill)
                .into()
        } else {
            widget::row()
                .push(message_bubble)
                .width(Length::Fill)
                .into()
        }
    }

    fn view_message_input(&self, spacing: &cosmic::cosmic_theme::Spacing) -> Element<'_, SmsMessage> {
        let mut content = widget::column().spacing(0);

        // Show emoji picker if enabled
        if self.show_emoji_picker {
            content = content.push(self.view_emoji_picker(spacing));
            content = content.push(widget::divider::horizontal::default());
        }

        // Message input row
        let input_row = widget::row()
            .push(
                widget::text_input("Type a message...", &self.message_input)
                    .on_input(SmsMessage::UpdateInput)
                    .on_submit(|_| SmsMessage::SendMessage)
                    .padding(spacing.space_s)
                    .width(Length::Fill)
            )
            .push(
                widget::button::icon(widget::icon::from_name("face-smile-symbolic"))
                    .on_press(SmsMessage::ToggleEmojiPicker)
                    .padding(spacing.space_xs)
            )
            .push(
                widget::button::suggested("Send")
                    .on_press(SmsMessage::SendMessage)
            )
            .spacing(spacing.space_xs)
            .padding(spacing.space_s)
            .align_y(Alignment::Center);

        content.push(input_row).into()
    }

    fn view_emoji_picker(&self, spacing: &cosmic::cosmic_theme::Spacing) -> Element<'_, SmsMessage> {
        let mut content = widget::column().spacing(spacing.space_xs);

        // Category tabs
        let mut category_row = widget::row().spacing(spacing.space_xxs);
        for category in EmojiCategory::all() {
            let is_selected = category == self.emoji_category;
            let label = category.label().to_string();  // Convert to owned String
            let button = if is_selected {
                widget::button::text(label)
                    .on_press(SmsMessage::SelectEmojiCategory(category))
                    .class(cosmic::theme::Button::Suggested)
            } else {
                widget::button::text(label)
                    .on_press(SmsMessage::SelectEmojiCategory(category))
            };
            category_row = category_row.push(button);
        }
        content = content.push(category_row);

        // Emoji grid
        let emojis = self.emoji_category.emojis();
        let mut emoji_grid = widget::column().spacing(spacing.space_xxs);
        let mut current_row = widget::row().spacing(spacing.space_xxs);
        let emojis_per_row = 10;

        for (i, emoji) in emojis.iter().enumerate() {
            let emoji_btn = widget::button::text(*emoji)
                .on_press(SmsMessage::InsertEmoji(emoji.to_string()))
                .width(Length::Fixed(45.0))  // Larger button
                .height(Length::Fixed(45.0));
            
            current_row = current_row.push(emoji_btn);

            if (i + 1) % emojis_per_row == 0 || i == emojis.len() - 1 {
                emoji_grid = emoji_grid.push(current_row);
                current_row = widget::row().spacing(spacing.space_xxs);
            }
        }

        content = content.push(
            widget::scrollable(emoji_grid)
                .height(Length::Fixed(250.0))
        );

        widget::container(content)
            .class(cosmic::theme::Container::Card)
            .padding(spacing.space_s)
            .into()
    }

    // Helper methods

    pub(crate) fn get_contact_name(&self, phone_number: &str) -> Option<String> {
        self.contacts.iter()
            .find(|(contact_phone, _)| phone_numbers_match(phone_number, contact_phone))
            .map(|(_, name)| name.clone())
    }

    fn get_current_conversation_phone(&self) -> Option<String> {
        let thread_id = self.selected_thread.as_ref()?;
        self.conversations.iter()
            .find(|c| c.thread_id == *thread_id)
            .map(|c| c.phone_number.clone())
    }
}