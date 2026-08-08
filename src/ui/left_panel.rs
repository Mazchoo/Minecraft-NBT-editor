use eframe::egui;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    Select,
    Place,
    Delete,
    Cuboid,
    Line,
    Cylinder,
}

impl Tool {
    pub const ALL: [Tool; 6] = [
        Tool::Select,
        Tool::Place,
        Tool::Delete,
        Tool::Cuboid,
        Tool::Line,
        Tool::Cylinder,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Tool::Select => "Select",
            Tool::Place => "Place",
            Tool::Delete => "Delete",
            Tool::Cuboid => "Cuboid",
            Tool::Line => "Line",
            Tool::Cylinder => "Cylinder",
        }
    }

    /// Short glyph shown on the square tool button.
    fn glyph(self) -> &'static str {
        match self {
            Tool::Select => "S",
            Tool::Place => "P",
            Tool::Delete => "D",
            Tool::Cuboid => "C",
            Tool::Line => "L",
            Tool::Cylinder => "O",
        }
    }
}

pub fn show(ui: &mut egui::Ui, active_tool: &mut Tool) {
    ui.add_space(4.0);
    ui.vertical_centered(|ui| {
        for tool in Tool::ALL {
            let selected = *active_tool == tool;
            let button = egui::Button::new(tool.glyph())
                .min_size(egui::vec2(40.0, 40.0))
                .selected(selected);
            if ui.add(button).on_hover_text(tool.name()).clicked() {
                *active_tool = tool;
            }
            ui.add_space(2.0);
        }
    });
}
