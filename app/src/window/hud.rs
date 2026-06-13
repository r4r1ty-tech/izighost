use eframe::egui;
use eframe::egui::{Color32, RichText, Vec2, Stroke};
use std::sync::Arc;
use crate::dbus::{DaemonClient, DaemonSignal};

pub struct HudState {
    pub input_text: String,
    pub chat_messages: Vec<(String, String)>, // (role, message)
    pub is_generating: bool,
    pub is_listening: bool,
    pub active_profile_name: String,
    pub show_preferences: bool,
}

impl HudState {
    pub fn new() -> Self {
        Self {
            input_text: String::new(),
            chat_messages: Vec::new(),
            is_generating: false,
            is_listening: false,
            active_profile_name: "Не выбран".to_string(),
            show_preferences: false,
        }
    }

    /// Обработка входящих D-Bus сигналов
    pub fn handle_dbus_signal(&mut self, signal: DaemonSignal) {
        match signal {
            DaemonSignal::ChatChunk(chunk) => {
                self.is_generating = true;
                if let Some((role, content)) = self.chat_messages.last_mut() {
                    if role == "assistant" {
                        content.push_str(&chunk);
                        return;
                    }
                }
                self.chat_messages.push(("assistant".to_string(), chunk));
            }
            DaemonSignal::ChatCompleted => {
                self.is_generating = false;
            }
            DaemonSignal::OcrCompleted(text) => {
                self.input_text = text;
            }
            DaemonSignal::AsrCompleted(text) => {
                self.is_listening = false;
                self.input_text = text;
            }
            DaemonSignal::ErrorOccurred(msg) => {
                self.is_generating = false;
                self.is_listening = false;
                self.chat_messages.push(("system".to_string(), format!("Ошибка: {}", msg)));
            }
        }
    }

    /// Отрисовка HUD интерфейса
    pub fn draw(&mut self, ui: &mut egui::Ui, dbus_client: &Option<Arc<DaemonClient>>, active_profile: &Option<String>) {
        ui.style_mut().spacing.item_spacing = Vec2::new(8.0, 8.0);

        // Обновляем имя активного профиля
        if let Some(profile_name) = active_profile {
            self.active_profile_name = profile_name.clone();
        } else {
            self.active_profile_name = "Не выбран".to_string();
        }

        // Рендерим HUD внутри контейнера с красивой рамкой
        let border_color = if self.is_generating {
            Color32::from_rgb(99, 102, 241) // Индиго пульсация (в коде статично)
        } else if self.is_listening {
            Color32::from_rgb(16, 185, 129) // Зеленый для ASR
        } else {
            Color32::from_rgb(45, 45, 50)
        };

        let frame = egui::Frame::none()
            .fill(Color32::from_rgba_unmultiplied(20, 20, 22, 240)) // Glassmorphism
            .stroke(Stroke::new(1.5, border_color))
            .inner_margin(12.0)
            .corner_radius(12.0);

        frame.show(ui, |ui| {
            self.draw_header(ui);
            ui.separator();
            self.draw_chat_history(ui);
            ui.separator();
            self.draw_input_bar(ui, dbus_client);
        });
    }

    /// Заголовок HUD
    fn draw_header(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("IziGhost HUD").bold().color(Color32::WHITE));
            ui.add_space(4.0);

            // Бейдж активного профиля
            let badge_frame = egui::Frame::none()
                .fill(Color32::from_rgb(45, 45, 50))
                .inner_margin(Vec2::new(6.0, 2.0))
                .corner_radius(4.0);
            
            badge_frame.show(ui, |ui| {
                ui.label(
                    RichText::new(&self.active_profile_name)
                        .size(11.0)
                        .color(Color32::from_rgb(160, 160, 170))
                );
            });

            // Кнопка открытия настроек
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let settings_btn = ui.add(
                    egui::Button::new(RichText::new("⚙").size(16.0).color(Color32::from_rgb(200, 200, 205)))
                        .frame(false)
                );
                
