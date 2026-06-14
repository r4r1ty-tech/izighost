use crate::dbus::DaemonClient;
use crate::window::theme;
use eframe::egui;
use eframe::egui::{RichText, Vec2};
use izighost_common::{KeyringStore, Profile};
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;

/// События, отправляемые из фоновых асинхронных задач в главный поток GUI
#[derive(Debug, Clone)]
pub enum GuiEvent {
    ProfilesLoaded(Vec<String>),
    ActiveProfileLoaded(Profile),
    ProfileDetailsLoaded(Profile, Option<String>, Option<String>, Option<String>), // profile, llm_key, asr_key, vision_key
    ProfileSaved(Profile),
    ProfileDeleted(String),
    RvmsStarted(u32),
    RvmsStopped,
    Error(String),
    ExtensionNotLoaded,
}

pub struct PreferencesState {
    pub profiles: Vec<String>,
    pub selected_id: Option<String>,
    pub active_id: Option<String>,

    // Форма редактирования текущего профиля
    pub edit_profile: Option<Profile>,
    pub llm_key_input: String,
    pub asr_key_input: String,
    pub vision_key_input: String,
    pub show_llm_key: bool,
    pub show_asr_key: bool,
    pub show_vision_key: bool,

    // Менеджмент ошибок и уведомлений
    pub status_message: Option<(String, bool)>, // (текст, это_ошибка)
    pub is_rvms_active: bool,
    pub pipewire_node_id: Option<u32>,

    // Канал для отправки событий из фоновых задач
    pub event_tx: UnboundedSender<GuiEvent>,
}

impl PreferencesState {
    pub fn new(event_tx: UnboundedSender<GuiEvent>) -> Self {
        Self {
            profiles: Vec::new(),
            selected_id: None,
            active_id: None,
            edit_profile: None,
            llm_key_input: String::new(),
            asr_key_input: String::new(),
            vision_key_input: String::new(),
            show_llm_key: false,
            show_asr_key: false,
            show_vision_key: false,
            status_message: None,
            is_rvms_active: false,
            pipewire_node_id: None,
            event_tx,
        }
    }

