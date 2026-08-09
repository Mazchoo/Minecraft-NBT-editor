pub mod camera;
pub mod grid;
mod key_events;

use eframe::egui;
use glam::Vec3;

use camera::Camera;
use grid::GridCallback;
use key_events::handle_input;

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

    pub fn show(&mut self, ui: &mut egui::Ui) {
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

        let view_proj = self.camera.view_proj(aspect);
        ui.painter()
            .add(eframe::egui_wgpu::Callback::new_paint_callback(
                rect,
                GridCallback { view_proj },
            ));
    }
}
