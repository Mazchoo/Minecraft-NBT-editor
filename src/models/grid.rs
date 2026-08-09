use std::ops::RangeInclusive;

use glam::IVec3;

const MINOR_COLOR: [f32; 4] = [0.45, 0.45, 0.48, 1.0];
const MAJOR_COLOR: [f32; 4] = [0.8, 0.8, 0.8, 1.0];
const X_AXIS_COLOR: [f32; 4] = [0.85, 0.28, 0.30, 1.0];
const Y_AXIS_COLOR: [f32; 4] = [0.42, 0.75, 0.25, 1.0];
const Z_AXIS_COLOR: [f32; 4] = [0.25, 0.45, 0.90, 1.0];

/// Inclusive display bounds in block coordinates. Default empty selection is the origin.
pub type Bounds = RangeInclusive<IVec3>;

pub fn default_bounds() -> Bounds {
    IVec3::ZERO..=IVec3::ZERO
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub color: [f32; 4],
}

/// Expand bounds outward so min/max land on multiples of `major_line_block_spacing`.
/// Degenerate axes round up to at least one major cell so something is visible.
fn round_bounds_to_grid(bounds: &Bounds, major_line_block_spacing: i32) -> (IVec3, IVec3) {
    let spacing = major_line_block_spacing.max(1);
    let min = *bounds.start();
    let max = *bounds.end();
    (
        IVec3::new(
            align_down(min.x, spacing),
            align_down(min.y, spacing),
            align_down(min.z, spacing),
        ),
        IVec3::new(
            round_up_max(min.x, max.x, spacing),
            round_up_max(min.y, max.y, spacing),
            round_up_max(min.z, max.z, spacing),
        ),
    )
}

fn round_up_max(min: i32, max: i32, spacing: i32) -> i32 {
    let lo = align_down(min, spacing);
    let hi = align_up(max, spacing);
    if hi <= lo { lo + spacing } else { hi }
}

fn align_down(value: i32, spacing: i32) -> i32 {
    value.div_euclid(spacing) * spacing
}

fn align_up(value: i32, spacing: i32) -> i32 {
    let aligned = align_down(value, spacing);
    if aligned == value {
        value
    } else {
        aligned + spacing
    }
}

/// CPU-side geometry for the reference ground grid and world axes.
pub fn build_vertices(bounds: Bounds, major_line_block_spacing: i32) -> Vec<Vertex> {
    let (min, max) = round_bounds_to_grid(&bounds, major_line_block_spacing);
    let spacing = major_line_block_spacing.max(1);
    let mut vertices = Vec::new();

    let mut line = |a: [f32; 3], b: [f32; 3], color: [f32; 4]| {
        vertices.push(Vertex { position: a, color });
        vertices.push(Vertex { position: b, color });
    };

    let min_x = min.x as f32;
    let max_x = max.x as f32;
    let min_z = min.z as f32;
    let max_z = max.z as f32;

    for x in min.x..=max.x {
        if x == 0 {
            continue;
        }
        let color = if x.rem_euclid(spacing) == 0 {
            MAJOR_COLOR
        } else {
            MINOR_COLOR
        };
        let t = x as f32;
        line([t, 0.0, min_z], [t, 0.0, max_z], color);
    }

    for z in min.z..=max.z {
        if z == 0 {
            continue;
        }
        let color = if z.rem_euclid(spacing) == 0 {
            MAJOR_COLOR
        } else {
            MINOR_COLOR
        };
        let t = z as f32;
        line([min_x, 0.0, t], [max_x, 0.0, t], color);
    }

    // World axes: X red, Y green, Z blue — clipped to rounded display bounds.
    line([min_x, 0.0, 0.0], [max_x, 0.0, 0.0], X_AXIS_COLOR);
    line(
        [0.0, min.y as f32, 0.0],
        [0.0, max.y as f32, 0.0],
        Y_AXIS_COLOR,
    );
    line([0.0, 0.0, min_z], [0.0, 0.0, max_z], Z_AXIS_COLOR);

    vertices
}
