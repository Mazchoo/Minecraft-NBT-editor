pub mod camera;
pub mod grid;
mod key_events;

use eframe::egui;
use glam::Vec2;

use camera::Camera;
use grid::GridCallback;
use key_events::handle_input;

pub struct Viewport {
    camera: Camera,
}

impl Viewport {
    pub fn new() -> Self {
        Self {
            camera: Camera::default(),
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        let (rect, response) =
            ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());
        if rect.width() < 1.0 || rect.height() < 1.0 {
            return;
        }
        let aspect = rect.width() / rect.height();

        if response.hovered() {
            handle_input(&mut self.camera, ui, rect, aspect);
        }

        // Drag: orbit around the focus point. Handled outside the hover check
        // so the rotation keeps tracking even if the cursor leaves the rect.
        if response.dragged() {
            let delta = response.drag_delta();
            self.camera.orbit(Vec2::new(delta.x, delta.y));
        }

        let view_proj = self.camera.view_proj(aspect);
        ui.painter()
            .add(eframe::egui_wgpu::Callback::new_paint_callback(
                rect,
                GridCallback { view_proj },
            ));
    }
}
