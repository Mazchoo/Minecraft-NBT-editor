use crate::config::Config;
use glam::{Mat4, Vec2, Vec3};

/// Blender-style orbit camera: a focus point plus yaw/pitch/distance.
pub struct Camera {
    pub focus: Vec3,
    /// Rotation around +Y, in radians. 0 looks toward -Z.
    pub yaw: f32,
    /// Tilt in radians; negative looks down at the focus point.
    pub pitch: f32,
    pub distance: f32,
    pub fov_y: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            focus: Vec3::ZERO,
            yaw: 45f32.to_radians(),
            pitch: -30f32.to_radians(),
            distance: 40.0,
            fov_y: 60f32.to_radians(),
        }
    }
}

impl Camera {
    /// Centres the view on the box `min`..`max` and backs off far enough to
    /// see all of it, keeping the current viewing angle.
    pub fn frame(&mut self, min: Vec3, max: Vec3) {
        let radius = (max - min).length() * 0.5;
        self.focus = (min + max) * 0.5;
        self.distance = (radius / (self.fov_y * 0.5).sin()).max(Config::MIN_FRAME_DISTANCE);
    }

    /// Direction the camera looks in (from eye toward focus), normalized.
    pub fn forward(&self) -> Vec3 {
        let (sin_yaw, cos_yaw) = self.yaw.sin_cos();
        let (sin_pitch, cos_pitch) = self.pitch.sin_cos();
        Vec3::new(cos_pitch * sin_yaw, sin_pitch, -cos_pitch * cos_yaw)
    }

    /// Horizontal unit vector pointing to the camera's right. Derived from
    /// yaw alone so it stays defined when looking straight up or down.
    fn right(&self) -> Vec3 {
        Vec3::new(self.yaw.cos(), 0.0, self.yaw.sin())
    }

    fn up(&self) -> Vec3 {
        self.right().cross(self.forward())
    }

    pub fn eye(&self) -> Vec3 {
        self.focus - self.forward() * self.distance
    }

    /// Moves the camera, orientation unchanged, so the eye sits at `eye`.
    fn set_eye(&mut self, eye: Vec3) {
        self.focus = eye + self.forward() * self.distance;
    }

    /// Looks straight at the point `centre` of a face from
    /// `Config::FACE_VIEW_DISTANCE` blocks out along its outward `normal`.
    /// A horizontal face keeps the current yaw.
    pub fn view_face(&mut self, centre: Vec3, normal: Vec3) {
        let forward = -normal;
        self.focus = centre;
        self.distance = Config::FACE_VIEW_DISTANCE;
        self.pitch = forward.y.clamp(-1.0, 1.0).asin();
        if forward.x != 0.0 || forward.z != 0.0 {
            self.yaw = forward.x.atan2(-forward.z);
        }
    }

    pub fn view_proj(&self, aspect: f32) -> Mat4 {
        let view = glam::camera::rh::view::look_at_mat4(self.eye(), self.focus, self.up());
        // The directx projection maps depth to [0, 1], matching wgpu clip space.
        let proj = glam::camera::rh::proj::directx::perspective(self.fov_y, aspect, 0.1, 5000.0);
        proj * view
    }

    /// World-space ray from the eye through the given NDC cursor position,
    /// scaled so it advances one block of view depth per unit.
    fn view_ray(&self, ndc: Vec2, aspect: f32) -> Vec3 {
        let half_height = (self.fov_y * 0.5).tan();
        self.forward()
            + self.right() * (ndc.x * half_height * aspect)
            + self.up() * (ndc.y * half_height)
    }

    /// World-space ray direction from the eye through the given NDC cursor position.
    pub fn cursor_ray(&self, ndc: Vec2, aspect: f32) -> Vec3 {
        self.view_ray(ndc, aspect).normalize()
    }

    /// The point under the given NDC cursor position at the focus point's
    /// view depth. Used as the orbit pivot when the cursor is over nothing.
    pub fn point_at_focus_depth(&self, ndc: Vec2, aspect: f32) -> Vec3 {
        self.eye() + self.view_ray(ndc, aspect) * self.distance
    }

    /// Dolly the camera along the cursor ray at a fixed number of blocks per
    /// scroll unit, wherever the camera is. Zooming in stops just short of
    /// `target`, the world point under the cursor, if there is one.
    pub fn zoom_toward(&mut self, ndc: Vec2, scroll_delta: f32, aspect: f32, target: Option<Vec3>) {
        let ray = self.cursor_ray(ndc, aspect);
        let mut travel = scroll_delta * Config::ZOOM_SPEED;
        if travel > 0.0
            && let Some(target) = target
        {
            let room = (target - self.eye()).dot(ray) - Config::MIN_ZOOM_DISTANCE;
            travel = travel.min(room.max(0.0));
        }
        // Eye and focus move together, so the orbit distance is unchanged.
        self.focus += ray * travel;
    }

