use std::path::PathBuf;

use eframe::egui;

/// A File menu command the user picked a file for.
pub enum Action {
    SetJar(PathBuf),
    ImportNbt(PathBuf),
}

/// `needs_reimport` shows a notice that the cached jar data is out of date.
pub fn show(ui: &mut egui::Ui, import_busy: bool, needs_reimport: bool) -> Option<Action> {
    let mut action = None;
    egui::MenuBar::new().ui(ui, |ui| {
        ui.menu_button("File", |ui| {
            if ui.button("Import NBT").clicked() {
                ui.close();
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Structure NBT", &["nbt"])
                    .set_title("Import NBT")
                    .pick_file()
                {
                    action = Some(Action::ImportNbt(path));
                }
            }
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
                    action = Some(Action::SetJar(path));
                }
            }
        });
        if import_busy {
            ui.spinner();
            ui.label("Importing jar…");
            ui.ctx().request_repaint();
        } else if needs_reimport {
            ui.colored_label(
                ui.visuals().warn_fg_color,
                "Block data is out of date. Use File > Set Minecraft Jar to import it again.",
            );
        }
    });
    action
}
