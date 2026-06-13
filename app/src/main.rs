use eframe::egui;

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "IziGhost eframe Hello World",
        options,
        Box::new(|_cc| {
            Ok(Box::new(MyApp::default()))
        }),
    )
}

struct MyApp {
    name: String,
}

impl Default for MyApp {
    fn default() -> Self {
        Self {
            name: "World".to_owned(),
        }
    }
}

impl eframe::App for MyApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.heading("Hello World from eframe!");
            ui.horizontal(|ui| {
                let name_label = ui.label("Your name: ");
                ui.text_edit_singleline(&mut self.name)
                    .labelled_by(name_label.id);
            });
            ui.label(format!("Hello {}!", self.name));
        });
    }
}
