use eframe::egui;
use eframe::egui::{Color32, RichText, ViewportCommand};

fn main() -> Result<(), eframe::Error> {
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
        Box::new(|_cc| Ok(Box::new(IziGhostApp::default()))),
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
}

impl Default for IziGhostApp {
    fn default() -> Self {
        Self {
            visibility: VisibilityState::Visible,
            input_text: String::new(),
        }
    }
}

impl eframe::App for IziGhostApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
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

            // ── Заглушка чата ──
            ui.add_space(8.0);
            ui.label("Чат появится здесь...");
            ui.add_space(8.0);

            // ── Поле ввода ──
            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                ui.horizontal(|ui| {
                    let response = ui.add_sized(
                        [ui.available_width() - 60.0, 32.0],
                        egui::TextEdit::singleline(&mut self.input_text)
                            .hint_text("Введи вопрос...")
                    );
                    if (response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                        || ui.button("➤").clicked()
                    {
                        if !self.input_text.trim().is_empty() {
                            // TODO: отправить в daemon
                            self.input_text.clear();
                        }
                    }
                });
            });
        });
    }
}

impl IziGhostApp {
    fn toggle_visibility(&mut self) {
        self.visibility = match self.visibility {
            VisibilityState::Visible => VisibilityState::HiddenManual,
            VisibilityState::HiddenManual => VisibilityState::Visible,
        };
    }
}
