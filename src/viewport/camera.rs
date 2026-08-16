use glam::{Mat4, Quat, Vec2, Vec3, Vec4, Vec4Swizzles};

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

    /// World pivot under the cursor for orbit/zoom: ray ∩ mesh, or the XZ
    /// plane (y = 0) when nothing is hit.
    pub fn get_orbit_point(&self, ndc: Vec2, aspect: f32) -> Vec3 {
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
    /// pixels (positive y = dragging down). After rotating, the camera is
    /// translated so `pivot` stays under the current cursor (same idea as
    /// zoom-toward-cursor).
    pub fn orbit_about(&mut self, pivot: Vec3, drag_delta: Vec2, cursor_ndc: Vec2, aspect: f32) {
        const SENSITIVITY: f32 = 0.008; // radians per pixel

        let drag_yaw = drag_delta.x * SENSITIVITY;
        let drag_pitch = (self.pitch + drag_delta.y * SENSITIVITY)
            .clamp(-89f32.to_radians(), 89f32.to_radians())
            - self.pitch;

        if drag_yaw != 0.0 || drag_pitch != 0.0 {
            let eye_offset = self.eye() - pivot;
            let focus_offset = self.focus - pivot;
            let (eye_offset, focus_offset) =
                Self::update_offsets_from_yaw(eye_offset, focus_offset, drag_yaw);
            let (eye_offset, focus_offset) =
                Self::update_offsets_from_pitch(eye_offset, focus_offset, drag_pitch);
            self.recalculate_camera_from_offsets(pivot, eye_offset, focus_offset);
        }

        self.keep_point_at_ndc(pivot, cursor_ndc, aspect);
    }

    #[inline]
    fn update_offsets_from_yaw(
        eye_offset: Vec3,
        focus_offset: Vec3,
        drag_yaw: f32,
    ) -> (Vec3, Vec3) {
        if drag_yaw == 0.0 {
            return (eye_offset, focus_offset);
        }
        Self::rotate_offsets(eye_offset, focus_offset, Quat::from_rotation_y(drag_yaw))
    }

    #[inline]
    fn update_offsets_from_pitch(
        eye_offset: Vec3,
        focus_offset: Vec3,
        drag_pitch: f32,
    ) -> (Vec3, Vec3) {
        if drag_pitch == 0.0 {
            return (eye_offset, focus_offset);
        }
        let look = (focus_offset - eye_offset).normalize_or(Vec3::NEG_Z);
        let right = look
            .cross(Vec3::Y)
            .try_normalize()
            .unwrap_or_else(|| Vec3::new(eye_offset.z, 0.0, -eye_offset.x).normalize_or(Vec3::X));
        Self::rotate_offsets(
            eye_offset,
            focus_offset,
            Quat::from_axis_angle(right, drag_pitch),
        )
    }

    #[inline]
    fn rotate_offsets(eye_offset: Vec3, focus_offset: Vec3, rotation: Quat) -> (Vec3, Vec3) {
        (rotation * eye_offset, rotation * focus_offset)
    }

    #[inline]
    fn recalculate_camera_from_offsets(
        &mut self,
        pivot: Vec3,
        eye_offset: Vec3,
        focus_offset: Vec3,
    ) {
        let focus = pivot + focus_offset;
        let to_focus = focus - (pivot + eye_offset);
        let distance = to_focus.length().max(0.5);
        let forward = to_focus.try_normalize().unwrap_or(Vec3::NEG_Z);
        self.focus = focus;
        self.yaw = forward.x.atan2(-forward.z);
        self.pitch = forward.y.clamp(-0.999999, 0.999999).asin();
        self.distance = distance;
    }

    /// Translate the camera (orientation unchanged) so `point` projects to `ndc`.
    fn keep_point_at_ndc(&mut self, point: Vec3, ndc: Vec2, aspect: f32) {
        let view_proj = self.view_proj(aspect);
        let clip = view_proj * point.extend(1.0);
        if clip.w.abs() < 1e-8 {
            return;
        }
        let depth = clip.z / clip.w;
        let inv = view_proj.inverse();
        let target = inv * Vec4::new(ndc.x, ndc.y, depth, 1.0);
        if target.w.abs() < 1e-8 {
            return;
        }
        self.focus += point - target.xyz() / target.w;
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
