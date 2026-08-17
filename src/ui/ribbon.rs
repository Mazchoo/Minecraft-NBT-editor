use std::path::PathBuf;

use eframe::egui;

pub fn show(ui: &mut egui::Ui, import_busy: bool) -> Option<PathBuf> {
    let mut picked = None;
    egui::MenuBar::new().ui(ui, |ui| {
        ui.menu_button("File", |ui| {
            if ui
                .add_enabled(!import_busy, egui::Button::new("Set Minecraft Jar"))
                .clicked()
            {
                ui.close();
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Minecraft Jar", &["jar"])
                    .set_title("Set Minecraft Jar")
                    .pick_file()
                {
                    picked = Some(path);
                }
            }
        });
        if import_busy {
            ui.spinner();
            ui.label("Importing textures…");
            ui.ctx().request_repaint();
        }
    });
    picked
}
