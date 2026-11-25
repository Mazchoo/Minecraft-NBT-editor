use std::collections::HashMap;

use glam::IVec3;

use crate::config::Config;
use crate::models::Face;

/// Grid flag for a cell the exterior flood fill reached. The low six bits of a
/// grid byte hold the cell's occlusion mask (see [`Face::bit`]).
const REACHED: u8 = 1 << 6;

/// Answers "can this face ever be seen from outside the structure?".
///
/// Built once per mesh rebuild from each cell's occlusion mask: bit
/// [`Face::bit`] is set when the block fully covers that side of its cell with
/// opaque pixels. Two things hide a face:
///
/// * the neighbour it points at covers the shared side, or
/// * the space it looks into is sealed off from the exterior. Open space is
///   flood-filled from outside the bounding box, moving between cells only
///   where neither side of the shared boundary is covered; anything the fill
///   never reaches is an enclosed cavity.
#[derive(Debug, Default)]
pub struct Visibility {
    occluders: HashMap<IVec3, u8>,
    exterior: Option<Exterior>,
}

/// Dense grid over the occluders' bounding box padded by one cell per side.
#[derive(Debug)]
struct Exterior {
    min: IVec3,
    size: IVec3,
    grid: Vec<u8>,
}

impl Visibility {
    /// `cells` yields every occupied cell with its occlusion mask. Cells with
    /// an empty mask may be omitted.
    pub fn compute(cells: impl IntoIterator<Item = (IVec3, u8)>) -> Self {
        let occluders: HashMap<IVec3, u8> =
            cells.into_iter().filter(|(_, mask)| *mask != 0).collect();
        let exterior = Exterior::flood(&occluders);
        Self {
            occluders,
            exterior,
        }
    }

    /// Whether a face of the block in `cell` should be drawn.
    ///
    /// `toward` is the side of the cell the face lies on or is culled against,
    /// or `None` for geometry strictly inside the cell.
    pub fn face_visible(&self, cell: IVec3, toward: Option<Face>) -> bool {
        let Some(face) = toward else {
            return self.is_exposed(cell);
        };
        let neighbour = cell + face.offset();
        if self.mask(neighbour) & face.opposite().bit() != 0 {
            return false;
        }
        // The cell's own exposure keeps recessed faces that carry a cullface
        // drawn when the block is seen from another side.
        self.is_exposed(neighbour) || self.is_exposed(cell)
    }

    /// Whether open space in `cell` connects to the outside of the structure.
    pub fn is_exposed(&self, cell: IVec3) -> bool {
        match &self.exterior {
            None => true,
            Some(exterior) => exterior
                .index(cell)
                .is_none_or(|i| exterior.grid[i] & REACHED != 0),
        }
    }

    fn mask(&self, cell: IVec3) -> u8 {
        self.occluders.get(&cell).copied().unwrap_or(0)
    }
}

impl Exterior {
    /// `None` when there is nothing to enclose a cavity, or the volume is too
    /// large to flood; every cell then counts as exposed.
    fn flood(occluders: &HashMap<IVec3, u8>) -> Option<Self> {
        let mut cells = occluders.keys().copied();
        let first = cells.next()?;
        let (lo, hi) = cells.fold((first, first), |(lo, hi), c| (lo.min(c), hi.max(c)));

        // One cell of padding per side guarantees a connected, unoccluded shell.
        let dims: [i64; 3] = std::array::from_fn(|i| hi[i] as i64 - lo[i] as i64 + 3);
        let volume = dims
            .iter()
            .try_fold(1i64, |volume, dim| volume.checked_mul(*dim))
            .filter(|volume| *volume <= Config::MAX_CAVITY_FLOOD_CELLS);
        let Some(volume) = volume else {
            log::warn!(
                "structure bounds {dims:?} are too large to flood-fill; \
                 enclosed cavities will not be culled"
            );
            return None;
        };

        let mut exterior = Self {
            min: lo - IVec3::ONE,
            size: IVec3::new(dims[0] as i32, dims[1] as i32, dims[2] as i32),
            grid: vec![0; volume as usize],
        };
        for (cell, mask) in occluders {
            let i = exterior.index(*cell).expect("occluder inside its bounds");
            exterior.grid[i] = *mask;
        }

        let start = exterior.min;
        exterior.grid[0] |= REACHED;
        let mut stack = vec![start];
        while let Some(cell) = stack.pop() {
            let here = exterior.grid[exterior.index(cell).expect("stacked cell in bounds")];
            for face in Face::ALL {
                if here & face.bit() != 0 {
                    continue;
                }
                let next = cell + face.offset();
                let Some(i) = exterior.index(next) else {
                    continue;
                };
                let there = exterior.grid[i];
                if there & (REACHED | face.opposite().bit()) != 0 {
                    continue;
                }
                exterior.grid[i] = there | REACHED;
                stack.push(next);
            }
        }
        Some(exterior)
    }

