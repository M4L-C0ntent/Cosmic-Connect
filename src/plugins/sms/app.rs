// src/plugins/sms/app.rs
//! Main SMS window application logic.

use cosmic::app::{Core, Task};
use cosmic::iced::{Length, Subscription};
use cosmic::widget;
use cosmic::{Application, ApplicationExt, Element};

use super::dbus;
use super::emoji::EmojiCategory;
use super::messages::SmsMessage;
use super::models::{ContactsMap, Conversation, Message, SignalEvent};
use super::utils::{now_millis, phone_numbers_match};

/// The SMS window application state.
pub struct SmsWindow {
    pub(crate) core: Core,
    pub(crate) device_id: String,
    #[allow(dead_code)] // Used in title, may be used in future features
    pub(crate) device_name: String,
    pub(crate) conversations: Vec<Conversation>,
    pub(crate) messages: Vec<Message>,
    pub(crate) selected_thread: Option<String>,
    pub(crate) message_input: String,
    pub(crate) search_query: String,
    pub(crate) is_loading: bool,
    pub(crate) contacts: ContactsMap,
    pub(crate) show_new_chat_dialog: bool,
    pub(crate) new_chat_phone_input: String,
    pub(crate) show_emoji_picker: bool,
    pub(crate) emoji_category: EmojiCategory,
}

impl Application for SmsWindow {
    type Executor = cosmic::executor::Default;
    type Flags = (String, String);
    type Message = SmsMessage;
    const APP_ID: &str = "io.github.M4LC0ntent.CosmicKdeConnect.SMS";

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(core: Core, flags: Self::Flags) -> (Self, Task<Self::Message>) {
        let (device_id, device_name) = flags;
        
        eprintln!("=== SMS Window Starting ===");
        eprintln!("Device: {} ({})", device_name, device_id);
        
        let mut app = SmsWindow {
            core,
            device_id: device_id.clone(),
            device_name: device_name.clone(),
            conversations: Vec::new(),
            messages: Vec::new(),
            selected_thread: None,
            message_input: String::new(),
            search_query: String::new(),
            is_loading: true,
            contacts: ContactsMap::new(),
            show_new_chat_dialog: false,
            new_chat_phone_input: String::new(),
            show_emoji_picker: false,
            emoji_category: EmojiCategory::Smileys,
        };

        let title = format!("SMS - {}", device_name);
        let title_task = app.set_window_title(title, app.core.main_window_id().unwrap());

        let device_id_conv = device_id.clone();
        let device_id_contacts = device_id.clone();

        (
            app,
            Task::batch(vec![
                title_task,
                cosmic::task::future(async move {
                    SmsMessage::ConversationsLoaded(dbus::fetch_conversations(device_id_conv).await)
                }),
                cosmic::task::future(async move {
                    SmsMessage::ContactsLoaded(dbus::fetch_contacts(device_id_contacts).await)
                }),
            ]),
        )
    }

