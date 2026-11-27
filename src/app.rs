use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use eframe::egui;

use crate::block_models::ModelLibrary;
use crate::config::{CONFIG, Config};
use crate::jar_import::{ImportedAssets, import_from_jar};
use crate::rendering;
use crate::textures::{JarError, TextureLibrary};
use crate::transactions::History;
use crate::ui::left_panel::{self, Tool};
use crate::ui::ribbon;
use crate::ui::right_panel;
use crate::viewport::Viewport;
use crate::world::{World, structure};

struct PendingJarImport {
    path: PathBuf,
    rx: mpsc::Receiver<Result<ImportedAssets, JarError>>,
}

pub struct App {
    active_tool: Tool,
    search_text: String,
    selected_block: Option<String>,
    viewport: Viewport,
    world: World,
    history: History,
    textures: TextureLibrary,
    models: ModelLibrary,
    jar_import: Option<PendingJarImport>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let textures = TextureLibrary::load();
        rendering::init(cc, &textures);

        Self {
            active_tool: Tool::Place,
            search_text: String::new(),
            selected_block: None,
            viewport: Viewport::new(),
            world: World::new(),
            history: History::new(CONFIG.undo_history_limit()),
            textures,
            models: ModelLibrary::load(),
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
                log::error!("jar import thread exited unexpectedly");
            }
            Some(Ok(result)) => {
                let path = self.jar_import.take().expect("pending import").path;
                match result {
                    Ok(imported) => {
                        log::info!(
                            "imported {} block textures and {} block models from {}",
                            imported.textures.len(),
                            imported.models.len(),
                            path.display()
                        );
                        self.textures.apply_import(imported.textures);
                        self.models.apply_import(imported.models);
                        self.prune_selected_block();
                        self.world.mark_dirty();
                    }
                    Err(err) => log::error!("failed to import jar {}: {err}", path.display()),
                }
            }
        }
    }

    /// Reloads the block picker when `_models.cache.json` changes on disk.
    /// Skipped while a jar import is in flight; that path already applies the new map.
    fn poll_model_manifest(&mut self) {
        if self.jar_import.is_some() {
            return;
        }
        if self.models.reload_if_changed() {
            log::info!("reloaded {} models from cache manifest", self.models.len());
            self.prune_selected_block();
            self.world.mark_dirty();
        }
    }

    fn prune_selected_block(&mut self) {
        if let Some(selected) = &self.selected_block
            && self.models.get_record(selected).is_none()
        {
            self.selected_block = None;
        }
    }

    fn start_jar_import(&mut self, path: PathBuf) {
        if self.jar_import.is_some() {
            return;
        }
        let texture_cache = self.textures.cache_dir().to_path_buf();
        let model_cache = self.models.cache_dir().to_path_buf();
        let textures_manifest = Config::textures_manifest_path();
        let models_manifest = Config::models_manifest_path();
        let jar_path = path.clone();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(import_from_jar(
                &jar_path,
                &texture_cache,
                &model_cache,
                &textures_manifest,
                &models_manifest,
            ));
        });
        self.jar_import = Some(PendingJarImport { path, rx });
    }

    /// Replaces the world with the structure file at `path` and points the
    /// camera at it. The undo history belongs to the old world and is dropped.
    /// On failure the current world is kept.
    fn import_nbt(&mut self, path: &Path) {
        match structure::load(path) {
            Ok(world) => {
                log::info!(
                    "imported {} blocks from {}",
                    world.blocks().count(),
                    path.display()
                );
                self.world = world;
                self.history.clear();
                self.viewport.frame_world(&self.world);
            }
            Err(err) => log::error!("failed to import {}: {err}", path.display()),
        }
    }

    /// Ctrl+Z undoes the last transaction. Left to the focused widget while
    /// one is taking keyboard input, so text fields keep their own undo.
    fn handle_undo_shortcut(&mut self, ui: &mut egui::Ui) {
        const UNDO: egui::KeyboardShortcut =
            egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::Z);
        if ui.ctx().egui_wants_keyboard_input() {
            return;
        }
        if ui.input_mut(|input| input.consume_shortcut(&UNDO)) {
            self.history.undo(&mut self.world);
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll_jar_import();
        self.poll_model_manifest();
        self.history.set_limit(CONFIG.undo_history_limit());
        self.handle_undo_shortcut(ui);
        let import_busy = self.jar_import.is_some();

        egui::Panel::top("ribbon").resizable(false).show(ui, |ui| {
            match ribbon::show(ui, import_busy) {
                Some(ribbon::Action::SetJar(path)) => self.start_jar_import(path),
                Some(ribbon::Action::ImportNbt(path)) => self.import_nbt(&path),
                None => return,
            }
            ui.ctx().request_repaint();
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
                right_panel::show(
                    ui,
                    &mut self.search_text,
                    &mut self.selected_block,
                    self.models.names(),
                );
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
                self.viewport.show(
                    ui,
                    &mut self.world,
                    &mut self.history,
                    self.active_tool == Tool::Place,
                    self.selected_block.as_deref(),
                    &self.models,
                    &self.textures,
                );
            });

        ui.ctx().request_repaint_after(Duration::from_millis(500));
    }
}
