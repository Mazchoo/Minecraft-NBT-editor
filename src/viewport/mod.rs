pub mod blocks;
pub mod camera;
pub mod grid;
pub mod highlight;
mod key_events;
pub mod raycast;

use std::sync::Arc;

use eframe::egui;
use glam::Vec3;

use camera::Camera;
use grid::GridCallback;
use key_events::{cursor_ndc, handle_input};

use crate::block_models::ModelLibrary;
use crate::textures::TextureLibrary;
use crate::world::World;

use blocks::{BlockCallback, build_gpu_mesh};
use highlight::HighlightCallback;
use raycast::FaceHit;

pub struct Viewport {
    camera: Camera,
    /// World pivot captured when a middle-mouse orbit drag starts.
    orbit_pivot: Option<Vec3>,
}

impl Viewport {
    pub fn new() -> Self {
        Self {
            camera: Camera::default(),
            orbit_pivot: None,
        }
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        world: &mut World,
        placing: bool,
        selected_block: Option<&str>,
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

        if placing
            && response.clicked()
            && let Some(hit) = hit
            && let Some(name) = selected_block
        {
            let cell = hit.placement_cell();
            if !world.is_occupied(cell) {
                world.place(cell, name);
            }
        }

        let mesh = world
            .take_dirty()
            .then(|| Arc::new(build_gpu_mesh(world, models, textures)));

        let view_proj = self.camera.view_proj(aspect);
        ui.painter()
            .add(eframe::egui_wgpu::Callback::new_paint_callback(
                rect,
                BlockCallback { view_proj, mesh },
            ));
        ui.painter()
            .add(eframe::egui_wgpu::Callback::new_paint_callback(
                rect,
                GridCallback { view_proj },
            ));
        ui.painter()
            .add(eframe::egui_wgpu::Callback::new_paint_callback(
                rect,
                HighlightCallback { view_proj, hit },
            ));
    }
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
    let pos = if response.clicked() {
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
