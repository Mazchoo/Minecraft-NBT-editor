/// Half-extent of the ground grid in world units (blocks).
const GRID_HALF_EXTENT: i32 = 64;
/// Every n-th line is drawn brighter, like Blender's major grid lines.
const MAJOR_LINE_EVERY: i32 = 8;

const MINOR_COLOR: [f32; 4] = [0.45, 0.45, 0.48, 1.0];
const MAJOR_COLOR: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
const X_AXIS_COLOR: [f32; 4] = [0.85, 0.28, 0.30, 1.0];
const Y_AXIS_COLOR: [f32; 4] = [0.42, 0.75, 0.25, 1.0];
const Z_AXIS_COLOR: [f32; 4] = [0.25, 0.45, 0.90, 1.0];

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub color: [f32; 4],
}

/// CPU-side geometry for the reference ground grid and world axes.
pub fn build_vertices() -> Vec<Vertex> {
    let n = GRID_HALF_EXTENT;
    let extent = n as f32;
    let mut vertices = Vec::new();

    let mut line = |a: [f32; 3], b: [f32; 3], color: [f32; 4]| {
        vertices.push(Vertex { position: a, color });
        vertices.push(Vertex { position: b, color });
    };

    for i in -n..=n {
        if i == 0 {
            // The center lines are drawn as colored axes below.
            continue;
        }
        let color = if i % MAJOR_LINE_EVERY == 0 {
            MAJOR_COLOR
        } else {
            MINOR_COLOR
        };
        let t = i as f32;
        // Lines parallel to Z, then parallel to X.
        line([t, 0.0, -extent], [t, 0.0, extent], color);
        line([-extent, 0.0, t], [extent, 0.0, t], color);
    }

    // World axes: X red, Y green, Z blue.
    line([-extent, 0.0, 0.0], [extent, 0.0, 0.0], X_AXIS_COLOR);
    line([0.0, -extent, 0.0], [0.0, extent, 0.0], Y_AXIS_COLOR);
    line([0.0, 0.0, -extent], [0.0, 0.0, extent], Z_AXIS_COLOR);

    vertices
}
