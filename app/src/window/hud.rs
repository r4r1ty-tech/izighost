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
    pub is_pinned: bool,
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
            is_pinned: true,
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

        let frame = egui::Frame::NONE
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
            // Заголовок HUD
            ui.label(RichText::new("IziGhost HUD").strong().color(Color32::WHITE));

            ui.add_space(2.0);

            // Бейдж активного профиля
            let badge_frame = egui::Frame::NONE
                .fill(Color32::from_rgb(45, 45, 50))
                .inner_margin(Vec2::new(6.0, 2.0))
                .corner_radius(4.0);
            
            badge_frame.show(ui, |ui| {
                ui.label(
                    RichText::new(&self.active_profile_name)
                        .size(11.0)
                        .color(Color32::from_rgb(16, 185, 129))
                );
            });

            // Кнопки управления (справа налево)
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // 1. Выход (close)
                let close_btn = icon_button(ui, egui::vec2(24.0, 24.0), "close", Color32::TRANSPARENT, false)
                    .on_hover_text("Закрыть приложение");
                if close_btn.clicked() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }

                ui.add_space(2.0);

                // 2. Настройки (gear)
                let settings_btn = icon_button(ui, egui::vec2(24.0, 24.0), "gear", Color32::TRANSPARENT, false)
                    .on_hover_text("Настройки профилей");
                if settings_btn.clicked() {
                    self.show_preferences = !self.show_preferences;
                }

                ui.add_space(2.0);

                // 3. Закрепить/Открепить (pin)
                let pin_btn = icon_button(ui, egui::vec2(24.0, 24.0), "pin", Color32::TRANSPARENT, self.is_pinned)
                    .on_hover_text(if self.is_pinned { "Открепить от экрана" } else { "Закрепить поверх всех окон" });
                if pin_btn.clicked() {
                    self.is_pinned = !self.is_pinned;
                    let level = if self.is_pinned {
                        egui::WindowLevel::AlwaysOnTop
                    } else {
                        egui::WindowLevel::Normal
                    };
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::WindowLevel(level));
                }

                ui.add_space(2.0);

                // 4. Перенести/Двигать (drag)
                let drag_btn = icon_button(ui, egui::vec2(24.0, 24.0), "drag", Color32::TRANSPARENT, false)
                    .on_hover_text("Зажмите ЛКМ для перемещения окна");
                if drag_btn.is_pointer_button_down_on() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
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

                            let msg_frame = egui::Frame::NONE
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
            // Кнопка скриншота (OCR) - камера
            let ocr_btn = icon_button(ui, egui::vec2(28.0, 28.0), "camera", Color32::from_rgb(45, 45, 50), false)
                .on_hover_text("Сделать скриншот и распознать текст");

            if ocr_btn.clicked() {
                if let Some(client) = dbus_client {
                    let client = client.clone();
                    tokio::spawn(async move {
                        let _ = client.trigger_ocr().await;
                    });
                }
            }

            // Кнопка голосового ввода (ASR) - микрофон
            let asr_color = if self.is_listening {
                Color32::from_rgb(16, 185, 129) // Green active
            } else {
                Color32::from_rgb(45, 45, 50)
            };
            let asr_btn = icon_button(ui, egui::vec2(28.0, 28.0), "mic", asr_color, self.is_listening)
                .on_hover_text("Голосовой ввод");

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
            let spacing = ui.spacing().item_spacing.x;
            let input_width = ui.available_width() - 28.0 - spacing;
            let text_edit = ui.add_sized(
                [input_width, 28.0],
                egui::TextEdit::singleline(&mut self.input_text)
                    .hint_text("Задать вопрос...")
            );

            // Кнопка отправки - бумажный самолетик
            let send_btn = icon_button(ui, egui::vec2(28.0, 28.0), "send", Color32::from_rgb(79, 70, 229), false);
            let send_clicked = send_btn.clicked();
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

