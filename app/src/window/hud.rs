use crate::dbus::{DaemonClient, DaemonSignal};
use crate::window::theme;
use eframe::egui;
use eframe::egui::{Color32, RichText, Stroke, Vec2};
use std::sync::Arc;

pub struct HudState {
    pub input_text: String,
    pub chat_messages: Vec<(String, String)>, // (role, message)
    pub is_generating: bool,
    pub is_listening: bool,
    pub active_profile_name: String,
    pub show_preferences: bool,
    pub is_pinned: bool,
    pub show_extension_warning: bool,
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
            show_extension_warning: false,
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
                self.chat_messages
                    .push(("system".to_string(), format!("Ошибка: {}", msg)));
            }
        }
    }

    /// Обработка вставки изображений из буфера обмена (Ctrl+V) и drag-and-drop файлов
    fn handle_image_inputs(&mut self, ui: &mut egui::Ui, dbus_client: &Option<Arc<DaemonClient>>) {
        // 1. Проверяем Ctrl+V (вставка из буфера обмена)
        let ctrl_v = ui.input(|i| i.key_pressed(egui::Key::V) && i.modifiers.command);
        if ctrl_v {
            if let Ok(mut clipboard) = arboard::Clipboard::new() {
                if let Ok(image_data) = clipboard.get_image() {
                    if let Some(client) = dbus_client {
                        let client = client.clone();
                        self.is_generating = true;
                        tokio::spawn(async move {
                            match save_clipboard_image(image_data) {
                                Ok(temp_path) => {
                                    let path_str = temp_path.to_string_lossy().to_string();
                                    if let Err(e) = client.trigger_ocr_from_file(&path_str).await {
                                        eprintln!("Ошибка вызова trigger_ocr_from_file: {:?}", e);
                                        let _ = std::fs::remove_file(&temp_path);
                                    }
                                }
                                Err(e) => {
                                    eprintln!("Ошибка сохранения изображения из буфера: {:?}", e);
                                }
                            }
                        });
                    }
                }
            }
        }

        // 2. Проверяем Drag-and-Drop файлов
        let dropped_files = ui.input(|i| i.raw.dropped_files.clone());
        if !dropped_files.is_empty() {
            for file in dropped_files {
                if let Some(ref path) = file.path {
                    if let Some(client) = dbus_client {
                        let client = client.clone();
                        let original_path = path.clone();
                        self.is_generating = true;
                        tokio::spawn(async move {
                            match copy_to_temp(original_path) {
                                Ok(temp_path) => {
                                    let path_str = temp_path.to_string_lossy().to_string();
                                    if let Err(e) = client.trigger_ocr_from_file(&path_str).await {
                                        eprintln!("Ошибка вызова trigger_ocr_from_file: {:?}", e);
                                        let _ = std::fs::remove_file(&temp_path);
                                    }
                                }
                                Err(e) => {
                                    eprintln!(
                                        "Ошибка копирования файла во временную папку: {:?}",
                                        e
                                    );
                                }
                            }
                        });
                    }
                }
            }
        }
    }

    /// Отрисовка HUD интерфейса
    pub fn draw(
        &mut self,
        ui: &mut egui::Ui,
        dbus_client: &Option<Arc<DaemonClient>>,
        active_profile: &Option<String>,
    ) {
        self.handle_image_inputs(ui, dbus_client);

        ui.style_mut().spacing.item_spacing = Vec2::new(8.0, 8.0);

        // Обновляем имя активного профиля
        if let Some(profile_name) = active_profile {
            self.active_profile_name = profile_name.clone();
        } else {
            self.active_profile_name = "Не выбран".to_string();
        }

        // Цвет рамки зависит от состояния
        let border_color = if self.is_generating {
            theme::ACCENT
        } else if self.is_listening {
            theme::GREEN
        } else {
            theme::BORDER_SUBTLE
        };

        let frame = egui::Frame::NONE
            .fill(Color32::from_rgba_unmultiplied(20, 20, 22, 240))
            .stroke(Stroke::new(1.5, border_color))
            .inner_margin(12.0)
            .corner_radius(12.0);

        frame.show(ui, |ui| {
            self.draw_header(ui, dbus_client);
            if self.show_extension_warning {
                ui.add_space(4.0);
                self.draw_extension_warning(ui);
            }
            ui.separator();
            self.draw_chat_history(ui);
            ui.separator();
            self.draw_input_bar(ui, dbus_client);
        });
    }

    /// Заголовок HUD
    fn draw_header(&mut self, ui: &mut egui::Ui, dbus_client: &Option<Arc<DaemonClient>>) {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("IziGhost")
                    .strong()
                    .size(14.0)
                    .color(theme::TEXT_PRIMARY),
            );

            ui.add_space(4.0);

            // Бейдж активного профиля
            let badge_frame = egui::Frame::NONE
                .fill(theme::BG_CARD)
                .inner_margin(Vec2::new(6.0, 2.0))
                .corner_radius(4.0);

            badge_frame.show(ui, |ui| {
                ui.label(
                    RichText::new(&self.active_profile_name)
                        .size(11.0)
                        .color(theme::GREEN),
                );
            });

            // Кнопки управления (справа налево)
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Выход
                if header_btn(ui, "x", "Закрыть приложение", false).clicked() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }

                // Настройки
                if header_btn(ui, "\u{2699}", "Настройки профилей", false).clicked()
                {
                    self.show_preferences = !self.show_preferences;
                }

                // Закрепить/Открепить
                let pin_tip = if self.is_pinned {
                    "Открепить от экрана"
                } else {
                    "Закрепить поверх всех окон"
                };
                if header_btn(ui, "\u{1F4CC}", pin_tip, self.is_pinned).clicked() {
                    self.is_pinned = !self.is_pinned;
                    let level = if self.is_pinned {
                        egui::WindowLevel::AlwaysOnTop
                    } else {
                        egui::WindowLevel::Normal
                    };
                    ui.ctx()
                        .send_viewport_cmd(egui::ViewportCommand::WindowLevel(level));

                    if let Some(ref client) = dbus_client {
                        let client_clone = client.clone();
                        let is_pinned = self.is_pinned;
                        tokio::spawn(async move {
                            let pid = std::process::id();
                            let res = if is_pinned {
                                client_clone.pin_window_by_pid(pid).await
                            } else {
                                client_clone.unpin_window_by_pid(pid).await
                            };
                            if res.is_err() {
                                // Ошибка pinning — предупреждение показывается в UI баннере
                            }
                        });
                    }
                }

                // Перетаскивание
                let drag_resp = header_btn(ui, "\u{2630}", "Зажмите для перемещения окна", false);
                if drag_resp.is_pointer_button_down_on() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }
            });
        });
    }

    /// Отрисовка предупреждения о необходимости перезапуска сессии
    fn draw_extension_warning(&mut self, ui: &mut egui::Ui) {
        let frame = egui::Frame::NONE
            .fill(theme::WARN_BG)
            .inner_margin(8.0)
            .corner_radius(6.0);

        frame.show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("Для Always-On-Top перезайдите в систему (Log Out).")
                        .size(11.0)
                        .color(Color32::from_rgb(240, 240, 240)),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add(egui::Button::new(RichText::new("x").size(11.0)).frame(false))
                        .clicked()
                    {
                        self.show_extension_warning = false;
                    }
                });
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
                        ui.label(
                            RichText::new("Ассистент готов к работе.").color(theme::TEXT_MUTED),
                        );
                        ui.label(
                            RichText::new("Задайте вопрос текстом, скриншотом или голосом.")
                                .size(11.0)
                                .color(theme::TEXT_HINT),
                        );
                    });
                } else {
                    for (role, text) in &self.chat_messages {
                        let is_user = role == "user";
                        let is_system = role == "system";

                        let align = if is_user {
                            egui::Align::Max
                        } else {
                            egui::Align::Min
                        };

                        ui.with_layout(egui::Layout::top_down(align), |ui| {
                            let bg = if is_user {
                                theme::ACCENT
                            } else if is_system {
                                theme::RED_SOFT
                            } else {
                                theme::BG_CARD
                            };

                            egui::Frame::NONE
                                .fill(bg)
                                .inner_margin(8.0)
                                .corner_radius(8.0)
                                .show(ui, |ui| {
                                    ui.label(RichText::new(text).color(theme::TEXT_PRIMARY));
                                });
                        });
                        ui.add_space(4.0);
                    }
                }

                if self.is_generating {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("IziGhost печатает...")
                                .italics()
                                .color(theme::TEXT_MUTED),
                        );
                    });
                }
            });
    }

    /// Панель ввода с кнопками OCR, ASR и отправки
    fn draw_input_bar(&mut self, ui: &mut egui::Ui, dbus_client: &Option<Arc<DaemonClient>>) {
        ui.horizontal(|ui| {
            // Кнопка скриншота (OCR)
            let ocr_btn = ui
                .add(
                    egui::Button::new(
                        RichText::new("\u{1F4F7}")
                            .size(16.0)
                            .color(theme::TEXT_SECONDARY),
                    )
                    .fill(theme::BG_BUTTON)
                    .corner_radius(6.0)
                    .min_size(egui::vec2(28.0, 28.0)),
                )
                .on_hover_text("Выделить область экрана и распознать текст");

            if ocr_btn.clicked() {
                if let Some(client) = dbus_client {
                    let client = client.clone();
                    self.is_generating = true;
                    tokio::spawn(async move {
                        use ashpd::desktop::screenshot::Screenshot;
                        match Screenshot::request()
                            .interactive(true)
                            .modal(true)
                            .send()
                            .await
                        {
                            Ok(request) => match request.response() {
                                Ok(response) => {
                                    let uri = response.uri();
                                    if let Ok(path) = uri.to_file_path() {
                                        let path_str = path.to_string_lossy().to_string();
                                        if let Err(e) =
                                            client.trigger_ocr_from_file(&path_str).await
                                        {
                                            eprintln!(
                                                "Ошибка вызова trigger_ocr_from_file: {:?}",
                                                e
                                            );
                                        }
                                    } else {
                                        eprintln!(
                                            "Не удалось получить локальный путь из URI: {:?}",
                                            uri
                                        );
                                    }
                                }
                                Err(e) => {
                                    eprintln!("Ошибка ответа Screenshot: {:?}", e);
                                }
                            },
                            Err(e) => {
                                eprintln!("Ошибка запроса Screenshot: {:?}", e);
                            }
                        }
                    });
                }
            }

            // Кнопка голосового ввода (ASR)
            let mic_bg = if self.is_listening {
                theme::GREEN
            } else {
                theme::BG_BUTTON
            };
            let asr_btn = ui
                .add(
                    egui::Button::new(
                        RichText::new("\u{1F3A4}")
                            .size(16.0)
                            .color(theme::TEXT_SECONDARY),
                    )
                    .fill(mic_bg)
                    .corner_radius(6.0)
                    .min_size(egui::vec2(28.0, 28.0)),
                )
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
                egui::TextEdit::singleline(&mut self.input_text).hint_text("Задать вопрос..."),
            );

            // Кнопка отправки или остановки печати в зависимости от состояния генерации
            let (btn_text, btn_color, is_stop) = if self.is_generating {
                ("■", theme::RED_SOFT, true)
            } else {
                ("▶", theme::ACCENT, false)
            };

            let action_btn = ui.add(
                egui::Button::new(
                    RichText::new(btn_text)
                        .size(14.0)
                        .color(theme::TEXT_PRIMARY),
                )
                .fill(btn_color)
                .corner_radius(6.0)
                .min_size(egui::vec2(28.0, 28.0)),
            );

            let btn_clicked = action_btn.clicked();
            let enter_pressed =
                text_edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));

            if is_stop {
                if btn_clicked {
                    self.is_generating = false;
                    if let Some(client) = dbus_client {
                        let client = client.clone();
                        tokio::spawn(async move {
                            let _ = client.cancel_generation().await;
                        });
                    }
                }
            } else if (btn_clicked || enter_pressed) && !self.input_text.trim().is_empty() {
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

/// Кнопка заголовка HUD (текстовая иконка)
fn header_btn(ui: &mut egui::Ui, icon: &str, tooltip: &str, active: bool) -> egui::Response {
    let color = if active {
        theme::TEXT_PRIMARY
    } else {
        theme::TEXT_SECONDARY
    };

    ui.add(egui::Button::new(RichText::new(icon).size(15.0).color(color)).frame(false))
        .on_hover_text(tooltip)
}

/// Сохраняет изображение из буфера обмена во временный файл PNG.
fn save_clipboard_image(
    image_data: arboard::ImageData,
) -> Result<std::path::PathBuf, anyhow::Error> {
    use image::{ImageBuffer, Rgba};
    let width = image_data.width as u32;
    let height = image_data.height as u32;
    let bytes = image_data.bytes.into_owned();

    let buffer: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_raw(width, height, bytes)
        .ok_or_else(|| anyhow::anyhow!("Не удалось создать буфер изображения из буфера обмена"))?;

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let temp_path = std::env::temp_dir().join(format!("izighost_clipboard_{}.png", timestamp));

    buffer.save(&temp_path)?;
    Ok(temp_path)
}

/// Копирует перетащенный файл во временную директорию перед отправкой на OCR.
fn copy_to_temp(original_path: std::path::PathBuf) -> Result<std::path::PathBuf, anyhow::Error> {
    let extension = original_path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("png");

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let temp_path = std::env::temp_dir().join(format!("izighost_drop_{}.{}", timestamp, extension));

    std::fs::copy(&original_path, &temp_path)?;
    Ok(temp_path)
}
