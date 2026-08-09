use eframe::egui;
use glam::Vec2;

use super::camera::Camera;

pub fn handle_input(camera: &mut Camera, ui: &mut egui::Ui, rect: egui::Rect, aspect: f32) {
    // Scroll: zoom toward the world point under the cursor.
    let scroll = ui.input(|i| i.smooth_scroll_delta.y);
    if scroll != 0.0 {
        if let Some(pos) = ui.input(|i| i.pointer.hover_pos()) {
            let ndc = Vec2::new(
                (pos.x - rect.left()) / rect.width() * 2.0 - 1.0,
                1.0 - (pos.y - rect.top()) / rect.height() * 2.0,
            );
            camera.zoom_toward(ndc, scroll, aspect);
        }
    }

    // WASD: pan the camera focus in its horizontal plane.
    let (dt, keys) = ui.input(|i| (i.stable_dt.min(0.1), i.keys_down.clone()));
    let mut pan = Vec2::ZERO;
    if keys.contains(&egui::Key::W) {
        pan.y += 1.0;
    }
    if keys.contains(&egui::Key::S) {
        pan.y -= 1.0;
    }
    if keys.contains(&egui::Key::D) {
        pan.x += 1.0;
    }
    if keys.contains(&egui::Key::A) {
        pan.x -= 1.0;
    }
    if pan != Vec2::ZERO {
        camera.pan(pan, dt);
        // Keep animating while keys are held.
        ui.ctx().request_repaint();
    }
}
