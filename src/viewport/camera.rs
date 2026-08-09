use glam::{Mat4, Quat, Vec2, Vec3, Vec4Swizzles};

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

    /// World point under the cursor for orbit/zoom: ray ∩ mesh, or the XZ
    /// plane (y = 0) when nothing is hit.
    pub fn pivot_under_cursor(&self, ndc: Vec2, aspect: f32) -> Vec3 {
        let ray = self.cursor_ray(ndc, aspect);
        let eye = self.eye();
        // TODO: intersect displayed mesh when one exists; fall back to y = 0.
        if ray.y.abs() < 1e-6 {
            return Vec3::new(self.focus.x, 0.0, self.focus.z);
        }
        let t = -eye.y / ray.y;
        if t < 0.0 {
            return Vec3::new(self.focus.x, 0.0, self.focus.z);
        }
        eye + ray * t
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

    /// Turntable orbit around `pivot`, driven by a mouse drag delta in screen
    /// pixels (positive y = dragging down). Eye and focus both rotate so the
    /// pivot stays fixed on screen (same idea as zoom-toward-cursor).
    pub fn orbit_about(&mut self, pivot: Vec3, drag_delta: Vec2) {
        const SENSITIVITY: f32 = 0.008; // radians per pixel

        let dyaw = drag_delta.x * SENSITIVITY;
        let target_pitch = (self.pitch + drag_delta.y * SENSITIVITY)
            .clamp(-89f32.to_radians(), 89f32.to_radians());
        let dpitch = target_pitch - self.pitch;
        if dyaw == 0.0 && dpitch == 0.0 {
            return;
        }

        let eye = self.eye();
        let mut eye_offset = eye - pivot;
        let mut focus_offset = self.focus - pivot;

        if dyaw != 0.0 {
            let q = Quat::from_rotation_y(dyaw);
            eye_offset = q * eye_offset;
            focus_offset = q * focus_offset;
        }

        if dpitch != 0.0 {
            let look = {
                let d = focus_offset - eye_offset;
                if d.length_squared() > 1e-12 {
                    d.normalize()
                } else {
                    self.forward()
                }
            };
            let right = {
                let r = look.cross(Vec3::Y);
                if r.length_squared() < 1e-12 {
                    let yaw = self.yaw + dyaw;
                    Vec3::new(yaw.cos(), 0.0, yaw.sin())
                } else {
                    r.normalize()
                }
            };
            let q = Quat::from_axis_angle(right, dpitch);
            eye_offset = q * eye_offset;
            focus_offset = q * focus_offset;
        }

        let new_eye = pivot + eye_offset;
        self.focus = pivot + focus_offset;
        let dir = self.focus - new_eye;
        self.distance = dir.length().max(0.5);
        let forward = if dir.length_squared() > 1e-12 {
            dir.normalize()
        } else {
            self.forward()
        };
        self.pitch = forward.y.clamp(-0.999999, 0.999999).asin();
        self.yaw = forward.x.atan2(-forward.z);
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