    /// Turntable orbit around `pivot`, driven by a mouse drag delta in screen
    /// pixels. The camera turns, then sits where `pivot` is under the cursor
    /// at the view depth it had before, so the pivot follows the mouse.
    pub fn orbit_about(&mut self, pivot: Vec3, drag_delta: Vec2, cursor_ndc: Vec2, aspect: f32) {
        let depth = (pivot - self.eye()).dot(self.forward());
        if depth <= 0.0 {
            return;
        }

        self.yaw -= drag_delta.x * Config::ORBIT_SENSITIVITY;
        self.pitch = (self.pitch + drag_delta.y * Config::ORBIT_SENSITIVITY)
            .clamp(-Config::MAX_ORBIT_PITCH, Config::MAX_ORBIT_PITCH);
        self.set_eye(pivot - self.view_ray(cursor_ndc, aspect) * depth);
    }

    /// Pan the focus point in the camera's horizontal plane.
    /// `input.x` is right/left (D/A), `input.y` is forward/back (W/S).
    pub fn pan(&mut self, input: Vec2, dt: f32) {
        let f = self.forward();
        let mut flat = Vec3::new(f.x, 0.0, f.z);
        if flat.length_squared() < 1e-6 {
            // Looking straight down: derive the direction from yaw alone.
            flat = Vec3::new(self.yaw.sin(), 0.0, -self.yaw.cos());
        }
        let forward_flat = flat.normalize();
        let right = forward_flat.cross(Vec3::Y).normalize();

        let direction = (forward_flat * input.y + right * input.x).normalize_or_zero();
        self.focus += direction * Config::PAN_SPEED * dt;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ASPECT: f32 = 1.6;

    fn ndc_of(camera: &Camera, point: Vec3) -> Vec2 {
        let clip = camera.view_proj(ASPECT) * point.extend(1.0);
        Vec2::new(clip.x / clip.w, clip.y / clip.w)
    }

    #[test]
    fn cursor_ray_matches_the_projection() {
        let camera = Camera::default();
        let ndc = Vec2::new(0.4, -0.7);
        let point = camera.eye() + camera.cursor_ray(ndc, ASPECT) * 25.0;
        assert!((ndc_of(&camera, point) - ndc).length() < 1e-4);
    }

    #[test]
    fn orbit_keeps_the_pivot_under_the_cursor() {
        let mut camera = Camera::default();
        let pivot = Vec3::new(3.0, 2.0, -4.0);
        let mut ndc = ndc_of(&camera, pivot);
        for _ in 0..2000 {
            ndc += Vec2::new(0.0004, 0.0002);
            camera.orbit_about(pivot, Vec2::new(1.0, -0.5), ndc, ASPECT);
            assert!((ndc_of(&camera, pivot) - ndc).length() < 1e-3);
        }
        assert!((camera.distance - Camera::default().distance).abs() < 1e-3);
    }

    #[test]
    fn starting_an_orbit_after_a_face_snap_does_not_move_the_camera() {
        let mut camera = Camera::default();
        camera.view_face(Vec3::new(1.0, 0.5, 0.5), Vec3::X);
        let eye = camera.eye();
        // Above the horizon, where the cursor is over neither block nor ground.
        let ndc = Vec2::new(0.3, 0.6);
        let pivot = camera.point_at_focus_depth(ndc, ASPECT);
        camera.orbit_about(pivot, Vec2::ZERO, ndc, ASPECT);
        assert!((camera.eye() - eye).length() < 1e-4);
    }

    #[test]
    fn orbit_step_moves_the_eye_in_proportion_to_the_drag() {
        let mut camera = Camera::default();
        camera.view_face(Vec3::new(1.0, 0.5, 0.5), Vec3::X);
        let eye = camera.eye();
        let pivot = Vec3::new(1.0, 0.1, 0.5);
        let ndc = ndc_of(&camera, pivot);
        camera.orbit_about(pivot, Vec2::new(1.0, 1.0), ndc, ASPECT);
        // One pixel of drag turns the eye about a pivot ~2 blocks away.
        assert!((camera.eye() - eye).length() < 0.1);
    }
}
