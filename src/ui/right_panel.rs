use eframe::egui;

pub fn show(
    ui: &mut egui::Ui,
    search_text: &mut String,
    selected_block: &mut Option<String>,
    blocks: &[String],
) {
    ui.add_space(4.0);
    ui.heading("Blocks");
    ui.add_space(4.0);

    ui.add(
        egui::TextEdit::singleline(search_text)
            .hint_text("Search blocks...")
            .desired_width(f32::INFINITY),
    );
    ui.add_space(4.0);
    ui.separator();

    if blocks.is_empty() {
        ui.add_space(8.0);
        ui.label("No cached models.");
        ui.label("Use File → Set Minecraft Jar to import.");
        return;
    }

    let query = search_text.to_lowercase();
    let filtered: Vec<&str> = blocks
        .iter()
        .map(String::as_str)
        .filter(|block| query.is_empty() || block.to_lowercase().contains(&query))
        .collect();

    if filtered.is_empty() {
        ui.add_space(8.0);
        ui.label("No matching blocks.");
        return;
    }

    let row_height = ui
        .spacing()
        .interact_size
        .y
        .max(ui.text_style_height(&egui::TextStyle::Body));
    egui::ScrollArea::vertical().show_rows(ui, row_height, filtered.len(), |ui, row_range| {
        ui.set_min_width(ui.available_width());
        for row in row_range {
            let block = filtered[row];
            let selected = selected_block.as_deref() == Some(block);
            if ui.selectable_label(selected, block).clicked() {
                *selected_block = Some(block.to_owned());
            }
        }
    });
}
