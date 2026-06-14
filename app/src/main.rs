use eframe::egui;
use eframe::egui::Color32;
use std::sync::Arc;
use tokio::sync::mpsc::{Sender, Receiver};

mod dbus;
mod hotkeys;
mod window;

use window::hud::HudState;
use window::preferences::{GuiEvent, PreferencesState};

/// Установка GNOME Shell расширения для Always-On-Top на Wayland.
/// Использует `gnome-extensions install --force` с ZIP для корректной регистрации.
fn install_extension_files() {
    let home = match std::env::var("HOME") {
        Ok(h) => h,
        Err(_) => return,
    };

    let ext_dir = std::path::PathBuf::from(&home)
        .join(".local/share/gnome-shell/extensions/window-pin-bridge@gnome.extension");

    let metadata_content = include_str!("../../extension/metadata.json");
    let extension_content = include_str!("../../extension/extension.js");

    // Проверяем, нужно ли обновление файлов
    let needs_write = |path: &std::path::Path, content: &str| -> bool {
        if let Ok(existing) = std::fs::read_to_string(path) {
            existing != content
        } else {
            true
        }
    };

    let metadata_path = ext_dir.join("metadata.json");
    let extension_path = ext_dir.join("extension.js");

    if !needs_write(&metadata_path, metadata_content)
        && !needs_write(&extension_path, extension_content)
    {
        // Файлы актуальны — просто пытаемся включить
        match std::process::Command::new("gnome-extensions")
            .arg("enable")
            .arg("window-pin-bridge@gnome.extension")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
        {
            Ok(status) if !status.success() => {
                tracing::warn!("gnome-extensions enable failed with status: {:?}", status);
            }
            Err(e) => {
                tracing::warn!("Failed to execute gnome-extensions enable: {:?}", e);
            }
            _ => {}
        }
        return;
    }

    // Создаём ZIP во временной директории и устанавливаем через gnome-extensions install
    let tmp_dir = std::path::PathBuf::from(&home).join(".cache/izighost-ext-tmp");
    if let Err(e) = std::fs::create_dir_all(&tmp_dir) {
        tracing::error!("Failed to create directory {:?}: {:?}", tmp_dir, e);
    }
    let zip_path = tmp_dir.join("window-pin-bridge.zip");

    // Создаём минимальный ZIP-файл вручную (без внешних зависимостей)
    if let Ok(()) = create_extension_zip(&zip_path, metadata_content, extension_content) {
        match std::process::Command::new("gnome-extensions")
            .arg("install")
            .arg("--force")
            .arg(&zip_path)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
        {
            Ok(status) if !status.success() => {
                tracing::warn!("gnome-extensions install failed with status: {:?}", status);
            }
            Err(e) => {
                tracing::warn!("Failed to execute gnome-extensions install: {:?}", e);
            }
            _ => {}
        }

        match std::process::Command::new("gnome-extensions")
            .arg("enable")
            .arg("window-pin-bridge@gnome.extension")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
        {
            Ok(status) if !status.success() => {
                tracing::warn!("gnome-extensions enable failed with status: {:?}", status);
            }
            Err(e) => {
                tracing::warn!("Failed to execute gnome-extensions enable: {:?}", e);
            }
            _ => {}
        }
    }

    // Убираем временные файлы
    if let Err(e) = std::fs::remove_dir_all(&tmp_dir) {
        tracing::warn!("Failed to remove directory {:?}: {:?}", tmp_dir, e);
    }
}

/// Создание ZIP-архива с двумя файлами расширения.
/// Используем std::process::Command для вызова `zip` утилиты.
fn create_extension_zip(
    zip_path: &std::path::Path,
    metadata: &str,
    extension_js: &str,
 ) -> Result<(), std::io::Error> {
    let tmp_dir = zip_path
        .parent()
        .ok_or_else(|| std::io::Error::other("no parent"))?
        .join("ext_files");
    if let Err(e) = std::fs::create_dir_all(&tmp_dir) {
        tracing::error!("Failed to create temporary directory {:?}: {:?}", tmp_dir, e);
    }

    std::fs::write(tmp_dir.join("metadata.json"), metadata)?;
    std::fs::write(tmp_dir.join("extension.js"), extension_js)?;

    // Удаляем старый zip если есть
    if let Err(e) = std::fs::remove_file(zip_path) {
        if e.kind() != std::io::ErrorKind::NotFound {
            tracing::warn!("Failed to remove old zip file {:?}: {:?}", zip_path, e);
        }
    }

    let status = std::process::Command::new("zip")
        .arg("-j") // junk directory paths
        .arg(zip_path)
        .arg(tmp_dir.join("metadata.json"))
        .arg(tmp_dir.join("extension.js"))
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()?;

    if let Err(e) = std::fs::remove_dir_all(&tmp_dir) {
        tracing::warn!("Failed to remove temporary directory {:?}: {:?}", tmp_dir, e);
    }

    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other("zip command failed"))
    }
}

