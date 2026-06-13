use eframe::egui;
use eframe::egui::{Color32, RichText, Vec2};
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;
use izighost_common::{Profile, KeyringStore};
use crate::dbus::DaemonClient;

/// События, отправляемые из фоновых асинхронных задач в главный поток GUI
#[derive(Debug, Clone)]
pub enum GuiEvent {
    ProfilesLoaded(Vec<String>),
    ActiveProfileLoaded(Profile),
    ProfileDetailsLoaded(Profile, Option<String>, Option<String>), // profile, llm_key, asr_key
    ProfileSaved(Profile),
    ProfileDeleted(String),
    RvmsStarted(u32),
    RvmsStopped,
    Error(String),
}

pub struct PreferencesState {
    pub profiles: Vec<String>,
    pub selected_id: Option<String>,
    pub active_id: Option<String>,
    
    // Форма редактирования текущего профиля
    pub edit_profile: Option<Profile>,
    pub llm_key_input: String,
    pub asr_key_input: String,
    pub show_llm_key: bool,
    pub show_asr_key: bool,
    
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
            show_llm_key: false,
            show_asr_key: false,
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
                        let _ = tx.send(GuiEvent::Error(format!("Ошибка загрузки профилей: {}", e)));
                    }
                }

                match client.get_active_profile().await {
                    Ok(active) => {
                        if !active.id.is_empty() {
                            let _ = tx.send(GuiEvent::ActiveProfileLoaded(active));
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(GuiEvent::Error(format!("Ошибка получения активного профиля: {}", e)));
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
            GuiEvent::ProfileDetailsLoaded(profile, llm_key, asr_key) => {
                self.selected_id = Some(profile.id.clone());
                self.llm_key_input = llm_key.unwrap_or_default();
                self.asr_key_input = asr_key.unwrap_or_default();
                self.edit_profile = Some(profile);
                self.show_llm_key = false;
                self.show_asr_key = false;
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
                self.status_message = Some((format!("Виртуальный монитор запущен (PW Node ID: {})", node_id), false));
            }
            GuiEvent::RvmsStopped => {
                self.is_rvms_active = false;
                self.pipewire_node_id = None;
                self.status_message = Some(("Виртуальный монитор остановлен".to_string(), false));
            }
            GuiEvent::Error(msg) => {
                self.status_message = Some((msg, true));
            }
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
                        
                        let llm_key = KeyringStore::get_password(&llm_key_name).await.unwrap_or(None);
                        let asr_key = KeyringStore::get_password(&asr_key_name).await.unwrap_or(None);
                        
                        let _ = tx.send(GuiEvent::ProfileDetailsLoaded(profile, llm_key, asr_key));
                    }
                    Err(e) => {
                        let _ = tx.send(GuiEvent::Error(format!("Ошибка загрузки профиля '{}': {}", id, e)));
                    }
                }
            });
        }
    }

    /// Рендеринг интерфейса настроек
    pub fn draw(&mut self, ui: &mut egui::Ui, dbus_client: &Option<Arc<DaemonClient>>) {
        ui.style_mut().spacing.item_spacing = Vec2::new(8.0, 12.0);
        
        // Разделяем экран на две колонки: слева список профилей, справа — редактирование
        egui::SidePanel::left("sidebar_panel")
            .resizable(false)
            .default_width(200.0)
            .show_inside(ui, |ui| {
                self.draw_sidebar(ui, dbus_client);
            });

        egui::CentralPanel::default().show_inside(ui, |ui| {
            self.draw_main_panel(ui, dbus_client);
        });
    }

    /// Левая колонка (Список профилей + RVMS управление)
    fn draw_sidebar(&mut self, ui: &mut egui::Ui, dbus_client: &Option<Arc<DaemonClient>>) {
        ui.vertical(|ui| {
            ui.add_space(8.0);
            ui.label(RichText::new("ПРОФИЛИ").strong().color(Color32::from_rgb(110, 110, 120)));
            ui.add_space(4.0);

            // Список профилей
            egui::ScrollArea::vertical().max_height(280.0).show(ui, |ui| {
                for id in &self.profiles {
                    let is_selected = Some(id.clone()) == self.selected_id;
                    let is_active = Some(id.clone()) == self.active_id;
                    
                    let text = if is_active {
                        format!("[Акт] {}", id)
                    } else {
                        id.clone()
                    };

                    let btn_color = if is_selected {
                        Color32::from_rgb(99, 102, 241) // Indigo
                    } else {
                        Color32::from_rgb(45, 45, 50)
                    };

                    let btn = ui.add_sized(
                        [ui.available_width() - 4.0, 32.0],
                        egui::Button::new(RichText::new(text).color(Color32::WHITE)).fill(btn_color)
                    );

                    if btn.clicked() {
                        self.load_profile_details(id.clone(), dbus_client);
                    }
                    ui.add_space(2.0);
                }
            });

            ui.add_space(10.0);
            
            // Кнопка создания нового профиля
            let create_btn = ui.add_sized(
                [ui.available_width() - 4.0, 36.0],
                egui::Button::new(RichText::new("+ Новый профиль").strong().color(Color32::WHITE))
                    .fill(Color32::from_rgb(16, 185, 129)) // Green
            );
            
            if create_btn.clicked() {
                let random_id = format!(
                    "profile_{}",
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .expect("System time is set before Unix Epoch")
                        .as_millis() % 10000
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

            ui.add_space(20.0);
            ui.separator();
            ui.add_space(10.0);

            // Панель управления RVMS
            ui.label(RichText::new("ВИРТУАЛЬНЫЙ ЭКРАН").strong().color(Color32::from_rgb(110, 110, 120)));
            ui.add_space(4.0);

            let status_text = if self.is_rvms_active {
                format!("Активен (PW ID: {})", self.pipewire_node_id.unwrap_or(0))
            } else {
                "Отключен".to_string()
            };

            let status_color = if self.is_rvms_active {
                Color32::from_rgb(52, 211, 153) // Green
            } else {
                Color32::from_rgb(248, 113, 113) // Red
            };

            ui.horizontal(|ui| {
                ui.label("Статус:");
                ui.label(RichText::new(status_text).strong().color(status_color));
            });

            ui.add_space(6.0);

            let rvms_action_text = if self.is_rvms_active { "Остановить RVMS" } else { "Запустить RVMS" };
            let rvms_action_color = if self.is_rvms_active {
                Color32::from_rgb(220, 38, 38)
            } else {
                Color32::from_rgb(79, 70, 229)
            };

            let rvms_btn = ui.add_sized(
                [ui.available_width() - 4.0, 34.0],
                egui::Button::new(RichText::new(rvms_action_text).strong().color(Color32::WHITE)).fill(rvms_action_color)
            );

            if rvms_btn.clicked() {
                if let Some(client) = dbus_client {
                    let client = client.clone();
                    let tx = self.event_tx.clone();
                    let is_active = self.is_rvms_active;
                    tokio::spawn(async move {
                        if is_active {
                            match client.stop_rvms().await {
                                Ok(_) => { let _ = tx.send(GuiEvent::RvmsStopped); }
                                Err(e) => { let _ = tx.send(GuiEvent::Error(format!("Ошибка остановки RVMS: {}", e))); }
                            }
                        } else {
                            match client.start_rvms().await {
                                Ok(node_id) => { let _ = tx.send(GuiEvent::RvmsStarted(node_id)); }
                                Err(e) => { let _ = tx.send(GuiEvent::Error(format!("Ошибка запуска RVMS: {}", e))); }
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
            let color = if *is_error { Color32::from_rgb(239, 68, 68) } else { Color32::from_rgb(16, 185, 129) };
            let msg_clone = msg.clone();
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(msg_clone).color(color).strong());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("x").clicked() {
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
                ui.label("Выберите профиль слева или создайте новый для начала работы.");
            });
            return;
        };

        // Разделяем заимствования `self` для замыкания `ScrollArea::show`
        let llm_key_input = &mut self.llm_key_input;
        let asr_key_input = &mut self.asr_key_input;
        let show_llm_key = &mut self.show_llm_key;
        let show_asr_key = &mut self.show_asr_key;
        let active_id = &self.active_id;
        let event_tx = self.event_tx.clone();

        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.heading(format!("Редактирование профиля: {}", profile.id));
            ui.add_space(8.0);

            // Имя профиля
            ui.horizontal(|ui| {
                ui.label("Имя профиля:");
                ui.text_edit_singleline(&mut profile.name);
            });

            ui.add_space(10.0);

            // Системный промпт LLM
            ui.label(RichText::new("Системный промпт ассистента:").strong());
            ui.add(
                egui::TextEdit::multiline(&mut profile.system_prompt)
                    .hint_text("Ты senior-ментор...")
                    .desired_rows(4)
                    .desired_width(ui.available_width() - 10.0)
            );

            ui.add_space(10.0);

            // Путь к файлу резюме
            ui.label(RichText::new("Резюме кандидата (CV):").strong());
            ui.horizontal(|ui| {
                ui.text_edit_singleline(&mut profile.cv_path);
                
                // Простая валидация пути
                let path_exists = std::path::Path::new(&profile.cv_path).exists() && !profile.cv_path.is_empty();
                if path_exists {
                    ui.label(RichText::new("[OK]").color(Color32::GREEN));
                } else if !profile.cv_path.is_empty() {
                    ui.label(RichText::new("[ERR]").color(Color32::RED));
                }
            });
            
            if !profile.cv_text.is_empty() {
                ui.collapsing("Показать спарсенный текст резюме", |ui| {
                    let preview: String = profile.cv_text.chars().take(200).collect();
                    ui.group(|ui| {
                        ui.label(format!("{}...", preview));
                    });
                });
            }

            ui.add_space(10.0);

            // Путь к файлу вакансии
            ui.label(RichText::new("Описание вакансии (Vacancy):").strong());
            ui.horizontal(|ui| {
                ui.text_edit_singleline(&mut profile.vacancy_path);
                
                let path_exists = std::path::Path::new(&profile.vacancy_path).exists() && !profile.vacancy_path.is_empty();
                if path_exists {
                    ui.label(RichText::new("[OK]").color(Color32::GREEN));
                } else if !profile.vacancy_path.is_empty() {
                    ui.label(RichText::new("[ERR]").color(Color32::RED));
                }
            });

            if !profile.vacancy_text.is_empty() {
                ui.collapsing("Показать спарсенный текст вакансии", |ui| {
                    let preview: String = profile.vacancy_text.chars().take(200).collect();
                    ui.group(|ui| {
                        ui.label(format!("{}...", preview));
                    });
                });
            }

            ui.add_space(10.0);

            // Факты о кандидате
            ui.label(RichText::new("Факты о кандидате (достижения, проекты):").strong());
            ui.add(
                egui::TextEdit::multiline(&mut profile.facts)
                    .hint_text("Знает Rust 3 года...")
                    .desired_rows(3)
                    .desired_width(ui.available_width() - 10.0)
            );

            ui.add_space(10.0);

            // Настройки LLM
            ui.group(|ui| {
                ui.label(RichText::new("Настройки LLM (Генератор ответов)").strong());
                
                ui.horizontal(|ui| {
                    ui.label("Провайдер:");
                    egui::ComboBox::from_id_salt("llm_provider")
                        .selected_text(&profile.llm.provider)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut profile.llm.provider, "openai_compat".to_string(), "OpenAI Compatible");
                            ui.selectable_value(&mut profile.llm.provider, "openai".to_string(), "OpenAI (Official)");
                            ui.selectable_value(&mut profile.llm.provider, "anthropic".to_string(), "Anthropic (Claude)");
                        });
                });

                ui.horizontal(|ui| {
                    ui.label("Модель LLM:");
                    ui.text_edit_singleline(&mut profile.llm.model);
                });

                ui.horizontal(|ui| {
                    ui.label("Базовый URL:");
                    ui.text_edit_singleline(&mut profile.llm.base_url);
                });

                // API ключ LLM
                ui.horizontal(|ui| {
                    ui.label("API ключ LLM:");
                    ui.add(egui::TextEdit::singleline(llm_key_input).password(!*show_llm_key));
                    if ui.button(if *show_llm_key { "Скрыть" } else { "Показать" }).clicked() {
                        *show_llm_key = !*show_llm_key;
                    }
                });

                ui.horizontal(|ui| {
                    ui.label("Температура:");
                    ui.add(egui::Slider::new(&mut profile.llm.temperature, 0.0..=2.0).step_by(0.1));
                });
            });

            ui.add_space(10.0);

            // Настройки ASR
            ui.group(|ui| {
                ui.label(RichText::new("Настройки ASR (Голосовой ввод)").strong());

                ui.horizontal(|ui| {
                    ui.label("Провайдер:");
                    egui::ComboBox::from_id_salt("asr_provider")
                        .selected_text(&profile.asr.provider)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut profile.asr.provider, "openai_compat".to_string(), "OpenAI Compatible");
                            ui.selectable_value(&mut profile.asr.provider, "openai".to_string(), "OpenAI Whisper");
                        });
                });

                ui.horizontal(|ui| {
                    ui.label("Модель ASR:");
                    ui.text_edit_singleline(&mut profile.asr.model);
                });

                ui.horizontal(|ui| {
                    ui.label("Базовый URL:");
                    ui.text_edit_singleline(&mut profile.asr.base_url);
                });

                // API ключ ASR
                ui.horizontal(|ui| {
                    ui.label("API ключ ASR:");
                    ui.add(egui::TextEdit::singleline(asr_key_input).password(!*show_asr_key));
                    if ui.button(if *show_asr_key { "Скрыть" } else { "Показать" }).clicked() {
                        *show_asr_key = !*show_asr_key;
                    }
                });
            });

            ui.add_space(15.0);

            // Ряд кнопок управления
            ui.horizontal(|ui| {
                // Кнопка Сохранить
                let save_btn = ui.add_sized(
                    [100.0, 36.0],
                    egui::Button::new(RichText::new("Сохранить").strong().color(Color32::WHITE)).fill(Color32::from_rgb(79, 70, 229))
                );

                if save_btn.clicked() {
                    Self::save_profile_static(
                        profile,
                        llm_key_input.clone(),
                        asr_key_input.clone(),
                        dbus_client,
                        event_tx.clone()
                    );
                }

                ui.add_space(6.0);

                // Кнопка Сделать активным
                let is_active = Some(profile.id.clone()) == *active_id;
                let active_btn = ui.add_enabled(
                    !is_active,
                    egui::Button::new(
                        RichText::new(if is_active { "[Активен]" } else { "Использовать" }).strong().color(Color32::WHITE)
                    ).fill(Color32::from_rgb(16, 185, 129))
                    .min_size(Vec2::new(130.0, 36.0))
                );

                if active_btn.clicked() {
                    if let Some(client) = dbus_client {
                        let client = client.clone();
                        let tx = event_tx.clone();
                        let profile_id = profile.id.clone();
                        tokio::spawn(async move {
                            match client.set_active_profile(&profile_id).await {
                                Ok(_) => {
                                    // Перезапрашиваем активный профиль
                                    if let Ok(active) = client.get_active_profile().await {
                                        let _ = tx.send(GuiEvent::ActiveProfileLoaded(active));
                                    }
                                }
                                Err(e) => {
                                    let _ = tx.send(GuiEvent::Error(format!("Ошибка активации профиля: {}", e)));
                                }
                            }
                        });
                    }
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Кнопка Удалить
                    let delete_btn = ui.add_sized(
                        [100.0, 36.0],
                        egui::Button::new(RichText::new("Удалить").strong().color(Color32::WHITE)).fill(Color32::from_rgb(220, 38, 38))
                    );

                    if delete_btn.clicked() {
                        if let Some(client) = dbus_client {
                            let client = client.clone();
                            let tx = event_tx.clone();
                            let profile_id = profile.id.clone();
                            tokio::spawn(async move {
                                // Удаляем пароли из Keyring
                                let llm_key_name = format!("llm_api_key_{}", profile_id);
                                let asr_key_name = format!("asr_api_key_{}", profile_id);
                                let _ = KeyringStore::delete_password(&llm_key_name).await;
                                let _ = KeyringStore::delete_password(&asr_key_name).await;
                                
                                match client.delete_profile(&profile_id).await {
                                    Ok(_) => {
                                        let _ = tx.send(GuiEvent::ProfileDeleted(profile_id));
                                    }
                                    Err(e) => {
                                        let _ = tx.send(GuiEvent::Error(format!("Ошибка удаления профиля: {}", e)));
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
        dbus_client: &Option<Arc<DaemonClient>>,
        event_tx: UnboundedSender<GuiEvent>,
    ) {
        let mut profile_to_save = profile.clone();
        
        // Очищаем секретные ключи в объекте профиля перед сохранением в plain-text YAML
        profile_to_save.llm.api_key = "".to_string();
        profile_to_save.asr.api_key = "".to_string();

        if let Some(client) = dbus_client {
            let client = client.clone();
            
            tokio::spawn(async move {
                // 1. Сохраняем ключи в безопасном хранилище Keyring
                let llm_key_name = format!("llm_api_key_{}", profile_to_save.id);
                let asr_key_name = format!("asr_api_key_{}", profile_to_save.id);
                
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

                // 2. Отправляем профиль демону для сохранения
                match client.save_profile(&profile_to_save).await {
                    Ok(saved) => {
                        let _ = event_tx.send(GuiEvent::ProfileSaved(saved));
                    }
                    Err(e) => {
                        let _ = event_tx.send(GuiEvent::Error(format!("Ошибка сохранения профиля: {}", e)));
                    }
                }
            });
        }
    }
}
