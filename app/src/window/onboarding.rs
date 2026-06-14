use crate::dbus::DaemonClient;
use crate::window::preferences::GuiEvent;
use crate::window::theme;
use eframe::egui;
use eframe::egui::{RichText, Vec2};
use izighost_common::Profile;
use std::sync::Arc;
use tokio::sync::mpsc::Sender;

#[derive(Debug, Clone, PartialEq)]
pub enum OnboardingStep {
    Welcome,
    ApiKeys,
    SystemCheck,
    CreateProfile,
    Finished,
}

pub struct OnboardingState {
    pub current_step: OnboardingStep,

    // Вводы API ключей
    pub llm_key: String,
    pub asr_key: String,
    pub vision_key: String,

    // Видимость ключей
    pub show_llm_key: bool,
    pub show_asr_key: bool,
    pub show_vision_key: bool,

    // Вводы профиля
    pub profile_name: String,
    pub system_prompt: String,

    // Результаты проверки зависимостей
    pub has_gstreamer: Option<bool>,
    pub has_tesseract: Option<bool>,
    pub has_python: Option<bool>,
    pub has_zip: Option<bool>,
    pub has_gnome_extensions: Option<bool>,

    pub is_saving: bool,
}

impl OnboardingState {
    pub fn new() -> Self {
        Self {
            current_step: OnboardingStep::Welcome,
            llm_key: String::new(),
            asr_key: String::new(),
            vision_key: String::new(),
            show_llm_key: false,
            show_asr_key: false,
            show_vision_key: false,
            profile_name: "Мой профиль".to_string(),
            system_prompt: "Ты senior-ментор для подготовки к собеседованию.\nОтвечай кратко, по делу, на русском.\nЕсли не знаешь ответа — скажи прямо.".to_string(),
            has_gstreamer: None,
            has_tesseract: None,
            has_python: None,
            has_zip: None,
            has_gnome_extensions: None,
            is_saving: false,
        }
    }

    pub fn check_system_dependencies(&mut self) {
        let check_cmd = |cmd: &str| -> bool {
            std::process::Command::new("which")
                .arg(cmd)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .map(|s| s.success())
                .unwrap_or(false)
        };

        self.has_gstreamer = Some(check_cmd("gst-launch-1.0"));
        self.has_tesseract = Some(check_cmd("tesseract"));
        self.has_python = Some(check_cmd("python3"));
        self.has_zip = Some(check_cmd("zip"));
        self.has_gnome_extensions = Some(check_cmd("gnome-extensions"));
    }