/// Применение глобальной тёмной темы к egui Visuals
fn apply_dark_theme(ctx: &egui::Context) {
    use window::theme;

    let mut visuals = egui::Visuals::dark();

    // Фон окон и панелей
    visuals.panel_fill = theme::BG_PRIMARY;
    visuals.window_fill = theme::BG_PRIMARY;
    visuals.extreme_bg_color = Color32::from_rgb(12, 12, 14);
    visuals.faint_bg_color = theme::BG_CARD;
    visuals.code_bg_color = theme::BG_CARD;

    // Виджеты — неактивные
    visuals.widgets.inactive.bg_fill = theme::BG_BUTTON;
    visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, Color32::from_rgb(55, 55, 60));
    visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, theme::TEXT_SECONDARY);
    visuals.widgets.inactive.corner_radius = egui::CornerRadius::same(6);

    // Виджеты — при наведении
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(55, 55, 60);
    visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, theme::ACCENT);
    visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, theme::TEXT_PRIMARY);
    visuals.widgets.hovered.corner_radius = egui::CornerRadius::same(6);

    // Виджеты — активные (нажатые)
    visuals.widgets.active.bg_fill = Color32::from_rgb(65, 65, 70);
    visuals.widgets.active.bg_stroke = egui::Stroke::new(1.0, theme::ACCENT);
    visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0, theme::TEXT_PRIMARY);
    visuals.widgets.active.corner_radius = egui::CornerRadius::same(6);

    // Виджеты — открытые (выпадающие меню и т.п.)
    visuals.widgets.open.bg_fill = Color32::from_rgb(50, 50, 55);
    visuals.widgets.open.bg_stroke = egui::Stroke::new(1.0, theme::ACCENT);
    visuals.widgets.open.fg_stroke = egui::Stroke::new(1.0, theme::TEXT_PRIMARY);
    visuals.widgets.open.corner_radius = egui::CornerRadius::same(6);

    // Виджеты — невзаимодействуемые
    visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(25, 25, 28);
    visuals.widgets.noninteractive.bg_stroke =
        egui::Stroke::new(0.5, Color32::from_rgb(50, 50, 55));
    visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, theme::TEXT_SECONDARY);
    visuals.widgets.noninteractive.corner_radius = egui::CornerRadius::same(6);

    // Выделение текста
    visuals.selection.bg_fill = Color32::from_rgba_unmultiplied(99, 102, 241, 100);
    visuals.selection.stroke = egui::Stroke::new(1.0, theme::ACCENT);

    // Скругление окон
    visuals.window_corner_radius = egui::CornerRadius::same(10);
    visuals.menu_corner_radius = egui::CornerRadius::same(8);

    // Тени
    visuals.popup_shadow = egui::Shadow {
        offset: [0, 4],
        blur: 12,
        spread: 0,
        color: Color32::from_black_alpha(80),
    };
    visuals.window_shadow = egui::Shadow {
        offset: [0, 4],
        blur: 16,
        spread: 0,
        color: Color32::from_black_alpha(60),
    };

    ctx.set_visuals(visuals);
}

