use eframe::egui;
use glam::{Vec2, Vec3};

use super::camera::Camera;

fn cursor_ndc(rect: egui::Rect, pos: egui::Pos2) -> Vec2 {
    Vec2::new(
        (pos.x - rect.left()) / rect.width() * 2.0 - 1.0,
        1.0 - (pos.y - rect.top()) / rect.height() * 2.0,
    )
}

pub fn handle_input(
    camera: &mut Camera,
    orbit_pivot: &mut Option<Vec3>,
    ui: &mut egui::Ui,
    response: &egui::Response,
    rect: egui::Rect,
    aspect: f32,
) {
    if response.hovered() {
        // Scroll: zoom toward the world point under the cursor.
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll != 0.0 {
            if let Some(pos) = ui.input(|i| i.pointer.hover_pos()) {
                camera.zoom_toward(cursor_ndc(rect, pos), scroll, aspect);
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

    // Middle-mouse drag: orbit around the point under the cursor at press,
    // keeping that world point under the mouse as it moves (like zoom).
    // Handled outside the hover check so tracking continues if the cursor
    // leaves the rect mid-drag.
    if response.drag_started_by(egui::PointerButton::Middle) {
        if let Some(pos) = response.interact_pointer_pos() {
            *orbit_pivot = Some(camera.get_orbit_point(cursor_ndc(rect, pos), aspect));
        }
    }
    if response.dragged_by(egui::PointerButton::Middle) {
        if let (Some(pivot), Some(pos)) = (*orbit_pivot, response.interact_pointer_pos()) {
            let delta = response.drag_delta();
            camera.orbit_about(
                pivot,
                Vec2::new(delta.x, delta.y),
                cursor_ndc(rect, pos),
                aspect,
            );
        }
    } else {
        *orbit_pivot = None;
    }
}
