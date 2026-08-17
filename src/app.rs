use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc;

use eframe::egui;

use crate::textures::{JarError, Texture, TextureLibrary, import_into_cache};
use crate::ui::left_panel::{self, Tool};
use crate::ui::ribbon;
use crate::ui::right_panel;
use crate::viewport::{Viewport, grid::GridRenderer};

struct PendingJarImport {
    path: PathBuf,
    rx: mpsc::Receiver<Result<HashMap<String, Texture>, JarError>>,
}

pub struct App {
    active_tool: Tool,
    search_text: String,
    selected_block: Option<String>,
    viewport: Viewport,
    textures: TextureLibrary,
    jar_import: Option<PendingJarImport>,
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
            textures: TextureLibrary::load(),
            jar_import: None,
        }
    }

    fn poll_jar_import(&mut self) {
        let received = self
            .jar_import
            .as_ref()
            .map(|pending| pending.rx.try_recv());
        match received {
            None | Some(Err(mpsc::TryRecvError::Empty)) => {}
            Some(Err(mpsc::TryRecvError::Disconnected)) => {
                self.jar_import = None;
                log::error!("texture import thread exited unexpectedly");
            }
            Some(Ok(result)) => {
                let path = self.jar_import.take().expect("pending import").path;
                match result {
                    Ok(textures) => {
                        log::info!(
                            "imported {} block textures from {}",
                            textures.len(),
                            path.display()
                        );
                        self.textures.apply_import(textures);
                    }
                    Err(err) => log::error!("failed to import jar {}: {err}", path.display()),
                }
            }
        }
    }

    fn start_jar_import(&mut self, path: PathBuf) {
        if self.jar_import.is_some() {
            return;
        }
        let cache_dir = self.textures.cache_dir().to_path_buf();
        let jar_path = path.clone();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(import_into_cache(&jar_path, &cache_dir));
        });
        self.jar_import = Some(PendingJarImport { path, rx });
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll_jar_import();
        let import_busy = self.jar_import.is_some();

        egui::Panel::top("ribbon").resizable(false).show(ui, |ui| {
            if let Some(path) = ribbon::show(ui, import_busy) {
                self.start_jar_import(path);
                ui.ctx().request_repaint();
            }
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