fn main() -> Result<(), eframe::Error> {
    // Инициализируем логирование
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    let logs_dir = std::path::PathBuf::from(home).join(".cache/izighost/logs").join("client");
    let _ = std::fs::create_dir_all(&logs_dir);

    let file_appender = tracing_appender::rolling::daily(&logs_dir, "izighost.log");
    let (non_blocking, _log_guard) = tracing_appender::non_blocking(file_appender);

    use tracing_subscriber::prelude::*;
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    let console_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stderr);
    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(non_blocking)
        .with_ansi(false);

    tracing_subscriber::registry()
        .with(filter)
        .with(console_layer)
        .with(file_layer)
        .init();

    // Устанавливаем файлы расширения (без крашей при ошибках)
    install_extension_files();

    // Инициализируем Tokio рантайм для zbus
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Не удалось создать рантайм Tokio");
    let _guard = rt.enter();

    // Пытаемся подключиться к D-Bus серверу демона
    let (dbus_client, signal_rx) = match rt.block_on(dbus::DaemonClient::connect()) {
        Ok((client, rx)) => (Some(Arc::new(client)), Some(rx)),
        Err(e) => {
            eprintln!("Не удалось подключиться к D-Bus демону: {:?}", e);
            (None, None)
        }
    };

    // Глобальные хоткеи отключены в пользу интерактивного выделения области экрана
    /*
    if let Some(ref client) = dbus_client {
        let client_clone = client.clone();
        rt.spawn(async move {
            if let Err(e) = hotkeys::init_hotkeys(Some(client_clone)).await {
                eprintln!("Ошибка инициализации глобальных хоткеев: {:?}", e);
            }
        });
    }
    */

    // Опции для главного оверлей-окна HUD (прозрачное, без рамок)
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("IziGhost HUD")
            .with_inner_size([380.0, 480.0])
            .with_decorations(false)
            .with_transparent(true)
            .with_always_on_top(),
        ..Default::default()
    };

    eframe::run_native(
        "IziGhost HUD",
        options,
        Box::new(move |cc| {
            apply_dark_theme(&cc.egui_ctx);
            Ok(Box::new(IziGhostApp::new(dbus_client, signal_rx, cc.egui_ctx.clone())))
        }),
    )
}

struct IziGhostApp {
    dbus_client: Option<Arc<dbus::DaemonClient>>,
    signal_rx: Option<Receiver<dbus::DaemonSignal>>,

    // Канал для обработки событий из фоновых задач настроек
    gui_event_tx: Sender<GuiEvent>,
    gui_event_rx: Receiver<GuiEvent>,

    // Состояния интерфейсов
    hud_state: HudState,
    preferences_state: PreferencesState,

    show_onboarding: bool,
    onboarding_state: window::onboarding::OnboardingState,
}

impl IziGhostApp {
    fn new(
        dbus_client: Option<Arc<dbus::DaemonClient>>,
        signal_rx: Option<Receiver<dbus::DaemonSignal>>,
        ctx: egui::Context,
    ) -> Self {
        let (gui_event_tx, mut gui_event_rx_raw) = tokio::sync::mpsc::channel(100);
        let (gui_event_forward_tx, gui_event_rx) = tokio::sync::mpsc::channel(100);

        // Форвардер событий с вызовом request_repaint()
        let ctx_clone = ctx.clone();
        tokio::spawn(async move {
            while let Some(event) = gui_event_rx_raw.recv().await {
                let _ = gui_event_forward_tx.send(event).await;
                ctx_clone.request_repaint();
            }
        });

        // Форвардер D-Bus сигналов с вызовом request_repaint()
        let signal_rx = if let Some(mut rx) = signal_rx {
            let (sig_tx, sig_rx) = tokio::sync::mpsc::channel(100);
            let ctx_clone = ctx.clone();
            tokio::spawn(async move {
                while let Some(sig) = rx.recv().await {
                    let _ = sig_tx.send(sig).await;
                    ctx_clone.request_repaint();
                }
            });
            Some(sig_rx)
        } else {
            None
        };

        let preferences_state = PreferencesState::new(gui_event_tx.clone());
        preferences_state.init(&dbus_client);

        // Пытаемся закрепить окно через расширение (тихо, без спама)
        if let Some(ref client) = dbus_client {
            let client_clone = client.clone();
            let event_tx_clone = gui_event_tx.clone();
            tokio::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                let pid = std::process::id();
                match client_clone.pin_window_by_pid(pid).await {
                    Ok(true) => {} // Успех — молча
                    _ => {
                        // Расширение не загружено — показываем предупреждение
                        let _ = event_tx_clone.send(GuiEvent::ExtensionNotLoaded).await;
                    }
                }
            });
        }

        Self {
            dbus_client,
            signal_rx,
            gui_event_tx,
            gui_event_rx,
            hud_state: HudState::new(),
            preferences_state,
            show_onboarding: false,
            onboarding_state: window::onboarding::OnboardingState::new(),
        }
    }

    /// Опрос D-Bus сигналов демона
    fn handle_signals(&mut self, ctx: &egui::Context) {
        if let Some(ref mut rx) = self.signal_rx {
            while let Ok(signal) = rx.try_recv() {
                self.hud_state.handle_dbus_signal(signal, ctx);
            }
        }
    }

    /// Опрос событий GUI из фоновых потоков
    fn handle_gui_events(&mut self, ctx: &egui::Context) {
        while let Ok(event) = self.gui_event_rx.try_recv() {
            self.onboarding_state.handle_event(&event);

            match event {
                GuiEvent::ExtensionNotLoaded => {
                    self.hud_state.show_extension_warning = true;
                }
                GuiEvent::ProfilesLoaded(ref list) => {
                    if list.is_empty() {
                        self.show_onboarding = true;
                    }
                    self.preferences_state
                        .handle_event(event.clone(), &self.dbus_client);
                }
                GuiEvent::ActiveProfileLoaded(_) => {
                    self.show_onboarding = false;
                    self.preferences_state
                        .handle_event(event.clone(), &self.dbus_client);

                    if let Some(ref client) = self.dbus_client {
                        let client = client.clone();
                        let tx = self.gui_event_tx.clone();
                        tokio::spawn(async move {
                            match client.get_chat_history().await {
                                Ok(history) => {
                                    let _ = tx.send(GuiEvent::ChatHistoryLoaded(history)).await;
                                }
                                Err(e) => {
                                    let _ = tx.send(GuiEvent::Error(format!("Ошибка загрузки истории чата: {}", e))).await;
                                }
                            }
                        });
                    }
                }
                GuiEvent::ChatHistoryLoaded(history) => {
                    self.hud_state.chat_messages = history;
                }
                GuiEvent::ScreenshotCaptured(path) => {
                    self.hud_state.set_attached_screenshot(path, ctx);
                }
                GuiEvent::Error(msg) => {
                    self.hud_state.is_generating = false;
                    self.hud_state.is_listening = false;
                    self.preferences_state
                        .handle_event(GuiEvent::Error(msg), &self.dbus_client);
                }
                other => {
                    self.preferences_state
                        .handle_event(other, &self.dbus_client);
                }
            }
        }
    }
}

