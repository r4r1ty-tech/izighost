use eframe::egui;
use eframe::egui::Color32;
use std::sync::Arc;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

mod dbus;
mod hotkeys;
mod window;

use window::hud::HudState;
use window::preferences::{GuiEvent, PreferencesState};

fn install_and_enable_extension() -> std::io::Result<()> {
    let home = std::env::var("HOME")
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::NotFound, e))?;
    let ext_dir = std::path::PathBuf::from(home)
        .join(".local/share/gnome-shell/extensions/window-pin-bridge@gnome.extension");

    std::fs::create_dir_all(&ext_dir)?;

    let metadata_path = ext_dir.join("metadata.json");
    let extension_path = ext_dir.join("extension.js");

    let metadata_content = include_str!("../../extension/metadata.json");
    let extension_content = include_str!("../../extension/extension.js");

    let needs_write = |path: &std::path::Path, content: &str| -> bool {
        if let Ok(existing) = std::fs::read_to_string(path) {
            existing != content
        } else {
            true
        }
    };

    let mut updated = false;
    if needs_write(&metadata_path, metadata_content) {
        std::fs::write(&metadata_path, metadata_content)?;
        updated = true;
    }
    if needs_write(&extension_path, extension_content) {
        std::fs::write(&extension_path, extension_content)?;
        updated = true;
    }

    if updated {
        println!("Установлены/обновлены файлы расширения Window Pin Bridge.");
    }

    // Включаем расширение через утилиту gnome-extensions
    let status = std::process::Command::new("gnome-extensions")
        .arg("enable")
        .arg("window-pin-bridge@gnome.extension")
        .status();

    match status {
        Ok(s) if s.success() => {
            println!("Расширение Window Pin Bridge успешно включено.");
        }
        _ => {
            eprintln!("Не удалось включить расширение Window Pin Bridge через gnome-extensions.");
        }
    }

    Ok(())
}

fn main() -> Result<(), eframe::Error> {
    // Автоматически устанавливаем и активируем расширение GNOME Shell для Wayland Always-On-Top
    if let Err(e) = install_and_enable_extension() {
        eprintln!("Ошибка автоматической установки расширения Window Pin Bridge: {:?}", e);
    }

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

    // Инициализируем глобальные хоткеи в фоновом режиме
    if let Some(ref client) = dbus_client {
        let client_clone = client.clone();
        rt.spawn(async move {
            if let Err(e) = hotkeys::init_hotkeys(Some(client_clone)).await {
                eprintln!("Ошибка инициализации глобальных хоткеев: {:?}", e);
            }
        });
    }

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
        Box::new(move |_cc| Ok(Box::new(IziGhostApp::new(dbus_client, signal_rx)))),
    )
}

struct IziGhostApp {
    dbus_client: Option<Arc<dbus::DaemonClient>>,
    signal_rx: Option<UnboundedReceiver<dbus::DaemonSignal>>,

    // Канал для обработки событий из фоновых задач настроек
    gui_event_tx: UnboundedSender<GuiEvent>,
    gui_event_rx: UnboundedReceiver<GuiEvent>,

    // Состояния интерфейсов
    hud_state: HudState,
    preferences_state: PreferencesState,
}

impl IziGhostApp {
    fn new(
        dbus_client: Option<Arc<dbus::DaemonClient>>,
        signal_rx: Option<UnboundedReceiver<dbus::DaemonSignal>>,
    ) -> Self {
        let (gui_event_tx, gui_event_rx) = unbounded_channel();

        let preferences_state = PreferencesState::new(gui_event_tx.clone());
        preferences_state.init(&dbus_client);

        // Автоматически отправляем запрос на закрепление окна при старте через 200 мс
        let event_tx_clone = gui_event_tx.clone();
        if let Some(ref client) = dbus_client {
            let client_clone = client.clone();
            tokio::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                let pid = std::process::id();
                match client_clone.pin_window_by_pid(pid).await {
                    Ok(true) => println!("Окно HUD успешно закреплено поверх всех окон Wayland."),
                    Ok(false) => {
                        eprintln!("Mutter не нашел окно с PID {}.", pid);
                        let _ = event_tx_clone.send(GuiEvent::ExtensionNotLoaded);
                    }
                    Err(e) => {
                        eprintln!("Ошибка D-Bus при первоначальном закреплении окна: {:?}", e);
                        let _ = event_tx_clone.send(GuiEvent::ExtensionNotLoaded);
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
        }
    }

    /// Опрос D-Bus сигналов демона
    fn handle_signals(&mut self) {
        if let Some(ref mut rx) = self.signal_rx {
            while let Ok(signal) = rx.try_recv() {
                self.hud_state.handle_dbus_signal(signal);
            }
        }
    }

    /// Опрос событий GUI из фоновых потоков
    fn handle_gui_events(&mut self) {
        while let Ok(event) = self.gui_event_rx.try_recv() {
            match event {
                GuiEvent::ExtensionNotLoaded => {
                    self.hud_state.show_extension_warning = true;
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
        self.handle_signals();
        self.handle_gui_events();

        // 1. Отрисовка HUD в главном прозрачном окне
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(Color32::TRANSPARENT))
            .show_inside(ui, |ui| {
                self.hud_state
                    .draw(ui, &self.dbus_client, &self.preferences_state.active_id);
            });

        // 2. Отрисовка дополнительного окна настроек (если флаг активен)
        if self.hud_state.show_preferences {
            // Временно достаем preferences_state для передачи в замыкание
            let mut preferences_state = std::mem::replace(
                &mut self.preferences_state,
                PreferencesState::new(self.gui_event_tx.clone()),
            );
            let dbus_client = self.dbus_client.clone();
            let mut show_preferences = self.hud_state.show_preferences;

            ui.ctx().show_viewport_immediate(
                egui::ViewportId::from_hash_of("preferences_viewport"),
                egui::ViewportBuilder::default()
                    .with_title("Настройки IziGhost")
                    .with_inner_size([650.0, 720.0])
                    .with_decorations(true)
                    .with_transparent(false),
                |ctx, _class| {
                    if ctx.input(|i| i.viewport().close_requested()) {
                        show_preferences = false;
                    }
                    #[allow(deprecated)]
                    egui::CentralPanel::default().show(ctx, |ui| {
                        preferences_state.draw(ui, &dbus_client);
                    });
                },
            );

            // Возвращаем preferences_state обратно в структуру
            self.preferences_state = preferences_state;
            self.hud_state.show_preferences = show_preferences;
        }

        // Запрашиваем перерисовку для плавной обработки асинхронных сигналов
        ui.ctx().request_repaint();
    }
}