                if settings_btn.clicked() {
                    self.show_preferences = !self.show_preferences;
                }
            });
        });
    }

    /// Список сообщений чата
    fn draw_chat_history(&mut self, ui: &mut egui::Ui) {
        let height = ui.available_height() - 48.0;
        
        egui::ScrollArea::vertical()
            .max_height(height)
            .auto_shrink(false)
            .stick_to_bottom(true)
            .show(ui, |ui| {
                if self.chat_messages.is_empty() {
                    ui.vertical_centered(|ui| {
                        ui.add_space(30.0);
                        ui.label(RichText::new("Ассистент готов к работе.").color(Color32::from_rgb(110, 110, 120)));
                        ui.label(RichText::new("Задайте вопрос текстом, скриншотом или голосом.").size(11.0).color(Color32::from_rgb(90, 90, 100)));
                    });
                } else {
                    for (role, text) in &self.chat_messages {
                        let is_user = role == "user";
                        let is_system = role == "system";
                        
                        let align = if is_user { egui::Align::Max } else { egui::Align::Min };
                        
                        ui.with_layout(egui::Layout::top_down(align), |ui| {
                            let bg = if is_user {
                                Color32::from_rgb(79, 70, 229) // Indigo
                            } else if is_system {
                                Color32::from_rgb(220, 38, 38) // Red
                            } else {
                                Color32::from_rgb(37, 37, 41) // Dark grey
                            };

                            let msg_frame = egui::Frame::none()
                                .fill(bg)
                                .inner_margin(8.0)
                                .corner_radius(8.0);

                            msg_frame.show(ui, |ui| {
                                ui.label(RichText::new(text).color(Color32::WHITE));
                            });
                        });
                        ui.add_space(4.0);
                    }
                }

                // Индикатор генерации
                if self.is_generating {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("IziGhost печатает...").italics().color(Color32::from_rgb(110, 110, 120)));
                    });
                }
            });
    }

    /// Панель ввода с кнопками OCR, ASR и отправки
    fn draw_input_bar(&mut self, ui: &mut egui::Ui, dbus_client: &Option<Arc<DaemonClient>>) {
        ui.horizontal(|ui| {
            // Кнопка скриншота (OCR)
            let ocr_btn = ui.add(
                egui::Button::new(RichText::new("📷").size(16.0))
                    .fill(Color32::from_rgb(45, 45, 50))
            ).on_hover_text("Сделать скриншот и распознать текст");

            if ocr_btn.clicked() {
                if let Some(client) = dbus_client {
                    let client = client.clone();
                    tokio::spawn(async move {
                        let _ = client.trigger_ocr().await;
                    });
                }
            }

            // Кнопка голосового ввода (ASR)
            let asr_color = if self.is_listening {
                Color32::from_rgb(16, 185, 129) // Green active
            } else {
                Color32::from_rgb(45, 45, 50)
            };

            let asr_btn = ui.add(
                egui::Button::new(RichText::new("🎙").size(16.0).color(Color32::WHITE))
                    .fill(asr_color)
            ).on_hover_text("Голосовой ввод");

            if asr_btn.clicked() {
                if let Some(client) = dbus_client {
                    let client = client.clone();
                    let was_listening = self.is_listening;
                    self.is_listening = !was_listening;
                    tokio::spawn(async move {
                        if was_listening {
                            let _ = client.stop_listening().await;
                        } else {
                            let _ = client.start_listening().await;
                        }
                    });
                }
            }

            // Поле текстового ввода
            let input_width = ui.available_width() - 36.0;
            let text_edit = ui.add_sized(
                [input_width, 26.0],
                egui::TextEdit::singleline(&mut self.input_text)
                    .hint_text("Задать вопрос...")
            );

            let send_clicked = ui.button("➤").clicked();
            let enter_pressed = text_edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));

            if (send_clicked || enter_pressed) && !self.input_text.trim().is_empty() {
                let text = self.input_text.trim().to_string();
                self.chat_messages.push(("user".to_string(), text.clone()));
                self.input_text.clear();
                self.is_generating = true;

                if let Some(client) = dbus_client {
                    let client = client.clone();
                    tokio::spawn(async move {
                        let _ = client.send_chat_message(&text).await;
                    });
                }
            }
        });
    }
}
