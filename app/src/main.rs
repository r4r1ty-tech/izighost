use eframe::egui;
use eframe::egui::Color32;
use std::sync::Arc;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

mod dbus;
mod window;

use window::hud::HudState;
use window::preferences::{PreferencesState, GuiEvent};

fn main() -> Result<(), eframe::Error> {
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
        Box::new(move |_cc| {
            Ok(Box::new(IziGhostApp::new(dbus_client, signal_rx)))
        }),
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
            self.preferences_state.handle_event(event, &self.dbus_client);
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
            .frame(egui::Frame::none().fill(Color32::TRANSPARENT))
            .show_inside(ui, |ui| {
                self.hud_state.draw(ui, &self.dbus_client, &self.preferences_state.active_id);
            });

        // 2. Отрисовка дополнительного окна настроек (если флаг активен)
        if self.hud_state.show_preferences {
            // Временно достаем preferences_state для передачи в замыкание
            let mut preferences_state = std::mem::replace(
                &mut self.preferences_state,
                PreferencesState::new(self.gui_event_tx.clone())
            );
            let dbus_client = self.dbus_client.clone();
            
            ui.ctx().show_viewport_immediate(
                egui::ViewportId::from_hash_of("preferences_viewport"),
                egui::ViewportBuilder::default()
                    .with_title("Настройки IziGhost")
                    .with_inner_size([650.0, 720.0])
                    .with_decorations(true)
                    .with_transparent(false),
                |ctx, _class| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        preferences_state.draw(ui, &dbus_client);
                    });
                }
            );
            
            // Возвращаем preferences_state обратно в структуру
            self.preferences_state = preferences_state;
        }

        // Запрашиваем перерисовку для плавной обработки асинхронных сигналов
        ui.ctx().request_repaint();
    }
}