impl eframe::App for IziGhostApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // Опрашиваем каналы
        self.handle_signals(ui.ctx());
        self.handle_gui_events(ui.ctx());

        // 1. Отрисовка HUD в главном прозрачном окне
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(if self.show_onboarding { window::theme::BG_PRIMARY } else { Color32::TRANSPARENT }))
            .show_inside(ui, |ui| {
                if self.show_onboarding {
                    let event_tx = self.gui_event_tx.clone();
                    self.onboarding_state.draw(
                        ui,
                        &self.dbus_client,
                        event_tx,
                    );
                } else {
                    let event_tx = self.gui_event_tx.clone();
                    self.hud_state
                        .draw(ui, &self.dbus_client, &self.preferences_state.active_id, event_tx);
                }
            });

        // 2. Отрисовка дополнительного окна настроек (если флаг активен)
        if self.hud_state.show_preferences {
            let preferences_state = &mut self.preferences_state;
            let dbus_client = self.dbus_client.clone();
            let mut show_preferences = self.hud_state.show_preferences;

            ui.ctx().show_viewport_immediate(
                egui::ViewportId::from_hash_of("preferences_viewport"),
                egui::ViewportBuilder::default()
                    .with_title("IziGhost — Настройки")
                    .with_inner_size([650.0, 720.0])
                    .with_decorations(true)
                    .with_transparent(false),
                |ctx, _class| {
                    if ctx.input(|i| i.viewport().close_requested()) {
                        show_preferences = false;
                    }

                    // Применяем ту же тёмную тему к окну настроек только при необходимости
                    if ctx.style().visuals.panel_fill != window::theme::BG_PRIMARY {
                        apply_dark_theme(ctx);
                    }

                    #[allow(deprecated)]
                    egui::CentralPanel::default().show(ctx, |ui| {
                        preferences_state.draw(ui, &dbus_client);
                    });
                },
            );

            self.hud_state.show_preferences = show_preferences;
        }

        // Включение repaint_after для анимации/обновлений, если запущены фоновые процессы (ASR/LLM/etc.)
        if self.hud_state.is_generating || self.hud_state.is_listening {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(200));
        }
    }
}
