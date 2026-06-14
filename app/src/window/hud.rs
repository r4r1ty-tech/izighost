use crate::dbus::{DaemonClient, DaemonSignal};
use crate::window::theme;
use eframe::egui;
use eframe::egui::{Color32, RichText, Stroke, Vec2};
use std::sync::Arc;
use tokio::sync::mpsc::Sender;
use crate::window::preferences::GuiEvent;

pub struct HudState {
    pub input_text: String,
    pub chat_messages: Vec<(String, String)>, // (role, message)
    pub is_generating: bool,
    pub is_listening: bool,
    pub active_profile_name: String,
    pub show_preferences: bool,
    pub is_pinned: bool,
    pub show_extension_warning: bool,
    pub active_ocr_task: Option<tokio::task::JoinHandle<()>>,
    pub active_asr_task: Option<tokio::task::JoinHandle<()>>,
    pub active_chat_task: Option<tokio::task::JoinHandle<()>>,
    pub is_cursor_on_virtual: bool,
    pub attached_screenshot_path: Option<String>,
    pub screenshot_texture: Option<egui::TextureHandle>,
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
            active_ocr_task: None,
            active_asr_task: None,
            active_chat_task: None,
            is_cursor_on_virtual: false,
            attached_screenshot_path: None,
            screenshot_texture: None,
        }
    }

    /// Обработка входящих D-Bus сигналов
    pub fn handle_dbus_signal(&mut self, signal: DaemonSignal, ctx: &egui::Context) {
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
                tracing::info!("GUI: Получен D-Bus сигнал OcrCompleted. Текст: {:?}", text);
                self.is_generating = false;
                self.input_text = text;
            }
            DaemonSignal::AsrCompleted(text) => {
                tracing::info!("GUI: Получен D-Bus сигнал AsrCompleted. Текст: {:?}", text);
                self.is_listening = false;
                self.input_text = text;
            }
            DaemonSignal::ErrorOccurred(msg) => {
                tracing::error!("GUI: Получен D-Bus сигнал ErrorOccurred: {}", msg);
                self.is_generating = false;
                self.is_listening = false;
                self.chat_messages
                    .push(("system".to_string(), format!("Ошибка: {}", msg)));
            }
            DaemonSignal::ScreenshotCaptured(filepath) => {
                tracing::info!("GUI: Получен D-Bus сигнал ScreenshotCaptured: {}", filepath);
                self.set_attached_screenshot(filepath, ctx);
            }
        }
    }

    pub fn set_attached_screenshot(&mut self, path: String, ctx: &egui::Context) {
        self.screenshot_texture = load_texture_from_path(ctx, &path);
        self.attached_screenshot_path = Some(path);
        self.is_generating = false;
    }


    /// Обработка вставки изображений из буфера обмена (Ctrl+V) и drag-and-drop файлов
    fn handle_image_inputs(&mut self, ui: &mut egui::Ui, _dbus_client: &Option<Arc<DaemonClient>>) {
        // 1. Проверяем Ctrl+V (вставка из буфера обмена)
        let ctrl_v = ui.input(|i| i.key_pressed(egui::Key::V) && i.modifiers.command);
        if ctrl_v {
            tracing::info!("GUI: Нажата комбинация Ctrl+V для вставки изображения...");
            if let Ok(mut clipboard) = arboard::Clipboard::new() {
                match clipboard.get_image() {
                    Ok(image_data) => {
                        match save_clipboard_image(image_data) {
                            Ok(temp_path) => {
                                let path_str = temp_path.to_string_lossy().to_string();
                                tracing::info!("GUI: Буфер обмена сохранен во временный файл: {}", path_str);
                                self.screenshot_texture = load_texture_from_path(ui.ctx(), &path_str);
                                self.attached_screenshot_path = Some(path_str);
                            }
                            Err(e) => {
                                tracing::error!("GUI: Ошибка сохранения изображения из буфера обмена: {:?}", e);
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!("GUI: В буфере обмена нет изображения: {:?}", e);
                    }
                }
            } else {
                tracing::error!("GUI: Не удалось инициализировать доступ к буферу обмена");
            }
        }

        // 2. Проверяем Drag-and-Drop файлов
        let dropped_files = ui.input(|i| i.raw.dropped_files.clone());
        if !dropped_files.is_empty() {
            for file in dropped_files {
                if let Some(ref path) = file.path {
                    match copy_to_temp(path.clone()) {
                        Ok(temp_path) => {
                            let path_str = temp_path.to_string_lossy().to_string();
                            tracing::info!("GUI: Перетащенный файл скопирован в: {}", path_str);
                            self.screenshot_texture = load_texture_from_path(ui.ctx(), &path_str);
                            self.attached_screenshot_path = Some(path_str);
                        }
                        Err(e) => {
                            tracing::error!("GUI: Ошибка копирования перетащенного файла: {:?}", e);
                        }
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
        gui_event_tx: Sender<GuiEvent>,
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
            self.draw_input_bar(ui, dbus_client, gui_event_tx);
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

                // Парковка курсора
                let cursor_tip = if self.is_cursor_on_virtual {
                    "Вернуть курсор на основной экран"
                } else {
                    "Запарковать курсор на виртуальном мониторе"
                };
                if header_btn(ui, "\u{1F5B1}", cursor_tip, self.is_cursor_on_virtual).clicked() {
                    self.is_cursor_on_virtual = !self.is_cursor_on_virtual;
                    if let Some(ref client) = dbus_client {
                        let client_clone = client.clone();
                        let on_virtual = self.is_cursor_on_virtual;
                        tokio::spawn(async move {
                            if on_virtual {
                                if let Err(e) = client_clone.save_cursor_position().await {
                                    eprintln!("Ошибка сохранения позиции курсора: {:?}", e);
                                }
                                if let Err(e) = client_clone.warp_to_virtual_monitor().await {
                                    eprintln!("Ошибка перемещения курсора на виртуальный монитор: {:?}", e);
                                }
                            } else {
                                if let Err(e) = client_clone.restore_cursor_position().await {
                                    eprintln!("Ошибка восстановления позиции курсора: {:?}", e);
                                }
                            }
                        });
                    }
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

    fn draw_chat_history(&mut self, ui: &mut egui::Ui) {
        let mut extra_offset = 48.0;
        if self.screenshot_texture.is_some() {
            extra_offset += 72.0;
        }
        let height = ui.available_height() - extra_offset;

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
    fn draw_input_bar(
        &mut self,
        ui: &mut egui::Ui,
        dbus_client: &Option<Arc<DaemonClient>>,
        gui_event_tx: Sender<GuiEvent>,
    ) {
        let mut remove_screenshot = false;
        if let Some(ref texture) = self.screenshot_texture {
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    let size = texture.size_vec2();
                    let aspect_ratio = size.x / size.y;
                    let height = 60.0;
                    let width = height * aspect_ratio;
                    
                    // Рисуем картинку с помощью egui::Image
                    ui.add(egui::Image::new(texture).max_height(height).max_width(width));

                    // Кнопка удаления прикрепленного скриншота
                    if ui.button("❌").on_hover_text("Удалить скриншот").clicked() {
                        remove_screenshot = true;
                    }
                });
                ui.add_space(4.0);
            });
        }

        if remove_screenshot {
            if let Some(ref path) = self.attached_screenshot_path {
                let _ = std::fs::remove_file(path);
            }
            self.attached_screenshot_path = None;
            self.screenshot_texture = None;
        }

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
                .on_hover_text("Сделать скриншот виртуального монитора");

            if ocr_btn.clicked() {
                tracing::info!("GUI: Нажата кнопка скриншота (фотоаппарат)");
                if let Some(client) = dbus_client {
                    let client = client.clone();
                    self.is_generating = true;
                    let tx = gui_event_tx.clone();
                    if let Some(task) = self.active_ocr_task.take() {
                        tracing::info!("GUI: Предыдущая OCR задача отменена.");
                        task.abort();
                    }
                    self.active_ocr_task = Some(tokio::spawn(async move {
                        tracing::info!("GUI: Вызов тихой скриншот-функции capture_virtual_screenshot() демона...");
                        match client.capture_virtual_screenshot().await {
                            Ok(path_str) => {
                                tracing::info!("GUI: Скриншот успешно получен: {}", path_str);
                                let _ = tx.send(GuiEvent::ScreenshotCaptured(path_str)).await;
                            }
                            Err(e) => {
                                tracing::warn!("GUI: Тихий скриншот не удался ({:?}). Запуск интерактивного портала скриншотов...", e);
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
                                                tracing::info!("GUI: Интерактивный скриншот сохранен в: {}", path_str);
                                                let _ = tx.send(GuiEvent::ScreenshotCaptured(path_str)).await;
                                            } else {
                                                let _ = tx.send(GuiEvent::Error("Не удалось получить локальный путь из URI скриншота".to_string())).await;
                                            }
                                        }
                                        Err(err) => {
                                            tracing::error!("GUI: Ошибка ответа Screenshot: {:?}", err);
                                            let _ = tx.send(GuiEvent::Error(format!("Ошибка ответа Screenshot: {:?}", err))).await;
                                        }
                                    },
                                    Err(err) => {
                                        tracing::error!("GUI: Ошибка запроса Screenshot: {:?}", err);
                                        let _ = tx.send(GuiEvent::Error(format!("Ошибка запроса Screenshot: {:?}", err))).await;
                                    }
                                }
                            }
                        }
                    }));
                } else {
                    tracing::warn!("GUI: Не удалось сделать скриншот, так как D-Bus клиент не подключен.");
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
                    if let Some(task) = self.active_asr_task.take() {
                        task.abort();
                    }
                    self.active_asr_task = Some(tokio::spawn(async move {
                        if was_listening {
                            let _ = client.stop_listening().await;
                        } else {
                            let _ = client.start_listening().await;
                        }
                    }));
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

            let has_input = !self.input_text.trim().is_empty();
            let has_screenshot = self.attached_screenshot_path.is_some();

            if is_stop {
                if btn_clicked {
                    self.is_generating = false;
                    if let Some(task) = self.active_chat_task.take() {
                        task.abort();
                    }
                    if let Some(task) = self.active_ocr_task.take() {
                        task.abort();
                    }
                    if let Some(task) = self.active_asr_task.take() {
                        task.abort();
                    }
                    if let Some(client) = dbus_client {
                        let client = client.clone();
                        tokio::spawn(async move {
                            let _ = client.cancel_generation().await;
                        });
                    }
                }
            } else if (btn_clicked || enter_pressed) && (has_input || has_screenshot) {
                let text = self.input_text.trim().to_string();
                self.input_text.clear();
                self.is_generating = true;

                tracing::info!("GUI: Отправка запроса пользователем. Текст: {:?}, Скриншот прикреплен: {}", text, has_screenshot);

                // Отображаем сообщение в истории чата
                let display_msg = if has_screenshot {
                    if text.is_empty() {
                        "[Отправлен скриншот]".to_string()
                    } else {
                        format!("{} [Скриншот]", text)
                    }
                } else {
                    text.clone()
                };
                self.chat_messages.push(("user".to_string(), display_msg));

                if let Some(client) = dbus_client {
                    let client = client.clone();
                    let attached_path = self.attached_screenshot_path.take();
                    self.screenshot_texture = None; // Очищаем превью

                    if let Some(task) = self.active_chat_task.take() {
                        tracing::info!("GUI: Отмена предыдущей фоновой задачи чата.");
                        task.abort();
                    }
                    self.active_chat_task = Some(tokio::spawn(async move {
                        tracing::info!("GUI: Запуск фоновой задачи для подготовки и отправки промпта...");
                        let mut final_prompt = text;
                        if let Some(path) = attached_path {
                            tracing::info!("GUI: Запуск фонового OCR для скриншота перед отправкой: {}", path);
                            match client.run_ocr_on_file(&path).await {
                                Ok(ocr_text) => {
                                    tracing::info!("GUI: OCR успешно выполнено. Символов распознано: {}", ocr_text.len());
                                    if final_prompt.is_empty() {
                                        final_prompt = format!("[Распознанный текст со скриншота]:\n{}", ocr_text);
                                    } else {
                                        final_prompt = format!("{}\n\n[Распознанный текст со скриншота]:\n{}", final_prompt, ocr_text);
                                    }
                                }
                                Err(e) => {
                                    tracing::error!("GUI: Ошибка распознавания прикрепленного скриншота: {:?}", e);
                                }
                            }
                            // Удаляем временный файл после отправки
                            tracing::info!("GUI: Удаление временного файла скриншота: {}", path);
                            let _ = std::fs::remove_file(path);
                        }

                        if !final_prompt.trim().is_empty() {
                            tracing::info!("GUI: Вызов D-Bus метода send_chat_message для отправки промпта в LLM...");
                            if let Err(e) = client.send_chat_message(&final_prompt).await {
                                tracing::error!("GUI: Ошибка отправки сообщения в чат LLM через D-Bus: {:?}", e);
                            }
                        } else {
                            tracing::warn!("GUI: Итоговый промпт пуст, отправка отменена.");
                        }
                    }));
                }
            }
        });
    }
}

fn load_texture_from_path(
    ctx: &egui::Context,
    path_str: &str,
) -> Option<egui::TextureHandle> {
    let path = std::path::Path::new(path_str);
    if let Ok(color_image) = load_image_from_path(path) {
        Some(ctx.load_texture(
            "screenshot-preview",
            color_image,
            egui::TextureOptions::default(),
        ))
    } else {
        None
    }
}

fn load_image_from_path(path: &std::path::Path) -> Result<egui::ColorImage, image::ImageError> {
    let image = image::open(path)?;
    let size = [image.width() as usize, image.height() as usize];
    let image_buffer = image.to_rgba8();
    let pixels = image_buffer.as_raw();
    Ok(egui::ColorImage::from_rgba_unmultiplied(
        size,
        pixels,
    ))
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
