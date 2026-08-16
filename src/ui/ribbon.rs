use eframe::egui;

pub fn show(ui: &mut egui::Ui) {
    egui::MenuBar::new().ui(ui, |ui| {
        ui.menu_button("File", |ui| {
            if ui.button("Set Minecraft Jar").clicked() {
                ui.close();
                let _ = rfd::FileDialog::new()
                    .add_filter("Minecraft Jar", &["jar"])
                    .set_title("Set Minecraft Jar")
                    .pick_file();
            }
        });
    });
}
