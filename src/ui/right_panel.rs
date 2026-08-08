use eframe::egui;

/// Placeholder palette until real block definitions are loaded from assets.
const SAMPLE_BLOCKS: &[&str] = &[
    "minecraft:stone",
    "minecraft:dirt",
    "minecraft:grass_block",
    "minecraft:oak_planks",
    "minecraft:oak_log",
    "minecraft:cobblestone",
    "minecraft:glass",
    "minecraft:sand",
    "minecraft:gravel",
    "minecraft:andesite",
    "create:shaft",
    "create:cogwheel",
    "create:large_cogwheel",
];

pub fn show(ui: &mut egui::Ui, search_text: &mut String, selected_block: &mut Option<String>) {
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

    let query = search_text.to_lowercase();
    egui::ScrollArea::vertical().show(ui, |ui| {
        for &block in SAMPLE_BLOCKS {
            if !query.is_empty() && !block.contains(&query) {
                continue;
            }
            let selected = selected_block.as_deref() == Some(block);
            if ui.selectable_label(selected, block).clicked() {
                *selected_block = Some(block.to_owned());
            }
        }
    });
}
