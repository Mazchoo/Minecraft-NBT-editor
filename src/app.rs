use eframe::egui;

use crate::ui::left_panel::{self, Tool};
use crate::ui::ribbon;
use crate::ui::right_panel;
use crate::viewport::{Viewport, grid::GridRenderer};

pub struct App {
    active_tool: Tool,
    search_text: String,
    selected_block: Option<String>,
    viewport: Viewport,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let render_state = cc
            .wgpu_render_state
            .as_ref()
            .expect("the wgpu renderer is required (NativeOptions::renderer)");
        GridRenderer::init(render_state);

        Self {
            active_tool: Tool::Select,
            search_text: String::new(),
            selected_block: None,
            viewport: Viewport::new(),
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::top("ribbon")
            .resizable(false)
            .show(ui, |ui| {
                ribbon::show(ui);
            });

        egui::Panel::left("tools_panel")
            .resizable(false)
            .exact_size(56.0)
            .show(ui, |ui| {
                left_panel::show(ui, &mut self.active_tool);
            });

        egui::Panel::right("blocks_panel")
            .default_size(240.0)
            .show(ui, |ui| {
                right_panel::show(ui, &mut self.search_text, &mut self.selected_block);
            });

        egui::Panel::bottom("hotbar").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(format!("Tool: {}", self.active_tool.name()));
                ui.separator();
                ui.label(match &self.selected_block {
                    Some(block) => format!("Block: {block}"),
                    None => "Block: none".to_owned(),
                });
            });
        });

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(egui::Color32::from_rgb(24, 24, 27)))
            .show(ui, |ui| {
                self.viewport.show(ui);
            });
    }
}