    /// Первичная инициализация данных
    pub fn init(&self, dbus_client: &Option<Arc<DaemonClient>>) {
        if let Some(client) = dbus_client {
            let client = client.clone();
            let tx = self.event_tx.clone();
            tokio::spawn(async move {
                match client.list_profiles().await {
                    Ok(list) => {
                        let _ = tx.send(GuiEvent::ProfilesLoaded(list));
                    }
                    Err(e) => {
                        let _ =
                            tx.send(GuiEvent::Error(format!("Ошибка загрузки профилей: {}", e)));
                    }
                }

                match client.get_active_profile().await {
                    Ok(active) => {
                        if !active.id.is_empty() {
                            let _ = tx.send(GuiEvent::ActiveProfileLoaded(active));
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(GuiEvent::Error(format!(
                            "Ошибка получения активного профиля: {}",
                            e
                        )));
                    }
                }
            });
        }
    }

    /// Обработка входящего GuiEvent
    pub fn handle_event(&mut self, event: GuiEvent, dbus_client: &Option<Arc<DaemonClient>>) {
        match event {
            GuiEvent::ProfilesLoaded(list) => {
                self.profiles = list;
                if self.selected_id.is_none() && !self.profiles.is_empty() {
                    let first = self.profiles[0].clone();
                    self.load_profile_details(first, dbus_client);
                }
            }
            GuiEvent::ActiveProfileLoaded(active) => {
                self.active_id = Some(active.id);
            }
            GuiEvent::ProfileDetailsLoaded(profile, llm_key, asr_key, vision_key) => {
                self.selected_id = Some(profile.id.clone());
                self.llm_key_input = llm_key.unwrap_or_default();
                self.asr_key_input = asr_key.unwrap_or_default();
                self.vision_key_input = vision_key.unwrap_or_default();
                self.edit_profile = Some(profile);
                self.show_llm_key = false;
                self.show_asr_key = false;
                self.show_vision_key = false;
            }
            GuiEvent::ProfileSaved(profile) => {
                self.status_message = Some(("Профиль успешно сохранен!".to_string(), false));

                // Перезагружаем список
                if !self.profiles.contains(&profile.id) {
                    self.profiles.push(profile.id.clone());
                    self.profiles.sort();
                }
                self.selected_id = Some(profile.id.clone());
                self.edit_profile = Some(profile);
            }
            GuiEvent::ProfileDeleted(id) => {
                self.status_message = Some((format!("Профиль '{}' удален", id), false));
                self.profiles.retain(|x| x != &id);
                if self.active_id.as_ref() == Some(&id) {
                    self.active_id = None;
                }
                if self.selected_id.as_ref() == Some(&id) {
                    self.selected_id = None;
                    self.edit_profile = None;
                    if !self.profiles.is_empty() {
                        let first = self.profiles[0].clone();
                        self.load_profile_details(first, dbus_client);
                    }
                }
            }
            GuiEvent::RvmsStarted(node_id) => {
                self.is_rvms_active = true;
                self.pipewire_node_id = Some(node_id);
                self.status_message = Some((
                    format!("Виртуальный монитор запущен (PW Node ID: {})", node_id),
                    false,
                ));
            }
            GuiEvent::RvmsStopped => {
                self.is_rvms_active = false;
                self.pipewire_node_id = None;
                self.status_message = Some(("Виртуальный монитор остановлен".to_string(), false));
            }
            GuiEvent::Error(msg) => {
                self.status_message = Some((msg, true));
            }
            GuiEvent::ExtensionNotLoaded => {}
        }
    }

    /// Асинхронная загрузка профиля и его ключей из Keyring
    fn load_profile_details(&self, id: String, dbus_client: &Option<Arc<DaemonClient>>) {
        if let Some(client) = dbus_client {
            let client = client.clone();
            let tx = self.event_tx.clone();
            tokio::spawn(async move {
                match client.get_profile(&id).await {
                    Ok(profile) => {
                        let llm_key_name = format!("llm_api_key_{}", id);
                        let asr_key_name = format!("asr_api_key_{}", id);
                        let vision_key_name = format!("vision_api_key_{}", id);

                        let llm_key = KeyringStore::get_password(&llm_key_name)
                            .await
                            .unwrap_or(None);
                        let asr_key = KeyringStore::get_password(&asr_key_name)
                            .await
                            .unwrap_or(None);
                        let vision_key = KeyringStore::get_password(&vision_key_name)
                            .await
                            .unwrap_or(None);

                        let _ = tx.send(GuiEvent::ProfileDetailsLoaded(profile, llm_key, asr_key, vision_key));
                    }
                    Err(e) => {
                        let _ = tx.send(GuiEvent::Error(format!(
                            "Ошибка загрузки профиля '{}': {}",
                            id, e
                        )));
                    }
                }
            });
        }
    }

    /// Рендеринг интерфейса настроек
    pub fn draw(&mut self, ui: &mut egui::Ui, dbus_client: &Option<Arc<DaemonClient>>) {
        ui.style_mut().spacing.item_spacing = Vec2::new(8.0, 8.0);

        let sidebar_frame = egui::Frame::NONE
            .fill(theme::BG_SIDEBAR)
            .inner_margin(10.0)
            .corner_radius(8.0);

        egui::Panel::left("sidebar_panel")
            .resizable(false)
            .default_size(210.0)
            .frame(sidebar_frame)
            .show_inside(ui, |ui| {
                self.draw_sidebar(ui, dbus_client);
            });

        let main_frame = egui::Frame::NONE.fill(theme::BG_PRIMARY).inner_margin(16.0);

        egui::CentralPanel::default()
            .frame(main_frame)
            .show_inside(ui, |ui| {
                self.draw_main_panel(ui, dbus_client);
            });
    }

    /// Левая колонка (Список профилей + RVMS управление)
    fn draw_sidebar(&mut self, ui: &mut egui::Ui, dbus_client: &Option<Arc<DaemonClient>>) {
        ui.vertical(|ui| {
            ui.add_space(4.0);
            theme::section_heading(ui, "ПРОФИЛИ");

            egui::ScrollArea::vertical()
                .max_height(280.0)
                .show(ui, |ui| {
                    for id in &self.profiles {
                        let is_selected = Some(id.clone()) == self.selected_id;
                        let is_active = Some(id.clone()) == self.active_id;

                        let text = if is_active {
                            format!("\u{2022} {}", id) // Зелёная точка перед активным
                        } else {
                            id.clone()
                        };

                        let text_color = if is_active {
                            theme::GREEN
                        } else {
                            theme::TEXT_PRIMARY
                        };

                        let btn_color = if is_selected {
                            theme::ACCENT
                        } else {
                            theme::BG_BUTTON
                        };

                        let btn = ui.add_sized(
                            [ui.available_width(), 30.0],
                            egui::Button::new(RichText::new(text).size(12.0).color(text_color))
                                .fill(btn_color)
                                .corner_radius(6.0),
                        );

                        if btn.clicked() {
                            self.load_profile_details(id.clone(), dbus_client);
                        }
                        ui.add_space(2.0);
                    }
                });

            ui.add_space(8.0);

            // Кнопка создания нового профиля
            if theme::green_button(ui, "+ Новый профиль", ui.available_width()).clicked()
            {
                let random_id = format!(
                    "profile_{}",
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .expect("System time is set before Unix Epoch")
                        .as_millis()
                        % 10000
                );
                let new_profile = Profile {
                    id: random_id,
                    name: "Новый профиль".to_string(),
                    ..Default::default()
                };
                self.edit_profile = Some(new_profile);
                self.selected_id = None;
                self.llm_key_input.clear();
                self.asr_key_input.clear();
            }

            theme::spaced_separator(ui);

            // Панель управления RVMS
            theme::section_heading(ui, "ВИРТУАЛЬНЫЙ ЭКРАН");

            let status_text = if self.is_rvms_active {
                format!("Активен (PW: {})", self.pipewire_node_id.unwrap_or(0))
            } else {
                "Отключен".to_string()
            };

            theme::status_indicator(ui, "Статус:", &status_text, self.is_rvms_active);

            ui.add_space(6.0);

            if self.is_rvms_active {
                if theme::danger_button(ui, "Остановить RVMS", ui.available_width()).clicked()
                {
                    if let Some(client) = dbus_client {
                        let client = client.clone();
                        let tx = self.event_tx.clone();
                        tokio::spawn(async move {
                            match client.stop_rvms().await {
                                Ok(_) => {
                                    let _ = tx.send(GuiEvent::RvmsStopped);
                                }
                                Err(e) => {
                                    let _ = tx.send(GuiEvent::Error(format!(
                                        "Ошибка остановки RVMS: {}",
                                        e
                                    )));
                                }
                            }
                        });
                    }
                }
            } else if theme::accent_button(ui, "Запустить RVMS", ui.available_width()).clicked()
            {
                if let Some(client) = dbus_client {
                    let client = client.clone();
                    let tx = self.event_tx.clone();
                    tokio::spawn(async move {
                        match client.start_rvms().await {
                            Ok(node_id) => {
                                let _ = tx.send(GuiEvent::RvmsStarted(node_id));
                            }
                            Err(e) => {
                                let _ =
                                    tx.send(GuiEvent::Error(format!("Ошибка запуска RVMS: {}", e)));
                            }
                        }
                    });
                }
            }
        });
    }

    /// Правая колонка — детальное редактирование профиля
    fn draw_main_panel(&mut self, ui: &mut egui::Ui, dbus_client: &Option<Arc<DaemonClient>>) {
        // Вывод системных сообщений / уведомлений
        let mut clear_message = false;
        if let Some((msg, is_error)) = &self.status_message {
            let is_err = *is_error;
            let msg_clone = msg.clone();
            theme::notification_frame(is_err).show(ui, |ui| {
                ui.horizontal(|ui| {
                    let color = if is_err { theme::RED } else { theme::GREEN };

                    // Ограничиваем ширину текста и включаем автоперенос, чтобы не выдавливать кнопку закрытия
                    let label =
                        egui::Label::new(RichText::new(msg_clone).color(color).strong()).wrap();
                    ui.add_sized([ui.available_width() - 24.0, 0.0], label);

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new("x").size(11.0).color(theme::TEXT_SECONDARY),
                                )
                                .frame(false),
                            )
                            .clicked()
                        {
                            clear_message = true;
                        }
                    });
                });
            });
            ui.add_space(4.0);
        }
        if clear_message {
            self.status_message = None;
        }

        let Some(ref mut profile) = self.edit_profile else {
            ui.centered_and_justified(|ui| {
                ui.label(
                    RichText::new("Выберите профиль слева или создайте новый.")
                        .color(theme::TEXT_MUTED),
                );
            });
            return;
        };

        // Разделяем заимствования `self` для замыкания `ScrollArea::show`
        let llm_key_input = &mut self.llm_key_input;
        let asr_key_input = &mut self.asr_key_input;
        let vision_key_input = &mut self.vision_key_input;
        let show_llm_key = &mut self.show_llm_key;
        let show_asr_key = &mut self.show_asr_key;
        let show_vision_key = &mut self.show_vision_key;
        let active_id = &self.active_id;
        let event_tx = self.event_tx.clone();

        egui::ScrollArea::vertical().show(ui, |ui| {
            // Заголовок профиля
            ui.label(
                RichText::new(format!("Профиль: {}", profile.id))
                    .strong()
                    .size(16.0)
                    .color(theme::TEXT_PRIMARY),
            );
            ui.add_space(12.0);

            // ── Основное ──
            theme::section_frame().show(ui, |ui| {
                theme::section_title(ui, "Основные настройки");
                theme::form_row(ui, "Имя профиля:", &mut profile.name);
                ui.add_space(6.0);

                ui.label(RichText::new("Системный промпт:").color(theme::TEXT_SECONDARY));
                ui.add(
                    egui::TextEdit::multiline(&mut profile.system_prompt)
                        .hint_text("Ты senior-ментор...")
                        .desired_rows(4)
                        .desired_width(ui.available_width()),
                );
            });

            ui.add_space(10.0);

            // ── Документы ──
            theme::section_frame().show(ui, |ui| {
                theme::section_title(ui, "Документы кандидата");

                ui.horizontal(|ui| {
                    ui.add_sized(
                        [120.0, 20.0],
                        egui::Label::new(
                            RichText::new("Резюме (CV):").color(theme::TEXT_SECONDARY),
                        ),
                    );
                    ui.add(
                        egui::TextEdit::singleline(&mut profile.cv_path)
                            .desired_width(ui.available_width() - 110.0),
                    );
                    if ui.add(egui::Button::new("Обзор...").fill(theme::BG_BUTTON).corner_radius(4.0)).clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("Документы (*.pdf, *.txt, *.md)", &["pdf", "txt", "md"])
                            .pick_file()
                        {
                            profile.cv_path = path.to_string_lossy().to_string();
                        }
                    }
                    let ok = std::path::Path::new(&profile.cv_path).exists()
                        && !profile.cv_path.is_empty();
                    if !profile.cv_path.is_empty() {
                        let (color, icon) = if ok {
                            (theme::GREEN, "\u{2713}")
                        } else {
                            (theme::RED, "\u{2717}")
                        };
                        ui.label(RichText::new(icon).color(color));
                    }
                });

                ui.add_space(4.0);
                ui.label(RichText::new("Текст резюме (CV):").color(theme::TEXT_SECONDARY));
                ui.add(
                    egui::TextEdit::multiline(&mut profile.cv_text)
                        .hint_text("Вставьте текст резюме вручную или выберите файл выше...")
                        .desired_rows(4)
                        .desired_width(ui.available_width()),
                );

                ui.add_space(8.0);

                ui.horizontal(|ui| {
                    ui.add_sized(
                        [120.0, 20.0],
                        egui::Label::new(RichText::new("Вакансия:").color(theme::TEXT_SECONDARY)),
                    );
                    ui.add(
                        egui::TextEdit::singleline(&mut profile.vacancy_path)
                            .desired_width(ui.available_width() - 110.0),
                    );
                    if ui.add(egui::Button::new("Обзор...").fill(theme::BG_BUTTON).corner_radius(4.0)).clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("Документы (*.pdf, *.txt, *.md)", &["pdf", "txt", "md"])
                            .pick_file()
                        {
                            profile.vacancy_path = path.to_string_lossy().to_string();
                        }
                    }
                    let ok = std::path::Path::new(&profile.vacancy_path).exists()
                        && !profile.vacancy_path.is_empty();
                    if !profile.vacancy_path.is_empty() {
                        let (color, icon) = if ok {
                            (theme::GREEN, "\u{2713}")
                        } else {
                            (theme::RED, "\u{2717}")
                        };
                        ui.label(RichText::new(icon).color(color));
                    }
                });

                ui.add_space(4.0);
                ui.label(RichText::new("Текст вакансии:").color(theme::TEXT_SECONDARY));
                ui.add(
                    egui::TextEdit::multiline(&mut profile.vacancy_text)
                        .hint_text("Вставьте текст вакансии вручную или выберите файл выше...")
                        .desired_rows(4)
                        .desired_width(ui.available_width()),
                );

                ui.add_space(6.0);

                ui.label(RichText::new("Факты о кандидате:").color(theme::TEXT_SECONDARY));
                ui.add(
                    egui::TextEdit::multiline(&mut profile.facts)
                        .hint_text("Знает Rust 3 года...")
                        .desired_rows(3)
                        .desired_width(ui.available_width()),
                );
            });

            ui.add_space(10.0);

            // ── Настройки LLM ──
            theme::section_frame().show(ui, |ui| {
                theme::section_title(ui, "LLM (Генератор ответов)");

                ui.horizontal(|ui| {
                    ui.add_sized(
                        [120.0, 20.0],
                        egui::Label::new(RichText::new("Провайдер:").color(theme::TEXT_SECONDARY)),
                    );
                    egui::ComboBox::from_id_salt("llm_provider")
                        .selected_text(&profile.llm.provider)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut profile.llm.provider,
                                "openai_compat".to_string(),
                                "OpenAI Compatible",
                            );
                            ui.selectable_value(
                                &mut profile.llm.provider,
                                "openai".to_string(),
                                "OpenAI (Official)",
                            );
                            ui.selectable_value(
                                &mut profile.llm.provider,
                                "anthropic".to_string(),
                                "Anthropic (Claude)",
                            );
                        });
                });
                theme::form_row(ui, "Модель:", &mut profile.llm.model);
                theme::form_row(ui, "Базовый URL:", &mut profile.llm.base_url);
                theme::form_password_row(ui, "API ключ:", llm_key_input, show_llm_key);

                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.add_sized(
                        [120.0, 20.0],
                        egui::Label::new(
                            RichText::new("Температура:").color(theme::TEXT_SECONDARY),
                        ),
                    );
                    ui.add(egui::Slider::new(&mut profile.llm.temperature, 0.0..=2.0).step_by(0.1));
                });
            });

            ui.add_space(10.0);

            // ── Настройки ASR ──
            theme::section_frame().show(ui, |ui| {
                theme::section_title(ui, "ASR (Голосовой ввод)");

                ui.horizontal(|ui| {
                    ui.add_sized(
                        [120.0, 20.0],
                        egui::Label::new(RichText::new("Провайдер:").color(theme::TEXT_SECONDARY)),
                    );
                    egui::ComboBox::from_id_salt("asr_provider")
                        .selected_text(&profile.asr.provider)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut profile.asr.provider,
                                "openai_compat".to_string(),
                                "OpenAI Compatible",
                            );
                            ui.selectable_value(
                                &mut profile.asr.provider,
                                "openai".to_string(),
                                "OpenAI Whisper",
                            );
                        });
                });
                theme::form_row(ui, "Модель:", &mut profile.asr.model);
                theme::form_row(ui, "Базовый URL:", &mut profile.asr.base_url);
                theme::form_password_row(ui, "API ключ:", asr_key_input, show_asr_key);
            });

            ui.add_space(10.0);

            // ── Настройки Vision (OCR) ──
            theme::section_frame().show(ui, |ui| {
                theme::section_title(ui, "Vision (Распознавание скриншотов)");

                ui.label(
                    RichText::new("Модель с поддержкой зрения для извлечения текста из скриншотов.\nЕсли ключ не задан — будет использован локальный Tesseract OCR.")
                        .size(11.0)
                        .color(theme::TEXT_MUTED),
                );
                ui.add_space(6.0);

                ui.horizontal(|ui| {
                    ui.add_sized(
                        [120.0, 20.0],
                        egui::Label::new(RichText::new("Провайдер:").color(theme::TEXT_SECONDARY)),
                    );
                    egui::ComboBox::from_id_salt("vision_provider")
                        .selected_text(&profile.vision.provider)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut profile.vision.provider,
                                "openai_compat".to_string(),
                                "OpenAI Compatible",
                            );
                            ui.selectable_value(
                                &mut profile.vision.provider,
                                "openai".to_string(),
                                "OpenAI (Official)",
                            );
                            ui.selectable_value(
                                &mut profile.vision.provider,
                                "groq".to_string(),
                                "Groq",
                            );
                        });
                });
                theme::form_row(ui, "Модель:", &mut profile.vision.model);
                theme::form_row(ui, "Базовый URL:", &mut profile.vision.base_url);
                theme::form_password_row(ui, "API ключ:", vision_key_input, show_vision_key);

                ui.add_space(6.0);
                ui.checkbox(
                    &mut profile.vision.use_ocr_prompt,
                    "Использовать промпт для извлечения текста (для универсальных LLM)",
                );

                if profile.vision.use_ocr_prompt {
                    ui.add_space(4.0);
                    ui.label(RichText::new("Промпт для распознавания:").color(theme::TEXT_SECONDARY));
                    ui.add(
                        egui::TextEdit::multiline(&mut profile.vision.ocr_prompt)
                            .hint_text("Extract all text...")
                            .desired_rows(3)
                            .desired_width(ui.available_width()),
                    );
                }
            });

            ui.add_space(16.0);

            // ── Кнопки действий ──
            ui.horizontal(|ui| {
                if theme::accent_button(ui, "Сохранить", 110.0).clicked() {
                    Self::save_profile_static(
                        profile,
                        llm_key_input.clone(),
                        asr_key_input.clone(),
                        vision_key_input.clone(),
                        dbus_client,
                        event_tx.clone(),
                    );
                }

                ui.add_space(6.0);

                let is_active = Some(profile.id.clone()) == *active_id;
                let activate_text = if is_active {
                    "Активен"
                } else {
                    "Использовать"
                };
                let activate_btn = ui.add_enabled(
                    !is_active,
                    egui::Button::new(
                        RichText::new(activate_text)
                            .strong()
                            .color(theme::TEXT_PRIMARY),
                    )
                    .fill(theme::GREEN)
                    .corner_radius(6.0)
                    .min_size(Vec2::new(120.0, 34.0)),
                );

                if activate_btn.clicked() {
                    if let Some(client) = dbus_client {
                        let client = client.clone();
                        let tx = event_tx.clone();
                        let profile_id = profile.id.clone();
                        tokio::spawn(async move {
                            match client.set_active_profile(&profile_id).await {
                                Ok(_) => {
                                    if let Ok(active) = client.get_active_profile().await {
                                        let _ = tx.send(GuiEvent::ActiveProfileLoaded(active));
                                    }
                                }
                                Err(e) => {
                                    let _ = tx.send(GuiEvent::Error(format!(
                                        "Ошибка активации профиля: {}",
                                        e
                                    )));
                                }
                            }
                        });
                    }
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if theme::danger_button(ui, "Удалить", 100.0).clicked() {
                        if let Some(client) = dbus_client {
                            let client = client.clone();
                            let tx = event_tx.clone();
                            let profile_id = profile.id.clone();
                            tokio::spawn(async move {
                                let llm_key_name = format!("llm_api_key_{}", profile_id);
                                let asr_key_name = format!("asr_api_key_{}", profile_id);
                                let vision_key_name = format!("vision_api_key_{}", profile_id);
                                let _ = KeyringStore::delete_password(&llm_key_name).await;
                                let _ = KeyringStore::delete_password(&asr_key_name).await;
                                let _ = KeyringStore::delete_password(&vision_key_name).await;

                                match client.delete_profile(&profile_id).await {
                                    Ok(_) => {
                                        let _ = tx.send(GuiEvent::ProfileDeleted(profile_id));
                                    }
                                    Err(e) => {
                                        let _ = tx.send(GuiEvent::Error(format!(
                                            "Ошибка удаления профиля: {}",
                                            e
                                        )));
                                    }
                                }
                            });
                        }
                    }
                });
            });

            ui.add_space(20.0);
        });
    }

    /// Статическая функция сохранения профиля для обхода borrow checker
    fn save_profile_static(
        profile: &Profile,
        llm_key: String,
        asr_key: String,
        vision_key: String,
        dbus_client: &Option<Arc<DaemonClient>>,
        event_tx: UnboundedSender<GuiEvent>,
    ) {
        let profile_to_save = profile.clone();

        if let Some(client) = dbus_client {
            let client = client.clone();

            tokio::spawn(async move {
                // 1. Сохраняем ключи в безопасном хранилище Keyring
                let llm_key_name = format!("llm_api_key_{}", profile_to_save.id);
                let asr_key_name = format!("asr_api_key_{}", profile_to_save.id);
                let vision_key_name = format!("vision_api_key_{}", profile_to_save.id);

                if !llm_key.is_empty() {
                    let _ = KeyringStore::set_password(&llm_key_name, &llm_key).await;
                } else {
                    let _ = KeyringStore::delete_password(&llm_key_name).await;
                }

                if !asr_key.is_empty() {
                    let _ = KeyringStore::set_password(&asr_key_name, &asr_key).await;
                } else {
                    let _ = KeyringStore::delete_password(&asr_key_name).await;
                }

                if !vision_key.is_empty() {
                    let _ = KeyringStore::set_password(&vision_key_name, &vision_key).await;
                } else {
                    let _ = KeyringStore::delete_password(&vision_key_name).await;
                }

                // 2. Отправляем профиль демону для сохранения
                match client.save_profile(&profile_to_save).await {
                    Ok(saved) => {
                        let _ = event_tx.send(GuiEvent::ProfileSaved(saved));
                    }
                    Err(e) => {
                        let _ = event_tx
                            .send(GuiEvent::Error(format!("Ошибка сохранения профиля: {}", e)));
                    }
                }
            });
        }
    }
}
