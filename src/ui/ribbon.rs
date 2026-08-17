use eframe::egui;

use crate::textures::TextureLibrary;

pub fn show(ui: &mut egui::Ui, textures: &mut TextureLibrary) {
    egui::MenuBar::new().ui(ui, |ui| {
        ui.menu_button("File", |ui| {
            if ui.button("Set Minecraft Jar").clicked() {
                ui.close();
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Minecraft Jar", &["jar"])
                    .set_title("Set Minecraft Jar")
                    .pick_file()
                {
                    match textures.import_jar(&path) {
                        Ok(count) => {
                            log::info!("imported {count} block textures from {}", path.display())
                        }
                        Err(err) => log::error!("failed to import jar {}: {err}", path.display()),
                    }
                }
            }
        });
    });
}
