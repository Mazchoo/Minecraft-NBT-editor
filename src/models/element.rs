#![allow(dead_code)] // consumed by the viewport once textured block drawing lands

use std::collections::BTreeMap;

use glam::Vec3;

use super::block::Face;

/// A single-axis rotation in model space.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    X,
    Y,
    Z,
}

/// One box in model space (0-16 per axis, permitted range -16..32) with up to six faces.
#[derive(Debug, Clone)]
pub struct Element {
    from: Vec3,
    to: Vec3,
    rotation: Option<ElementRotation>,
    faces: [Option<ElementFace>; 6],
    shade: bool,
}

/// Per-element rotation about one axis, applied before the block origin.
#[derive(Debug, Clone, Copy)]
pub struct ElementRotation {
    origin: Vec3,
    axis: Axis,
    angle: f32,
    rescale: bool,
}

/// One face of an element. Absent faces are not drawn.
#[derive(Debug, Clone)]
pub struct ElementFace {
    texture: String,
    uv: [f32; 4],
    uv_rotation: u16,
    cullface: Option<Face>,
    tint_index: Option<u8>,
}

/// Orientation from NBT palette properties, in 90 degree steps about the block centre.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StateRotation {
    x: u16,
    y: u16,
}

impl Element {
    pub(crate) fn new(
        from: Vec3,
        to: Vec3,
        rotation: Option<ElementRotation>,
        faces: [Option<ElementFace>; 6],
        shade: bool,
    ) -> Self {
        Self {
            from,
            to,
            rotation,
            faces,
            shade,
        }
    }

    pub fn from(&self) -> Vec3 {
        self.from
    }

    pub fn to(&self) -> Vec3 {
        self.to
    }

    pub fn rotation(&self) -> Option<ElementRotation> {
        self.rotation
    }

    pub fn faces(&self) -> &[Option<ElementFace>; 6] {
        &self.faces
    }

    pub fn shade(&self) -> bool {
        self.shade
    }

    /// True when this face's two spanning axes have extent, so it is not a collapsed plane.
    pub fn face_has_area(&self, face: Face) -> bool {
        const EPS: f32 = 1e-6;
        let dx = (self.to.x - self.from.x).abs() > EPS;
        let dy = (self.to.y - self.from.y).abs() > EPS;
        let dz = (self.to.z - self.from.z).abs() > EPS;
        match face {
            Face::Down | Face::Up => dx && dz,
            Face::North | Face::South => dx && dy,
            Face::West | Face::East => dy && dz,
        }
    }
}

impl ElementRotation {
    pub(crate) fn new(origin: Vec3, axis: Axis, angle: f32, rescale: bool) -> Self {
        Self {
            origin,
            axis,
            angle,
            rescale,
        }
    }

    #[allow(dead_code)] // inspected by the properties panel and orientation debug
    pub fn origin(&self) -> Vec3 {
        self.origin
    }

    #[allow(dead_code)]
    pub fn axis(&self) -> Axis {
        self.axis
    }

    #[allow(dead_code)]
    pub fn angle(&self) -> f32 {
        self.angle
    }

    #[allow(dead_code)]
    pub fn rescale(&self) -> bool {
        self.rescale
    }

    /// Rotates `point` about [`Self::origin`], then optionally rescales by `1 / cos(angle)`.
    pub fn apply(self, point: Vec3) -> Vec3 {
        let rel = point - self.origin;
        let rad = self.angle.to_radians();
        let (sin, cos) = rad.sin_cos();
        let mut q = match self.axis {
            Axis::X => Vec3::new(rel.x, rel.y * cos - rel.z * sin, rel.y * sin + rel.z * cos),
            Axis::Y => Vec3::new(rel.x * cos + rel.z * sin, rel.y, -rel.x * sin + rel.z * cos),
            Axis::Z => Vec3::new(rel.x * cos - rel.y * sin, rel.x * sin + rel.y * cos, rel.z),
        };
        if self.rescale {
            let scale = 1.0 / cos.abs().max(1e-6);
            match self.axis {
                Axis::X => {
                    q.y *= scale;
                    q.z *= scale;
                }
                Axis::Y => {
                    q.x *= scale;
                    q.z *= scale;
                }
                Axis::Z => {
                    q.x *= scale;
                    q.y *= scale;
                }
            }
        }
        q + self.origin
    }
}

impl ElementFace {
    pub(crate) fn new(
        texture: String,
        uv: [f32; 4],
        uv_rotation: u16,
        cullface: Option<Face>,
        tint_index: Option<u8>,
    ) -> Self {
        Self {
            texture,
            uv,
            uv_rotation,
            cullface,
            tint_index,
        }
    }

    pub fn texture(&self) -> &str {
        &self.texture
    }

    pub fn uv(&self) -> [f32; 4] {
        self.uv
    }

    pub fn uv_rotation(&self) -> u16 {
        self.uv_rotation
    }

    pub fn cullface(&self) -> Option<Face> {
        self.cullface
    }

    pub fn tint_index(&self) -> Option<u8> {
        self.tint_index
    }
}

impl StateRotation {
    pub const IDENTITY: Self = Self { x: 0, y: 0 };

    pub fn new(x: u16, y: u16) -> Self {
        Self { x, y }
    }

    pub fn x(self) -> u16 {
        self.x
    }

    pub fn y(self) -> u16 {
        self.y
    }

