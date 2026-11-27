use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use eframe::egui;

use crate::block_models::ModelLibrary;
use crate::config::{CONFIG, Config};
use crate::constants::HOTBAR_LABEL_WIDTH_FRACTION;
use crate::import::jar::{ImportedAssets, JarError, import_from_jar};
use crate::import::nbt;
use crate::rendering;
use crate::textures::TextureLibrary;
use crate::transactions::History;
use crate::ui::left_panel::{self, Tool};
use crate::ui::ribbon;
use crate::ui::right_panel;
use crate::viewport::Viewport;
use crate::world::{BlockState, World};

struct PendingJarImport {
    path: PathBuf,
    rx: mpsc::Receiver<Result<ImportedAssets, JarError>>,
}

pub struct App {
    active_tool: Tool,
    search_text: String,
    selected_block: Option<String>,
    /// Every state of [`Self::selected_block`], and the one a click places.
    variants: Vec<BTreeMap<String, String>>,
    variant_index: usize,
    /// The block `variants` was listed for. `None` forces a new listing.
    variants_block: Option<String>,
    /// Tab presses not yet applied: +1 each, -1 with Shift.
    variant_steps: i32,
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
        let models = ModelLibrary::load();
        if models.needs_reimport() {
            log::warn!(
                "{} is missing; blocks without a model of their own name draw as fallback cubes until File > Set Minecraft Jar is run again",
                Config::blockstates_manifest_path().display()
            );
        }

        Self {
            active_tool: Tool::Place,
            search_text: String::new(),
            selected_block: None,
            variants: Vec::new(),
            variant_index: 0,
            variants_block: None,
            variant_steps: 0,
            viewport: Viewport::new(),
            world: World::new(),
            history: History::new(CONFIG.undo_history_limit()),
            textures,
            models,
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
                            "imported {} block textures, {} block models and {} blockstates from {}",
                            imported.textures.len(),
                            imported.models.len(),
                            imported.definitions.len(),
                            path.display()
                        );
                        self.textures.apply_import(imported.textures);
                        self.models
                            .apply_import(imported.models, imported.definitions);
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

    /// Drops a selection the reloaded caches no longer list, and lists the
    /// variants again in case they changed.
    fn prune_selected_block(&mut self) {
        if let Some(selected) = &self.selected_block
            && !self.models.has_block(selected)
        {
            self.selected_block = None;
        }
        self.variants_block = None;
    }

    /// Lists the variants of a newly selected block, then applies pending
    /// Tab presses, wrapping at either end.
    fn update_variant(&mut self) {
        if self.variants_block != self.selected_block {
            (self.variants, self.variant_index) = match &self.selected_block {
                Some(block) => self.models.variants(block),
                None => (Vec::new(), 0),
            };
            self.variants_block = self.selected_block.clone();
        }
        let steps = std::mem::take(&mut self.variant_steps);
        if !self.variants.is_empty() {
            let count = self.variants.len() as i64;
            self.variant_index =
                (self.variant_index as i64 + steps as i64).rem_euclid(count) as usize;
        }
    }

    /// Variant `offset` steps from the current one, wrapping.
    fn variant_at(&self, offset: i64) -> Option<&BTreeMap<String, String>> {
        let count = self.variants.len() as i64;
        (count > 0).then(|| {
            &self.variants[(self.variant_index as i64 + offset).rem_euclid(count) as usize]
        })
    }

    /// The block state a click places: the selected block in its current variant.
    fn selected_state(&self) -> Option<BlockState> {
        let block = self.selected_block.as_deref()?;
        Some(BlockState::new(block, self.variant_at(0)?.clone()))
    }

