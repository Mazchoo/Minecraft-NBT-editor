use glam::{Mat4, Vec2, Vec3, Vec4Swizzles};

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
    /// Direction the camera looks in (from eye toward focus), normalized.
    pub fn forward(&self) -> Vec3 {
        let (sin_yaw, cos_yaw) = self.yaw.sin_cos();
        let (sin_pitch, cos_pitch) = self.pitch.sin_cos();
        Vec3::new(cos_pitch * sin_yaw, sin_pitch, -cos_pitch * cos_yaw)
    }

    pub fn eye(&self) -> Vec3 {
        self.focus - self.forward() * self.distance
    }

    pub fn view_proj(&self, aspect: f32) -> Mat4 {
        let view = glam::camera::rh::view::look_at_mat4(self.eye(), self.focus, Vec3::Y);
        // The directx projection maps depth to [0, 1], matching wgpu clip space.
        let proj = glam::camera::rh::proj::directx::perspective(self.fov_y, aspect, 0.1, 5000.0);
        proj * view
    }

    /// World-space ray direction from the eye through the given NDC cursor position.
    fn cursor_ray(&self, ndc: Vec2, aspect: f32) -> Vec3 {
        let inv = self.view_proj(aspect).inverse();
        let far = inv * glam::Vec4::new(ndc.x, ndc.y, 1.0, 1.0);
        (far.xyz() / far.w - self.eye()).normalize()
    }

    /// Dolly the camera along the cursor ray so the point under the cursor
    /// stays (approximately) fixed on screen while zooming.
    pub fn zoom_toward(&mut self, ndc: Vec2, scroll_delta: f32, aspect: f32) {
        let factor = 0.998f32.powf(scroll_delta);
        let new_distance = (self.distance * factor).clamp(0.5, 3000.0);
        let travel = self.distance - new_distance;
        if travel == 0.0 {
            return;
        }

        let ray = self.cursor_ray(ndc, aspect);
        let new_eye = self.eye() + ray * travel;
        self.distance = new_distance;
        self.focus = new_eye + self.forward() * new_distance;
    }

    /// Turntable orbit around the focus point, driven by a mouse drag delta
    /// in screen pixels (positive y = dragging down).
    pub fn orbit(&mut self, drag_delta: Vec2) {
        const SENSITIVITY: f32 = 0.008; // radians per pixel

        self.yaw += drag_delta.x * SENSITIVITY;
        // Dragging up tilts the view further above the scene.
        self.pitch = (self.pitch + drag_delta.y * SENSITIVITY)
            .clamp(-89f32.to_radians(), 89f32.to_radians());
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

        // Scale by distance so panning feels consistent at any zoom level.
        let speed = self.distance * 0.9;
        self.focus += (forward_flat * input.y + right * input.x) * speed * dt;
    }
}