    pub fn is_identity(self) -> bool {
        self.x % 360 == 0 && self.y % 360 == 0
    }

    /// Derives orientation from common palette properties (`axis`, `facing`).
    pub fn from_properties(properties: &BTreeMap<String, String>) -> Self {
        if let Some(axis) = properties.get("axis") {
            return match axis.as_str() {
                "x" => Self { x: 90, y: 90 },
                "z" => Self { x: 90, y: 0 },
                _ => Self::IDENTITY,
            };
        }
        if let Some(facing) = properties.get("facing") {
            return match facing.as_str() {
                "south" => Self { x: 0, y: 180 },
                "west" => Self { x: 0, y: 270 },
                "east" => Self { x: 0, y: 90 },
                "down" => Self { x: 90, y: 0 },
                "up" => Self { x: 270, y: 0 },
                _ => Self::IDENTITY,
            };
        }
        Self::IDENTITY
    }

    /// Rotates `point` about the block centre `(8, 8, 8)`, X first then Y.
    pub fn apply(self, point: Vec3) -> Vec3 {
        let mut q = point - Vec3::splat(8.0);
        q = rotate_x_steps(q, self.x);
        q = rotate_y_steps(q, self.y);
        q + Vec3::splat(8.0)
    }
}

fn rotate_x_steps(p: Vec3, degrees: u16) -> Vec3 {
    match degrees % 360 {
        90 => Vec3::new(p.x, -p.z, p.y),
        180 => Vec3::new(p.x, -p.y, -p.z),
        270 => Vec3::new(p.x, p.z, -p.y),
        _ => p,
    }
}

fn rotate_y_steps(p: Vec3, degrees: u16) -> Vec3 {
    // Clockwise from above, matching Minecraft's model `y` rotation.
    match degrees % 360 {
        90 => Vec3::new(-p.z, p.y, p.x),
        180 => Vec3::new(-p.x, p.y, -p.z),
        270 => Vec3::new(p.z, p.y, -p.x),
        _ => p,
    }
}

/// Default UV rectangle: the element's bounds projected onto `face`, on the 0-16 scale.
pub(crate) fn default_uv(face: Face, from: Vec3, to: Vec3) -> [f32; 4] {
    match face {
        Face::Down => [from.x, 16.0 - to.z, to.x, 16.0 - from.z],
        Face::Up => [from.x, from.z, to.x, to.z],
        Face::North => [16.0 - to.x, 16.0 - to.y, 16.0 - from.x, 16.0 - from.y],
        Face::South => [from.x, 16.0 - to.y, to.x, 16.0 - from.y],
        Face::West => [from.z, 16.0 - to.y, to.z, 16.0 - from.y],
        Face::East => [16.0 - to.z, 16.0 - to.y, 16.0 - from.z, 16.0 - from.y],
    }
}

pub(crate) fn tint_color(
    name: &str,
    tint_index: Option<u8>,
    properties: &BTreeMap<String, String>,
) -> [u8; 4] {
    let Some(_index) = tint_index else {
        return [255, 255, 255, 255];
    };
    let stem = {
        let path = name.rsplit_once(':').map(|(_, rest)| rest).unwrap_or(name);
        path.rsplit_once('/').map(|(_, s)| s).unwrap_or(path)
    };
    if stem.contains("redstone") {
        let power = properties
            .get("power")
            .and_then(|s| s.parse::<u8>().ok())
            .unwrap_or(0)
            .min(15);
        return redstone_color(power);
    }
    if stem.contains("grass") || stem == "dirt_path" {
        return [145, 189, 89, 255];
    }
    if stem.contains("leaves") || stem.contains("vine") || stem.contains("fern") {
        return [72, 181, 24, 255];
    }
    if stem.contains("water") || stem.contains("kelp") {
        return [63, 118, 228, 255];
    }
    [255, 255, 255, 255]
}

fn redstone_color(power: u8) -> [u8; 4] {
    let t = power as f32 / 15.0;
    let r = 77.0 + t * (255.0 - 77.0);
    let g = t * 43.0;
    let b = t * 13.0;
    [r as u8, g as u8, b as u8, 255]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_uv_of_full_cube_is_the_whole_texture() {
        let from = Vec3::ZERO;
        let to = Vec3::splat(16.0);
        assert_eq!(default_uv(Face::Up, from, to), [0.0, 0.0, 16.0, 16.0]);
        assert_eq!(default_uv(Face::Down, from, to), [0.0, 0.0, 16.0, 16.0]);
        assert_eq!(default_uv(Face::North, from, to), [0.0, 0.0, 16.0, 16.0]);
        assert_eq!(default_uv(Face::South, from, to), [0.0, 0.0, 16.0, 16.0]);
        assert_eq!(default_uv(Face::West, from, to), [0.0, 0.0, 16.0, 16.0]);
        assert_eq!(default_uv(Face::East, from, to), [0.0, 0.0, 16.0, 16.0]);
    }

    #[test]
    fn axis_x_is_90_90() {
        let mut props = BTreeMap::new();
        props.insert("axis".into(), "x".into());
        assert_eq!(
            StateRotation::from_properties(&props),
            StateRotation::new(90, 90)
        );
    }

    #[test]
    fn facing_south_is_y180() {
        let mut props = BTreeMap::new();
        props.insert("facing".into(), "south".into());
        assert_eq!(
            StateRotation::from_properties(&props),
            StateRotation::new(0, 180)
        );
    }
}
