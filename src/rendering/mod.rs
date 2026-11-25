//! Everything that draws the 3D viewport: mesh building, face culling, GPU
//! pipelines and shaders. The rest of the editor describes *what* to draw
//! through [`init`], [`build_gpu_mesh`] and [`paint`], and never touches wgpu.

mod block_mesh;
mod blocks;
mod common;
mod grid;
mod grid_mesh;
mod highlight;
mod visibility;

use std::sync::Arc;

use eframe::egui;
use eframe::egui_wgpu::Callback;
use glam::{IVec3, Mat4};

use crate::models::Face;
use crate::textures::TextureLibrary;

pub use blocks::{GpuMesh, build_gpu_mesh};

/// What the viewport shows this frame.
pub struct Scene {
    pub view_proj: Mat4,
    /// A freshly built block mesh to upload, or `None` to keep drawing the
    /// last one.
    pub mesh: Option<Arc<GpuMesh>>,
    /// Block face to overlay as the placement target.
    pub highlight: Option<(IVec3, Face)>,
}

/// Creates the GPU resources once at startup.
pub fn init(cc: &eframe::CreationContext<'_>, textures: &TextureLibrary) {
    let render_state = cc
        .wgpu_render_state
        .as_ref()
        .expect("the wgpu renderer is required (NativeOptions::renderer)");
    grid::GridRenderer::init(render_state);
    blocks::BlockRenderer::init(render_state, textures);
    highlight::HighlightRenderer::init(render_state);
}

/// Queues the scene for drawing into `rect`: blocks, then the reference grid,
/// then the placement highlight.
pub fn paint(painter: &egui::Painter, rect: egui::Rect, scene: Scene) {
    let Scene {
        view_proj,
        mesh,
        highlight,
    } = scene;
    painter.add(Callback::new_paint_callback(
        rect,
        blocks::BlockCallback { view_proj, mesh },
    ));
    painter.add(Callback::new_paint_callback(
        rect,
        grid::GridCallback { view_proj },
    ));
    painter.add(Callback::new_paint_callback(
        rect,
        highlight::HighlightCallback {
            view_proj,
            target: highlight,
        },
    ));
}