    fn start_jar_import(&mut self, path: PathBuf) {
        if self.jar_import.is_some() {
            return;
        }
        let texture_cache = self.textures.cache_dir().to_path_buf();
        let model_cache = self.models.cache_dir().to_path_buf();
        let textures_manifest = Config::textures_manifest_path();
        let models_manifest = Config::models_manifest_path();
        let blockstates_manifest = Config::blockstates_manifest_path();
        let jar_path = path.clone();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(import_from_jar(
                &jar_path,
                &texture_cache,
                &model_cache,
                &textures_manifest,
                &models_manifest,
                &blockstates_manifest,
            ));
        });
        self.jar_import = Some(PendingJarImport { path, rx });
    }

    /// Replaces the world with the structure file at `path` and points the
    /// camera at it. The undo history belongs to the old world and is dropped.
    /// On failure the current world is kept.
    fn import_nbt(&mut self, path: &Path) {
        match nbt::load(path) {
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

/// How a variant is shown in the hotbar: `axis=x, half=top`.
fn variant_label(properties: &BTreeMap<String, String>) -> String {
    if properties.is_empty() {
        return "default".to_owned();
    }
    properties
        .iter()
        .map(|(name, value)| format!("{name}={value}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// A single-line label cut off with an ellipsis at `max_width`. Hovering a
/// cut-off label shows its full text.
fn truncated_label(ui: &mut egui::Ui, text: String, max_width: f32) {
    ui.scope(|ui| {
        ui.set_max_width(max_width);
        ui.add(egui::Label::new(text).truncate());
    });
}

impl eframe::App for App {
    /// Takes Tab and Shift+Tab for variant cycling before egui uses them to
    /// move keyboard focus. Left alone while a text field is being typed in.
    fn raw_input_hook(&mut self, ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        raw_input.events.retain(|event| match event {
            egui::Event::Key {
                key: egui::Key::Tab,
                pressed,
                modifiers,
                ..
            } => {
                if *pressed {
                    self.variant_steps += if modifiers.shift { -1 } else { 1 };
                }
                false
            }
            _ => true,
        });
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll_jar_import();
        self.poll_model_manifest();
        self.history.set_limit(CONFIG.undo_history_limit());
        self.handle_undo_shortcut(ui);
        let import_busy = self.jar_import.is_some();

        egui::Panel::top("ribbon").resizable(false).show(ui, |ui| {
            match ribbon::show(ui, import_busy, self.models.needs_reimport()) {
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
                    self.models.block_names(),
                );
            });
        self.update_variant();
        let selected_state = self.selected_state();

        egui::Panel::bottom("hotbar").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(format!("Tool: {}", self.active_tool.name()));
                ui.separator();
                let block_text = match (&self.selected_block, self.variant_at(0)) {
                    (Some(block), Some(variant)) if self.variants.len() > 1 => format!(
                        "Block: {block} ({}/{}) [{}]",
                        self.variant_index + 1,
                        self.variants.len(),
                        variant_label(variant)
                    ),
                    (Some(block), _) => format!("Block: {block}"),
                    (None, _) => "Block: none".to_owned(),
                };
                if self.variants.len() > 1
                    && let (Some(previous), Some(next)) = (self.variant_at(-1), self.variant_at(1))
                {
                    // Each label takes at most a share of the space left, so
                    // the three always fit however long the variants are.
                    let share = ui.available_width() * HOTBAR_LABEL_WIDTH_FRACTION;
                    truncated_label(ui, block_text, share);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let share = ui.available_width() * HOTBAR_LABEL_WIDTH_FRACTION;
                        truncated_label(ui, format!("Tab: {}", variant_label(next)), share);
                        ui.separator();
                        let rest = ui.available_width();
                        truncated_label(
                            ui,
                            format!("Shift+Tab: {}", variant_label(previous)),
                            rest,
                        );
                    });
                } else {
                    let rest = ui.available_width();
                    truncated_label(ui, block_text, rest);
                }
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
                    selected_state.as_ref(),
                    &self.models,
                    &self.textures,
                );
            });

        ui.ctx().request_repaint_after(Duration::from_millis(500));
    }
}
