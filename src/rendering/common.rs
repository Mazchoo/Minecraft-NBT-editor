//! Geometry helpers shared between renderers.

use glam::IVec3;

use crate::config::Config;
use crate::models::Face;

use super::grid_mesh::Vertex;

/// Two triangles covering `face` of the block in `cell`, in the placement
/// highlight colour and lifted off the surface by the highlight outset.
pub(super) fn face_triangles(face: Face, cell: IVec3) -> [Vertex; 6] {
    let q = face.world_quad(cell, Config::HIGHLIGHT_OUTSET);
    let v = |p: [f32; 3]| Vertex {
        position: p,
        color: Config::HIGHLIGHT_COLOR,
    };
    [v(q[0]), v(q[1]), v(q[2]), v(q[0]), v(q[2]), v(q[3])]
}