/// Векторный рендеринг кнопок с иконками для обеспечения соответствия правилам
/// исключения эмодзи (agent.md) и обеспечения премиального дизайна.
fn icon_button(
    ui: &mut egui::Ui,
    size: Vec2,
    icon_type: &str,
    fill_color: Color32,
    active: bool,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    
    // Отрисовка фона кнопки
    let bg_color = if response.hovered() {
        Color32::from_rgb(60, 60, 65)
    } else {
        fill_color
    };
    
    ui.painter().rect_filled(rect, 4.0, bg_color);
    
    let stroke_color = if active {
        Color32::WHITE
    } else {
        Color32::from_rgb(200, 200, 205)
    };
    
    // Отрисовка векторной иконки на основе типа
    match icon_type {
        "gear" => {
            let center = rect.center();
            let r = rect.width() * 0.25;
            ui.painter().circle_stroke(center, r, egui::Stroke::new(1.5, stroke_color));
            for i in 0..8 {
                let angle = (i as f32) * std::f32::consts::TAU / 8.0;
                let start = center + egui::vec2(angle.cos(), angle.sin()) * r;
                let end = center + egui::vec2(angle.cos(), angle.sin()) * (r * 1.35);
                ui.painter().line_segment([start, end], egui::Stroke::new(1.5, stroke_color));
            }
        }
        "camera" => {
            let center = rect.center();
            let w = rect.width() * 0.5;
            let h = rect.height() * 0.35;
            let cam_rect = egui::Rect::from_center_size(center, egui::vec2(w, h));
            ui.painter().rect(cam_rect, 2.0, Color32::TRANSPARENT, egui::Stroke::new(1.5, stroke_color), egui::StrokeKind::Inside);
            ui.painter().circle_stroke(center, w * 0.25, egui::Stroke::new(1.5, stroke_color));
            
            // Вспышка/выступ камеры сверху
            let top_bit = egui::Rect::from_min_max(
                cam_rect.min + egui::vec2(w * 0.2, -h * 0.25),
                cam_rect.min + egui::vec2(w * 0.45, 0.0)
            );
            ui.painter().rect_filled(top_bit, 1.0, stroke_color);
        }
        "mic" => {
            let center = rect.center();
            let w = rect.width() * 0.22;
            let h = rect.height() * 0.38;
            let mic_rect = egui::Rect::from_center_size(center - egui::vec2(0.0, h * 0.1), egui::vec2(w, h));
            ui.painter().rect_filled(mic_rect, w * 0.5, stroke_color);
            
            // Подставка микрофона (U-образная дуга)
            let cup_r = w * 1.4;
            let cup_center = center + egui::vec2(0.0, h * 0.05);
            let left_top = cup_center + egui::vec2(-cup_r, -h * 0.2);
            let left_bottom = cup_center + egui::vec2(-cup_r, 0.0);
            let right_top = cup_center + egui::vec2(cup_r, -h * 0.2);
            let right_bottom = cup_center + egui::vec2(cup_r, 0.0);
            ui.painter().line_segment([left_top, left_bottom], egui::Stroke::new(1.5, stroke_color));
            ui.painter().line_segment([right_top, right_bottom], egui::Stroke::new(1.5, stroke_color));
            
            // Полукруглая часть дуги подставки
            ui.painter().circle_stroke(cup_center, cup_r, egui::Stroke::new(1.5, stroke_color));
            // Очищаем верхнюю половину дуги, рисуя U-образно
            // (В egui для простоты можно нарисовать полукруг линией, либо оставить круглую рамку)
            
            // Ножка и основание подставки
            ui.painter().line_segment([cup_center + egui::vec2(0.0, cup_r), cup_center + egui::vec2(0.0, h * 0.55)], egui::Stroke::new(1.5, stroke_color));
            ui.painter().line_segment(
                [cup_center + egui::vec2(-cup_r, h * 0.55), cup_center + egui::vec2(cup_r, h * 0.55)],
                egui::Stroke::new(1.5, stroke_color)
            );
        }
        "send" => {
            let center = rect.center();
            let size = rect.width() * 0.35;
            let p1 = center + egui::vec2(size, 0.0);
            let p2 = center + egui::vec2(-size, -size * 0.8);
            let p3 = center + egui::vec2(-size * 0.3, 0.0);
            let p4 = center + egui::vec2(-size, size * 0.8);
            ui.painter().line_segment([p1, p2], egui::Stroke::new(1.5, stroke_color));
            ui.painter().line_segment([p2, p3], egui::Stroke::new(1.5, stroke_color));
            ui.painter().line_segment([p3, p1], egui::Stroke::new(1.5, stroke_color));
            ui.painter().line_segment([p3, p4], egui::Stroke::new(1.5, stroke_color));
            ui.painter().line_segment([p4, p1], egui::Stroke::new(1.5, stroke_color));
        }
        "pin" => {
            let center = rect.center();
            let size = rect.width() * 0.3;
            if active {
                // Иголка вертикально (закреплено)
                let head = egui::Rect::from_center_size(center - egui::vec2(0.0, size * 0.6), egui::vec2(size * 1.2, size * 0.3));
                let body = egui::Rect::from_center_size(center - egui::vec2(0.0, size * 0.2), egui::vec2(size * 0.6, size * 0.5));
                ui.painter().rect_filled(head, 1.0, stroke_color);
                ui.painter().rect_filled(body, 1.0, stroke_color);
                ui.painter().line_segment([center, center + egui::vec2(0.0, size * 0.7)], egui::Stroke::new(2.0, stroke_color));
            } else {
                // Иголка наклонена (откреплено)
                let angle = -std::f32::consts::FRAC_PI_4;
                let rotate = |p: Vec2| -> Vec2 {
                    egui::vec2(p.x * angle.cos() - p.y * angle.sin(), p.x * angle.sin() + p.y * angle.cos())
                };
                let head_center = center + rotate(egui::vec2(0.0, -size * 0.6));
                let body_center = center + rotate(egui::vec2(0.0, -size * 0.2));
                
                let head = egui::Rect::from_center_size(head_center, egui::vec2(size * 1.2, size * 0.3));
                let body = egui::Rect::from_center_size(body_center, egui::vec2(size * 0.6, size * 0.5));
                
                ui.painter().rect_filled(head, 1.0, stroke_color);
                ui.painter().rect_filled(body, 1.0, stroke_color);
                ui.painter().line_segment([center, center + rotate(egui::vec2(0.0, size * 0.7))], egui::Stroke::new(2.0, stroke_color));
            }
        }
        "drag" => {
            let center = rect.center();
            let w = rect.width() * 0.3;
            for i in -1..=1 {
                let y = center.y + (i as f32) * 4.0;
                ui.painter().line_segment(
                    [egui::pos2(center.x - w, y), egui::pos2(center.x + w, y)],
                    egui::Stroke::new(1.5, stroke_color)
                );
            }
        }
        "close" => {
            let center = rect.center();
            let size = rect.width() * 0.22;
            ui.painter().line_segment(
                [center - egui::vec2(size, size), center + egui::vec2(size, size)],
                egui::Stroke::new(1.5, stroke_color)
            );
            ui.painter().line_segment(
                [center - egui::vec2(size, -size), center + egui::vec2(size, -size)],
                egui::Stroke::new(1.5, stroke_color)
            );
        }
        _ => {}
    }
    
    response
}
