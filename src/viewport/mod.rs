pub mod camera;
mod key_events;
pub mod raycast;

use std::sync::Arc;

use eframe::egui;
use glam::Vec3;

use camera::Camera;
use key_events::{cursor_ndc, handle_input};

use crate::block_models::ModelLibrary;
use crate::config::Config;
use crate::rendering::{self, Highlight, Scene};
use crate::textures::TextureLibrary;
use crate::transactions::{History, PlaceBlock, PlaceBlockFacing};
use crate::world::{BlockState, World};

use raycast::FaceHit;

pub struct Viewport {
    camera: Camera,
    /// World pivot captured when a middle-mouse orbit drag starts.
    orbit_pivot: Option<Vec3>,
    /// Toggled by E. While on, a click places the block with its bottom
    /// facing the highlighted face.
    facing_mode: bool,
}

impl Viewport {
    pub fn new() -> Self {
        Self {
            camera: Camera::default(),
            orbit_pivot: None,
            facing_mode: false,
        }
    }

    /// Points the camera at everything in `world`. Does nothing when it is
    /// empty.
    pub fn frame_world(&mut self, world: &World) {
        if let Some((min, max)) = world.bounds() {
            // A cell spans one unit from its position.
            self.camera.frame(min.as_vec3(), (max + 1).as_vec3());
        }
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        world: &mut World,
        history: &mut History,
        placing: bool,
        selected_state: Option<&BlockState>,
        models: &ModelLibrary,
        textures: &TextureLibrary,
    ) {
        let (rect, response) =
            ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());
        if rect.width() < 1.0 || rect.height() < 1.0 {
            return;
        }
        let aspect = rect.width() / rect.height();

        handle_input(
            &mut self.camera,
            &mut self.orbit_pivot,
            ui,
            &response,
            rect,
            aspect,
        );

        let hit = placement_hit(placing, &self.camera, &response, rect, aspect, world);

        if e_pressed(ui) {
            self.facing_mode = !self.facing_mode;
        }

        if placing
            && response.clicked()
            && let Some(hit) = hit
            && let Some(state) = selected_state
        {
            let cell = hit.placement_cell();
            let state = state.clone();
            if self.facing_mode {
                history.execute(world, PlaceBlockFacing::new(cell, state, hit.face));
            } else {
                history.execute(world, PlaceBlock::new(cell, state));
            }
        }

        // Right click deletes the highlighted block. A ground hit targets the
        // virtual cell at `y = -1`, which is never occupied, so it is a no-op.
        let mut hit = hit;
        if response.secondary_clicked()
            && let Some(target) = hit
            && world.remove(target.cell)
        {
            hit = None;
            ui.ctx().request_repaint();
        }

        let mesh = world
            .take_dirty()
            .then(|| Arc::new(rendering::build_gpu_mesh(world, models, textures)));

        rendering::paint(
            ui.painter(),
            rect,
            Scene {
                view_proj: self.camera.view_proj(aspect),
                mesh,
                highlight: hit.map(|hit| Highlight {
                    cell: hit.cell,
                    face: hit.face,
                    color: if self.facing_mode {
                        Config::HIGHLIGHT_FACING_COLOR
                    } else {
                        Config::HIGHLIGHT_COLOR
                    },
                }),
            },
        );
    }
}

/// Whether E went down this frame. Key repeat is ignored, and so is E while a
/// text field has keyboard focus, so typing an "e" in the search box does not
/// toggle facing mode.
fn e_pressed(ui: &egui::Ui) -> bool {
    if ui.ctx().egui_wants_keyboard_input() {
        return false;
    }
    ui.input(|input| {
        input.events.iter().any(|event| {
            matches!(
                event,
                egui::Event::Key {
                    key: egui::Key::E,
                    pressed: true,
                    repeat: false,
                    ..
                }
            )
        })
    })
}

fn placement_hit(
    placing: bool,
    camera: &Camera,
    response: &egui::Response,
    rect: egui::Rect,
    aspect: f32,
    world: &World,
) -> Option<FaceHit> {
    if !placing {
        return None;
    }
    let pos = if response.clicked() || response.secondary_clicked() {
        response.interact_pointer_pos()
    } else {
        response.hover_pos()
    }?;
    if !rect.contains(pos) {
        return None;
    }
    let origin = camera.eye();
    let dir = camera.cursor_ray(cursor_ndc(rect, pos), aspect);
    raycast::pick(origin, dir, |cell| world.is_occupied(cell))
}