    fn index(&self, cell: IVec3) -> Option<usize> {
        let p = cell - self.min;
        if p.cmplt(IVec3::ZERO).any() || p.cmpge(self.size).any() {
            return None;
        }
        let (sx, sy) = (self.size.x as usize, self.size.y as usize);
        Some(p.x as usize + sx * (p.y as usize + sy * p.z as usize))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FULL: u8 = 0b11_1111;

    /// 3x3x3 block of full cubes with the centre cell left empty.
    fn hollow_shell() -> Vec<(IVec3, u8)> {
        let mut cells = Vec::new();
        for x in 0..3 {
            for y in 0..3 {
                for z in 0..3 {
                    let cell = IVec3::new(x, y, z);
                    if cell != IVec3::ONE {
                        cells.push((cell, FULL));
                    }
                }
            }
        }
        cells
    }

    #[test]
    fn empty_world_exposes_everything() {
        let vis = Visibility::compute([]);
        assert!(vis.is_exposed(IVec3::ZERO));
        assert!(vis.face_visible(IVec3::ZERO, Some(Face::Up)));
        assert!(vis.face_visible(IVec3::ZERO, None));
    }

    #[test]
    fn facing_full_blocks_hide_their_shared_faces() {
        let vis = Visibility::compute([(IVec3::ZERO, FULL), (IVec3::X, FULL)]);
        assert!(!vis.face_visible(IVec3::ZERO, Some(Face::East)));
        assert!(!vis.face_visible(IVec3::X, Some(Face::West)));
        for face in [Face::West, Face::Up, Face::Down, Face::North, Face::South] {
            assert!(vis.face_visible(IVec3::ZERO, Some(face)), "{face:?}");
        }
    }

    #[test]
    fn neighbour_only_hides_the_side_it_covers() {
        // The neighbour to the east covers only its own east side.
        let vis = Visibility::compute([(IVec3::ZERO, FULL), (IVec3::X, Face::East.bit())]);
        assert!(vis.face_visible(IVec3::ZERO, Some(Face::East)));
    }

    #[test]
    fn sealed_cavity_hides_the_faces_lining_it() {
        let vis = Visibility::compute(hollow_shell());
        assert!(!vis.is_exposed(IVec3::ONE));
        // Faces looking into the cavity.
        assert!(!vis.face_visible(IVec3::new(0, 1, 1), Some(Face::East)));
        assert!(!vis.face_visible(IVec3::new(1, 2, 1), Some(Face::Down)));
        // Outer faces of the same blocks.
        assert!(vis.face_visible(IVec3::new(0, 1, 1), Some(Face::West)));
        assert!(vis.face_visible(IVec3::new(1, 2, 1), Some(Face::Up)));
        // A non-occluding block sitting inside the cavity is hidden entirely.
        assert!(!vis.face_visible(IVec3::ONE, None));
    }

    #[test]
    fn opening_the_cavity_reveals_its_lining() {
        let mut cells = hollow_shell();
        cells.retain(|(cell, _)| *cell != IVec3::new(1, 2, 1));
        let vis = Visibility::compute(cells);
        assert!(vis.is_exposed(IVec3::ONE));
        assert!(vis.face_visible(IVec3::new(0, 1, 1), Some(Face::East)));
        assert!(vis.face_visible(IVec3::ONE, None));
    }

    #[test]
    fn non_occluding_block_does_not_seal_a_cavity() {
        let mut cells = hollow_shell();
        for (cell, mask) in &mut cells {
            if *cell == IVec3::new(1, 2, 1) {
                *mask = 0; // e.g. glass or a fence
            }
        }
        let vis = Visibility::compute(cells);
        assert!(vis.is_exposed(IVec3::ONE));
        assert!(vis.face_visible(IVec3::new(0, 1, 1), Some(Face::East)));
    }

    #[test]
    fn partial_occluder_seals_through_its_covered_side() {
        // The lid covers only its top side (like an upper slab): the cavity
        // and the lid's own cell are sealed, but the lid's top stays visible.
        let lid = IVec3::new(1, 2, 1);
        let mut cells = hollow_shell();
        for (cell, mask) in &mut cells {
            if *cell == lid {
                *mask = Face::Up.bit();
            }
        }
        let vis = Visibility::compute(cells);
        assert!(!vis.is_exposed(IVec3::ONE));
        assert!(!vis.is_exposed(lid));
        assert!(!vis.face_visible(lid, None));
        assert!(!vis.face_visible(IVec3::new(0, 1, 1), Some(Face::East)));
        assert!(vis.face_visible(lid, Some(Face::Up)));
    }

    #[test]
    fn oversized_bounds_fall_back_to_neighbour_culling() {
        let far = IVec3::new(1_000_000, 1_000_000, 1_000_000);
        let vis = Visibility::compute([(IVec3::ZERO, FULL), (IVec3::X, FULL), (far, FULL)]);
        assert!(vis.is_exposed(IVec3::new(5, 5, 5)));
        assert!(!vis.face_visible(IVec3::ZERO, Some(Face::East)));
        assert!(vis.face_visible(IVec3::ZERO, Some(Face::West)));
    }
}
