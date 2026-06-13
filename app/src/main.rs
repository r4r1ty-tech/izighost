use eframe::egui;
use eframe::egui::{Color32, RichText, ViewportCommand};
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedReceiver;

mod dbus;

fn main() -> Result<(), eframe::Error> {
    // Создаем рантайм Tokio для zbus и других асинхронных задач
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

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("IziGhost")
            .with_inner_size([420.0, 600.0])
            .with_always_on_top(),
        ..Default::default()
    };
    eframe::run_native(
        "IziGhost",
        options,
        Box::new(move |_cc| {
            Ok(Box::new(IziGhostApp::new(dbus_client, signal_rx)))
        }),
    )
}

#[derive(PartialEq)]
enum VisibilityState {
    Visible,
    HiddenManual,
}

struct IziGhostApp {
    visibility: VisibilityState,
    input_text: String,
    dbus_client: Option<Arc<dbus::DaemonClient>>,
    signal_rx: Option<UnboundedReceiver<dbus::DaemonSignal>>,
    chat_messages: Vec<(String, String)>, // (role, message_text)
}

impl IziGhostApp {
    fn new(
        dbus_client: Option<Arc<dbus::DaemonClient>>,
        signal_rx: Option<UnboundedReceiver<dbus::DaemonSignal>>,
    ) -> Self {
        Self {
            visibility: VisibilityState::Visible,
            input_text: String::new(),
            dbus_client,
            signal_rx,
            chat_messages: Vec::new(),
        }
    }

    fn toggle_visibility(&mut self) {
        self.visibility = match self.visibility {
            VisibilityState::Visible => VisibilityState::HiddenManual,
            VisibilityState::HiddenManual => VisibilityState::Visible,
        };
    }

    fn handle_signals(&mut self) {
        if let Some(ref mut rx) = self.signal_rx {
            while let Ok(signal) = rx.try_recv() {
                match signal {
                    dbus::DaemonSignal::ChatChunk(chunk) => {
                        // Если последнее сообщение от ассистента — дополняем его, иначе создаем новое
                        if let Some((role, content)) = self.chat_messages.last_mut() {
                            if role == "assistant" {
                                content.push_str(&chunk);
                                continue;
                            }
                        }
                        self.chat_messages.push(("assistant".to_string(), chunk));
                    }
                    dbus::DaemonSignal::ChatCompleted => {
                        // Генерация ответа завершена
                    }
                    dbus::DaemonSignal::OcrCompleted(text) => {
                        self.input_text = text;
                    }
                    dbus::DaemonSignal::AsrCompleted(text) => {
                        self.input_text = text;
                    }
                    dbus::DaemonSignal::ErrorOccurred(message) => {
                        self.chat_messages.push(("system".to_string(), format!("Ошибка: {}", message)));
                    }
                }
            }
        }
    }
}

impl eframe::App for IziGhostApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // Опрашиваем D-Bus сигналы перед отрисовкой каждого кадра
        self.handle_signals();

        // ── Если скрыты — сразу прячем окно и выходим ──
        if self.visibility == VisibilityState::HiddenManual {
            ui.ctx().send_viewport_cmd(ViewportCommand::Visible(false));
            return;
        } else {
            ui.ctx().send_viewport_cmd(ViewportCommand::Visible(true));
        }

        egui::CentralPanel::default().show_inside(ui, |ui| {
            // ── Шапка ──
            ui.horizontal(|ui| {
                ui.heading("IziGhost");

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Лампочка-индикатор
                    let (lamp_emoji, lamp_color, lamp_tooltip) =
                        match self.visibility {
                            VisibilityState::Visible =>
                                ("🔴", Color32::from_rgb(220, 50, 50), "Видно — кликни чтобы скрыть"),
                            VisibilityState::HiddenManual =>
                                ("🟢", Color32::from_rgb(50, 200, 80), "Скрыто — кликни чтобы показать"),
                        };

                    let btn = ui.add(
                        egui::Button::new(
                            RichText::new(lamp_emoji)
                                .size(20.0)
                                .color(lamp_color)
                        )
                        .frame(false)
                    )
                    .on_hover_text(lamp_tooltip);

                    if btn.clicked() {
                        self.toggle_visibility();
                    }
                });
            });

            ui.separator();

            // ── Чат-окно ──
            ui.add_space(8.0);
            let chat_height = ui.available_height() - 50.0;
            egui::ScrollArea::vertical().max_height(chat_height).show(ui, |ui| {
                if self.chat_messages.is_empty() {
                    ui.label("Чат появится здесь...");
                } else {
                    for (role, msg) in &self.chat_messages {
                        let alignment = if role == "user" {
                            egui::Align::Max
                        } else {
                            egui::Align::Min
                        };
                        ui.with_layout(egui::Layout::top_down(alignment), |ui| {
                            let bg_color = if role == "user" {
                                Color32::from_rgb(40, 100, 200)
                            } else if role == "system" {
                                Color32::from_rgb(180, 50, 50)
                            } else {
                                Color32::from_rgb(45, 45, 48)
                            };

                            let frame = egui::Frame::NONE
                                .fill(bg_color)
                                .inner_margin(8.0)
                                .corner_radius(8.0);
                            
                            frame.show(ui, |ui| {
                                ui.label(RichText::new(msg).color(Color32::WHITE));
                            });
                        });
                        ui.add_space(4.0);
                    }
                }
            });

            ui.add_space(8.0);

            // ── Поле ввода ──
            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                ui.horizontal(|ui| {
                    let response = ui.add_sized(
                        [ui.available_width() - 60.0, 32.0],
                        egui::TextEdit::singleline(&mut self.input_text)
                            .hint_text("Введи вопрос...")
                    );
                    
                    let enter_pressed = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    let send_clicked = ui.button("➤").clicked();

                    if enter_pressed || send_clicked {
                        let cleaned_text = self.input_text.trim().to_string();
                        if !cleaned_text.is_empty() {
                            // Добавляем сообщение пользователя в локальный чат
                            self.chat_messages.push(("user".to_string(), cleaned_text.clone()));

                            // Отправляем в D-Bus демон в фоне
                            if let Some(ref client) = self.dbus_client {
                                let client = client.clone();
                                tokio::spawn(async move {
                                    let _ = client.send_chat_message(&cleaned_text).await;
                                });
                            }
                            
                            self.input_text.clear();
                        }
                    }
                });
            });
        });
    }
}