    pub fn draw(
        &mut self,
        ui: &mut egui::Ui,
        dbus_client: &Option<Arc<DaemonClient>>,
        event_tx: Sender<GuiEvent>,
    ) {
        ui.vertical_centered(|ui| {
            ui.add_space(10.0);
            ui.label(
                RichText::new("IziGhost")
                    .strong()
                    .size(24.0)
                    .color(theme::ACCENT),
            );
            ui.label(
                RichText::new("Мастер первого запуска")
                    .size(12.0)
                    .color(theme::TEXT_MUTED),
            );
            ui.add_space(10.0);
        });

        // Индикатор прогресса
        let step_idx = match self.current_step {
            OnboardingStep::Welcome => 1,
            OnboardingStep::ApiKeys => 2,
            OnboardingStep::SystemCheck => 3,
            OnboardingStep::CreateProfile => 4,
            OnboardingStep::Finished => 5,
        };

        ui.horizontal(|ui| {
            ui.add_space(12.0);
            for i in 1..=5 {
                let color = if i <= step_idx {
                    theme::ACCENT
                } else {
                    theme::BG_BUTTON
                };
                let (rect, _) = ui.allocate_exact_size(Vec2::new(60.0, 4.0), egui::Sense::hover());
                ui.painter().rect_filled(rect, 2.0, color);
                ui.add_space(4.0);
            }
        });
        ui.add_space(16.0);

        // Контент шага
        theme::section_frame().show(ui, |ui| {
            ui.set_min_height(260.0);
            match self.current_step {
                OnboardingStep::Welcome => self.draw_welcome(ui),
                OnboardingStep::ApiKeys => self.draw_api_keys(ui),
                OnboardingStep::SystemCheck => self.draw_system_check(ui),
                OnboardingStep::CreateProfile => self.draw_create_profile(ui),
                OnboardingStep::Finished => self.draw_finished(ui),
            }
        });

        ui.add_space(16.0);

        // Навигационные кнопки
        ui.horizontal(|ui| {
            if self.current_step != OnboardingStep::Welcome
                && self.current_step != OnboardingStep::Finished
                && ui
                    .add(
                        egui::Button::new(
                            RichText::new("Назад").strong().color(theme::TEXT_PRIMARY),
                        )
                        .fill(theme::BG_BUTTON)
                        .corner_radius(6.0)
                        .min_size(Vec2::new(100.0, 32.0)),
                    )
                    .clicked()
            {
                self.prev_step();
            }

            ui.with_layout(
                egui::Layout::right_to_left(egui::Align::Center),
                |ui| match self.current_step {
                    OnboardingStep::Finished => {
                        let btn = ui.add_enabled(
                            !self.is_saving,
                            egui::Button::new(
                                RichText::new("Начать работу")
                                    .strong()
                                    .color(theme::TEXT_PRIMARY),
                            )
                            .fill(theme::GREEN)
                            .corner_radius(6.0)
                            .min_size(Vec2::new(140.0, 32.0)),
                        );
                        if btn.clicked() {
                            self.save_and_finish(dbus_client, event_tx);
                        }
                    }
                    _ => {
                        let btn_text = if self.current_step == OnboardingStep::Welcome {
                            "Начать"
                        } else {
                            "Далее"
                        };
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new(btn_text).strong().color(theme::TEXT_PRIMARY),
                                )
                                .fill(theme::ACCENT)
                                .corner_radius(6.0)
                                .min_size(Vec2::new(100.0, 32.0)),
                            )
                            .clicked()
                        {
                            self.next_step();
                        }
                    }
                },
            );
        });
    }

    fn draw_welcome(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            theme::section_title(ui, "Приветствуем в IziGhost!");
            ui.add_space(8.0);
            ui.label(
                RichText::new("IziGhost — это твой личный AI-ментор и ассистент для прохождения технических собеседований на Linux GNOME.")
                    .color(theme::TEXT_PRIMARY)
                    .size(12.0)
            );
            ui.add_space(8.0);
            ui.label(
                RichText::new("Главная фишка: Полная скрытность.\nБлагодаря технологии RVMS (Reverse Virtual Monitor Stream), HUD оверлей-чат виден только тебе на основном экране, но гарантированно скрыт от трансляции экрана в Zoom, Teams или Discord.")
                    .color(theme::TEXT_SECONDARY)
                    .size(11.0)
            );
            ui.add_space(8.0);
            ui.label(
                RichText::new("Давай выполним первоначальную настройку в несколько простых шагов.")
                    .color(theme::TEXT_MUTED)
                    .size(11.0)
            );
        });
    }

    fn draw_api_keys(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            theme::section_title(ui, "1. Настройка API Ключей");
            ui.label(
                RichText::new("Ключи сохраняются в системный GNOME Keyring. Оставьте поля пустыми, если планируете использовать локальные офлайн-модели (Tesseract и faster-whisper).")
                    .color(theme::TEXT_MUTED)
                    .size(10.5)
            );
            ui.add_space(8.0);

            // Разделяем заимствования
            let llm_key = &mut self.llm_key;
            let asr_key = &mut self.asr_key;
            let vision_key = &mut self.vision_key;
            let show_llm_key = &mut self.show_llm_key;
            let show_asr_key = &mut self.show_asr_key;
            let show_vision_key = &mut self.show_vision_key;

            theme::form_password_row(ui, "LLM API Key:", llm_key, show_llm_key);
            ui.add_space(4.0);
            theme::form_password_row(ui, "ASR API Key:", asr_key, show_asr_key);
            ui.add_space(4.0);
            theme::form_password_row(ui, "Vision API Key:", vision_key, show_vision_key);
        });
    }

    fn draw_system_check(&mut self, ui: &mut egui::Ui) {
        if self.has_gstreamer.is_none() {
            self.check_system_dependencies();
        }

        ui.vertical(|ui| {
            theme::section_title(ui, "2. Проверка системных компонентов");
            ui.label(
                RichText::new("Наличие необходимых CLI утилит в ОС для локального OCR, записи звука и работы виртуального экрана.")
                    .color(theme::TEXT_MUTED)
                    .size(10.5)
            );
            ui.add_space(12.0);

            let draw_dep = |ui: &mut egui::Ui, name: &str, desc: &str, status: Option<bool>| {
                ui.horizontal(|ui| {
                    ui.add_sized([100.0, 18.0], egui::Label::new(RichText::new(name).strong().color(theme::TEXT_PRIMARY)));
                    ui.label(RichText::new(desc).color(theme::TEXT_SECONDARY).size(11.0));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        match status {
                            Some(true) => ui.label(RichText::new("Да").color(theme::GREEN)),
                            _ => ui.label(RichText::new("Нет").color(theme::RED)),
                        };
                    });
                });
                ui.add_space(6.0);
            };

            draw_dep(ui, "GStreamer", "запись аудио и трансляция RVMS", self.has_gstreamer);
            draw_dep(ui, "Tesseract", "локальное распознавание скриншотов", self.has_tesseract);
            draw_dep(ui, "Python 3", "локальный откат голосового ввода", self.has_python);
            draw_dep(ui, "Zip", "упаковка расширения GNOME", self.has_zip);
            draw_dep(ui, "GNOME Ext", "управление расширениями GNOME", self.has_gnome_extensions);

            ui.add_space(10.0);
            if ui.add(egui::Button::new("Перепроверить").fill(theme::BG_BUTTON).corner_radius(4.0)).clicked() {
                self.check_system_dependencies();
            }
        });
    }

    fn draw_create_profile(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            theme::section_title(ui, "3. Создание профиля");
            ui.label(
                RichText::new("Задайте имя и первоначальные инструкции для вашего ассистента.")
                    .color(theme::TEXT_MUTED)
                    .size(10.5),
            );
            ui.add_space(8.0);

            theme::form_row(ui, "Имя профиля:", &mut self.profile_name);
            ui.add_space(8.0);

            ui.label(
                RichText::new("Системный промпт (роль ассистента):").color(theme::TEXT_SECONDARY),
            );
            ui.add(
                egui::TextEdit::multiline(&mut self.system_prompt)
                    .desired_rows(4)
                    .desired_width(ui.available_width()),
            );
        });
    }

    fn draw_finished(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            theme::section_title(ui, "Все готово к запуску!");
            ui.add_space(8.0);
            ui.label(
                RichText::new("Базовая конфигурация IziGhost завершена.")
                    .color(theme::TEXT_PRIMARY)
                    .size(12.0)
            );
            ui.add_space(8.0);
            ui.label(
                RichText::new("После входа вы сможете:\n- Запустить виртуальный экран в окне настроек.\n- Пользоваться OCR скриншотов по кнопке экрана или вставкой Ctrl+V.\n- Надиктовывать вопросы голосом через микрофон.")
                    .color(theme::TEXT_SECONDARY)
                    .size(11.0)
            );
            ui.add_space(8.0);
            ui.label(
                RichText::new("Нажмите кнопку ниже, чтобы открыть чат HUD.")
                    .color(theme::TEXT_MUTED)
                    .size(11.0)
            );
        });
    }

    fn next_step(&mut self) {
        self.current_step = match self.current_step {
            OnboardingStep::Welcome => OnboardingStep::ApiKeys,
            OnboardingStep::ApiKeys => OnboardingStep::SystemCheck,
            OnboardingStep::SystemCheck => OnboardingStep::CreateProfile,
            OnboardingStep::CreateProfile => OnboardingStep::Finished,
            OnboardingStep::Finished => OnboardingStep::Finished,
        };
    }

    fn prev_step(&mut self) {
        self.current_step = match self.current_step {
            OnboardingStep::Welcome => OnboardingStep::Welcome,
            OnboardingStep::ApiKeys => OnboardingStep::Welcome,
            OnboardingStep::SystemCheck => OnboardingStep::ApiKeys,
            OnboardingStep::CreateProfile => OnboardingStep::SystemCheck,
            OnboardingStep::Finished => OnboardingStep::CreateProfile,
        };
    }

    pub fn handle_event(&mut self, event: &GuiEvent) {
        match event {
            GuiEvent::ActiveProfileLoaded(_) | GuiEvent::ProfileSaved(_) => {
                self.is_saving = false;
            }
            GuiEvent::Error(_) => {
                self.is_saving = false;
            }
            _ => {}
        }
    }

    fn save_and_finish(
        &mut self,
        dbus_client: &Option<Arc<DaemonClient>>,
        event_tx: Sender<GuiEvent>,
    ) {
        self.is_saving = true;

        let dbus_clone = dbus_client.clone();
        let name = self.profile_name.clone();
        let prompt = self.system_prompt.clone();
        let llm = self.llm_key.clone();
        let asr = self.asr_key.clone();
        let vis = self.vision_key.clone();

        tokio::spawn(async move {
            let id = "default".to_string();
            let profile = Profile {
                id: id.clone(),
                name,
                system_prompt: prompt,
                ..Default::default()
            };

            // Сохраняем ключи в безопасном Keyring
            let llm_key_name = format!("llm_api_key_{}", id);
            let asr_key_name = format!("asr_api_key_{}", id);
            let vision_key_name = format!("vision_api_key_{}", id);

            if !llm.is_empty() {
                let _ = izighost_common::KeyringStore::set_password(&llm_key_name, &llm).await;
            }
            if !asr.is_empty() {
                let _ = izighost_common::KeyringStore::set_password(&asr_key_name, &asr).await;
            }
            if !vis.is_empty() {
                let _ = izighost_common::KeyringStore::set_password(&vision_key_name, &vis).await;
            }

            // Отправляем демону команду сохранения профиля
            if let Some(client) = dbus_clone {
                match client.save_profile(&profile).await {
                    Ok(saved) => {
                        let _ = client.set_active_profile(&id).await;
                        let _ = event_tx.send(GuiEvent::ProfileSaved(saved.clone())).await;
                        let _ = event_tx.send(GuiEvent::ActiveProfileLoaded(saved)).await;
                    }
                    Err(e) => {
                        let _ = event_tx
                            .send(GuiEvent::Error(format!(
                                "Не удалось сохранить профиль: {}",
                                e
                            )))
                            .await;
                    }
                }
            } else {
                let _ = event_tx
                    .send(GuiEvent::Error("Демон недоступен".to_string()))
                    .await;
            }
        });
    }
}