    fn update(&mut self, message: Self::Message) -> Task<Self::Message> {
        match message {
            SmsMessage::SignalReceived(event) => self.handle_signal_event(event),
            SmsMessage::LoadConversations => return self.load_conversations(),
            SmsMessage::ConversationsLoaded(conversations) => {
                self.on_conversations_loaded(conversations);
            }
            SmsMessage::ContactsLoaded(contacts) => {
                self.on_contacts_loaded(contacts);
            }
            SmsMessage::SelectThread(thread_id) => {
                self.select_thread(thread_id);
            }
            SmsMessage::UpdateInput(text) => {
                self.message_input = text;
            }
            SmsMessage::UpdateSearch(query) => {
                self.search_query = query;
            }
            SmsMessage::SendMessage => {
                self.send_message();
            }
            SmsMessage::RefreshThread => {
                self.refresh_current_thread();
            }
            SmsMessage::CloseWindow => {
                return cosmic::iced::exit();
            }
            SmsMessage::OpenNewChatDialog => {
                self.show_new_chat_dialog = true;
                self.new_chat_phone_input.clear();
            }
            SmsMessage::CloseNewChatDialog => {
                self.show_new_chat_dialog = false;
                self.new_chat_phone_input.clear();
            }
            SmsMessage::UpdateNewChatPhone(phone) => {
                self.new_chat_phone_input = phone;
            }
            SmsMessage::SelectContactForNewChat(phone, _name) => {
                self.show_new_chat_dialog = false;
                return cosmic::task::message(cosmic::Action::App(
                    SmsMessage::StartChatWithNumber(phone)
                ));
            }
            SmsMessage::StartChatWithNumber(phone) => {
                return self.start_chat_with_number(phone);
            }
            SmsMessage::ToggleEmojiPicker => {
                self.show_emoji_picker = !self.show_emoji_picker;
            }
            SmsMessage::SelectEmojiCategory(category) => {
                self.emoji_category = category;
            }
            SmsMessage::InsertEmoji(emoji) => {
                self.message_input.push_str(&emoji);
                self.show_emoji_picker = false;
            }
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Self::Message> {
        let spacing = cosmic::theme::active().cosmic().spacing;

        if self.show_new_chat_dialog {
            return widget::container(
                widget::container(self.view_new_chat_dialog(&spacing))
                    .center_x(Length::Fill)
                    .center_y(Length::Fill)
                    .width(Length::Fill)
                    .height(Length::Fill)
            )
            .into();
        }

        let left_panel = self.view_conversations_list(&spacing);
        let right_panel = self.view_message_thread(&spacing);

        let content = widget::row()
            .push(left_panel)
            .push(widget::divider::vertical::default())
            .push(right_panel)
            .spacing(0);

        widget::container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        let device_id = self.device_id.clone();
        
        Subscription::batch(vec![
            cosmic::iced::time::every(std::time::Duration::from_secs(30))
                .map(|_| SmsMessage::LoadConversations),
            Subscription::run_with_id(
                "sms_signals",
                dbus::listen_for_sms_signals_stream(device_id)
            ),
        ])
    }
}

// Private implementation methods
impl SmsWindow {
    fn handle_signal_event(&mut self, event: SignalEvent) {
        match event {
            SignalEvent::MessageReceived(msg) => {
                eprintln!("ÃƒÂ°Ã…Â¸Ã…Â½Ã‚Â¯ UI: Received message for thread {}", msg.thread_id);
                eprintln!("   Body: {}", msg.body.chars().take(50).collect::<String>());
                eprintln!("   Selected thread: {:?}", self.selected_thread);
                
                // Add to messages if it's for the selected thread
                if self.selected_thread.as_ref() == Some(&msg.thread_id) {
                    if !self.messages.iter().any(|m| m.id == msg.id) {
                        eprintln!("   ÃƒÂ¢Ã…â€œÃ¢â‚¬Å“ Adding to current thread view!");
                        self.messages.push(msg.clone());
                        self.messages.sort_by_key(|m| m.date);
                        eprintln!("   Total messages now: {}", self.messages.len());
                    } else {
                        eprintln!("   ÃƒÂ¢Ã…Â¡Ã‚Â  Message already exists, skipping");
                    }
                } else {
                    eprintln!("   ÃƒÂ¢Ã…Â¡Ã‚Â  Not for selected thread, skipping UI update");
                }
                
                // Update conversation last message
                if let Some(conv) = self.conversations.iter_mut()
                    .find(|c| c.thread_id == msg.thread_id)
                {
                    conv.last_message = msg.body.clone();
                    conv.timestamp = msg.date;
                }
                self.conversations.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
            }
            SignalEvent::Error(err) => {
                eprintln!("ÃƒÂ¢Ã‚ÂÃ…â€™ Signal error: {}", err);
            }
        }
    }

    fn load_conversations(&mut self) -> Task<SmsMessage> {
        self.is_loading = true;
        let device_id = self.device_id.clone();
        cosmic::task::future(async move {
            SmsMessage::ConversationsLoaded(dbus::fetch_conversations(device_id).await)
        })
    }

    fn on_conversations_loaded(&mut self, conversations: Vec<Conversation>) {
        eprintln!("Loaded {} conversations", conversations.len());
        self.conversations = conversations;
        self.conversations.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        self.is_loading = false;
    }

    fn on_contacts_loaded(&mut self, contacts: ContactsMap) {
        eprintln!("=== CONTACTS LOADED ===");
        eprintln!("Loaded {} contacts", contacts.len());
        for (phone, name) in contacts.iter().take(5) {
            eprintln!("  {} -> {}", phone, name);
        }
        self.contacts = contacts;
        eprintln!("Total contacts in app: {}", self.contacts.len());
        
        // UPDATE: Apply contact names to existing conversations
        eprintln!("Updating conversation contact names...");
        let mut updated_count = 0;
        for conv in &mut self.conversations {
            // Try to find a matching contact using phone number matching
            // This handles different phone number formats (e.g., +1-555-123-4567 vs 5551234567)
            if let Some((_, name)) = self.contacts.iter()
                .find(|(contact_phone, _)| phone_numbers_match(&conv.phone_number, contact_phone))
            {
                if conv.contact_name != *name {
                    eprintln!("  {} -> {}", conv.phone_number, name);
                    conv.contact_name = name.clone();
                    updated_count += 1;
                }
            }
        }
        eprintln!("Ã¢Å“â€œ Updated {} conversation names", updated_count);
    }

    fn select_thread(&mut self, thread_id: String) {
        eprintln!("Selected thread: {}", thread_id);
        self.selected_thread = Some(thread_id.clone());
        self.messages.clear();
        
        let device_id = self.device_id.clone();
        let thread_clone = thread_id.clone();
        
        tokio::spawn(async move {
            dbus::request_conversation_messages(device_id, thread_clone).await;
        });
    }

    fn send_message(&mut self) {
        let _ = std::fs::write("/tmp/sms_debug.log", format!(
            "SendMessage triggered at {}\n",
            now_millis() / 1000
        ));
        
        if self.message_input.trim().is_empty() {
            eprintln!("SendMessage: message is empty, ignoring");
            return;
        }

        let Some(thread_id) = &self.selected_thread else {
            eprintln!("SendMessage: no thread selected");
            return;
        };

        let Some(conv) = self.conversations.iter_mut().find(|c| c.thread_id == *thread_id) else {
            eprintln!("SendMessage: conversation not found for thread_id");
            return;
        };

        let device_id = self.device_id.clone();
        let phone = conv.phone_number.clone();
        let message = self.message_input.clone();
        let thread_id_clone = thread_id.clone();
        
        let log_msg = format!(
            "=== SendMessage Handler Triggered ===\nThread ID: {}\nPhone: {}\nMessage: {}\n",
            thread_id, phone, message
        );
        eprintln!("{}", log_msg);
        let _ = std::fs::write("/tmp/sms_debug.log", log_msg);
        
        // Create optimistic message for instant UI feedback
        let optimistic_msg = Message {
            id: format!("sending_{}", now_millis()),
            thread_id: thread_id_clone,
            body: message.clone(),
            address: phone.clone(),
            date: now_millis(),
            type_: 2, // Sent message
            read: true,
        };
        
        self.messages.push(optimistic_msg);
        self.messages.sort_by_key(|m| m.date);
        
        // Update conversation
        conv.last_message = message.clone();
        conv.timestamp = now_millis();
        self.conversations.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        
        self.message_input.clear();
        
        eprintln!("Creating task to send SMS...");
        let _ = std::fs::write("/tmp/sms_debug.log", "About to spawn task\n");
        
        // Spawn send operation
        tokio::spawn(async move {
            let _ = std::fs::write("/tmp/sms_send_task.log", "Task started\n");
            eprintln!("Task started - about to call send_sms");
            dbus::send_sms(device_id, phone, message).await;
            eprintln!("send_sms completed");
            let _ = std::fs::write("/tmp/sms_send_task.log", "Task completed\n");
        });
        
        eprintln!("Task spawned, returning from SendMessage handler");
    }

    fn refresh_current_thread(&mut self) {
        if let Some(thread_id) = &self.selected_thread {
            let device_id = self.device_id.clone();
            let thread_id = thread_id.clone();
            
            tokio::spawn(async move {
                dbus::request_conversation_messages(device_id, thread_id).await;
            });
        }
    }

    fn start_chat_with_number(&mut self, phone: String) -> Task<SmsMessage> {
        eprintln!("=== Starting chat with: {} ===", phone);
        
        // Check for existing conversation
        if let Some(conv) = self.conversations.iter()
            .find(|conv| phone_numbers_match(&conv.phone_number, &phone))
        {
            eprintln!("Found existing conversation: {}", conv.thread_id);
            self.show_new_chat_dialog = false;
            return cosmic::task::message(cosmic::Action::App(
                SmsMessage::SelectThread(conv.thread_id.clone())
            ));
        }
        
        eprintln!("No existing conversation found, creating new one");
        
        let thread_id = format!("new_{}", now_millis());
        
        let contact_name = self.contacts.iter()
            .find(|(contact_phone, _)| phone_numbers_match(&phone, contact_phone))
            .map(|(_, name)| name.clone())
            .unwrap_or_else(|| phone.clone());
        
        let new_conversation = Conversation {
            thread_id: thread_id.clone(),
            contact_name,
            phone_number: phone,
            last_message: "New conversation".to_string(),
            timestamp: now_millis(),
            unread: false,
        };
        
        self.conversations.insert(0, new_conversation);
        self.show_new_chat_dialog = false;
        
        eprintln!("Created new conversation with thread_id: {}", thread_id);
        cosmic::task::message(cosmic::Action::App(SmsMessage::SelectThread(thread_id)))
    }
}

/// Runs the SMS window application.
pub fn run(device_id: String, device_name: String) -> cosmic::iced::Result {
    let settings = cosmic::app::Settings::default()
        .size_limits(cosmic::iced::Limits::NONE.min_width(700.0).min_height(500.0))
        .size(cosmic::iced::Size::new(900.0, 600.0));
    
    cosmic::app::run::<SmsWindow>(settings, (device_id, device_name))
}