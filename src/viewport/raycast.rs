use glam::{IVec3, Vec3};

use crate::models::Face;

/// Maximum ray travel, in blocks, for placement picking.
const MAX_DISTANCE: f32 = 256.0;
const DIR_EPS: f32 = 1e-8;
const T_EPS: f32 = 1e-5;

/// A face the cursor ray hit: an occupied cell, or the ground plane treated as
/// the `Up` face of the virtual cell at `y = -1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FaceHit {
    pub cell: IVec3,
    pub face: Face,
}

impl FaceHit {
    /// Empty neighbour on the outward side of the hit face.
    pub fn placement_cell(self) -> IVec3 {
        self.cell + self.face.offset()
    }

    /// World-space centre of the hit face.
    pub fn face_centre(self) -> Vec3 {
        self.cell.as_vec3() + Vec3::splat(0.5) + self.face.offset().as_vec3() * 0.5
    }
}

/// Nearest hit along `origin + t * dir` (`dir` need not be normalised, `t > 0`).
///
/// Occupied voxels win over a coplanar ground hit so clicking the underside of
/// a block at `y = 0` targets that face rather than the ground cell it occupies.
pub fn pick(origin: Vec3, dir: Vec3, occupied: impl Fn(IVec3) -> bool) -> Option<FaceHit> {
    if dir.length_squared() < DIR_EPS * DIR_EPS {
        return None;
    }
    let block = voxel_hit(origin, dir, &occupied);
    let ground = ground_hit(origin, dir, &occupied);
    match (block, ground) {
        (Some((bt, bhit)), Some((gt, ghit))) => {
            if bt <= gt + T_EPS {
                Some(bhit)
            } else {
                Some(ghit)
            }
        }
        (Some((_, hit)), None) | (None, Some((_, hit))) => Some(hit),
        (None, None) => None,
    }
}

fn ground_hit(origin: Vec3, dir: Vec3, occupied: impl Fn(IVec3) -> bool) -> Option<(f32, FaceHit)> {
    if dir.y.abs() < DIR_EPS {
        return None;
    }
    let t = -origin.y / dir.y;
    if t < T_EPS || t > MAX_DISTANCE {
        return None;
    }
    let hit = origin + dir * t;
    let cell = IVec3::new(floor_i(hit.x), 0, floor_i(hit.z));
    if occupied(cell) {
        return None;
    }
    Some((
        t,
        FaceHit {
            cell: IVec3::new(cell.x, -1, cell.z),
            face: Face::Up,
        },
    ))
}

fn voxel_hit(origin: Vec3, dir: Vec3, occupied: impl Fn(IVec3) -> bool) -> Option<(f32, FaceHit)> {
    let mut x = floor_i(origin.x);
    let mut y = floor_i(origin.y);
    let mut z = floor_i(origin.z);

    let step_x = sign(dir.x);
    let step_y = sign(dir.y);
    let step_z = sign(dir.z);

    let t_delta_x = inv_abs(dir.x);
    let t_delta_y = inv_abs(dir.y);
    let t_delta_z = inv_abs(dir.z);

    let mut t_max_x = t_to_boundary(origin.x, dir.x, x, step_x);
    let mut t_max_y = t_to_boundary(origin.y, dir.y, y, step_y);
    let mut t_max_z = t_to_boundary(origin.z, dir.z, z, step_z);

    // The voxel containing the eye is skipped; we report the first cell entered.
    for _ in 0..512 {
        let (t, axis, step) = if t_max_x <= t_max_y && t_max_x <= t_max_z {
            let t = t_max_x;
            t_max_x += t_delta_x;
            x += step_x;
            (t, 0, step_x)
        } else if t_max_y <= t_max_z {
            let t = t_max_y;
            t_max_y += t_delta_y;
            y += step_y;
            (t, 1, step_y)
        } else {
            let t = t_max_z;
            t_max_z += t_delta_z;
            z += step_z;
            (t, 2, step_z)
        };
        if t > MAX_DISTANCE {
            return None;
        }
        if occupied(IVec3::new(x, y, z)) {
            return Some((
                t,
                FaceHit {
                    cell: IVec3::new(x, y, z),
                    face: entered_face(axis, step),
                },
            ));
        }
    }
    None
}

fn entered_face(axis: i32, step: i32) -> Face {
    match (axis, step > 0) {
        (0, true) => Face::West,
        (0, false) => Face::East,
        (1, true) => Face::Down,
        (1, false) => Face::Up,
        (2, true) => Face::North,
        _ => Face::South,
    }
}

fn floor_i(v: f32) -> i32 {
    v.floor() as i32
}

fn sign(v: f32) -> i32 {
    if v > DIR_EPS {
        1
    } else if v < -DIR_EPS {
        -1
    } else {
        0
    }
}

fn inv_abs(v: f32) -> f32 {
    if v.abs() < DIR_EPS {
        f32::INFINITY
    } else {
        1.0 / v.abs()
    }
}

fn t_to_boundary(origin: f32, dir: f32, voxel: i32, step: i32) -> f32 {
    if step == 0 {
        return f32::INFINITY;
    }
    let boundary = if step > 0 {
        voxel as f32 + 1.0
    } else {
        voxel as f32
    };
    (boundary - origin) / dir
}

#[cfg(test)]
mod tests {
    use super::*;

    fn none(_: IVec3) -> bool {
        false
    }

    #[test]
    fn downward_ray_hits_ground_cell() {
        let hit = pick(Vec3::new(0.5, 10.0, 0.5), Vec3::NEG_Y, none).unwrap();
        assert_eq!(hit.cell, IVec3::new(0, -1, 0));
        assert_eq!(hit.face, Face::Up);
        assert_eq!(hit.placement_cell(), IVec3::ZERO);
    }

    #[test]
    fn negative_xz_ground_uses_floor() {
        let hit = pick(Vec3::new(-0.25, 4.0, -1.75), Vec3::NEG_Y, none).unwrap();
        assert_eq!(hit.placement_cell(), IVec3::new(-1, 0, -2));
    }

    #[test]
    fn block_top_face_places_above() {
        let occupied = |c: IVec3| c == IVec3::ZERO;
        let hit = pick(Vec3::new(0.5, 10.0, 0.5), Vec3::NEG_Y, occupied).unwrap();
        assert_eq!(hit.cell, IVec3::ZERO);
        assert_eq!(hit.face, Face::Up);
        assert_eq!(hit.placement_cell(), IVec3::Y);
    }

    #[test]
    fn block_east_face_places_to_the_east() {
        let occupied = |c: IVec3| c == IVec3::ZERO;
        let hit = pick(Vec3::new(5.5, 0.5, 0.5), Vec3::NEG_X, occupied).unwrap();
        assert_eq!(hit.cell, IVec3::ZERO);
        assert_eq!(hit.face, Face::East);
        assert_eq!(hit.placement_cell(), IVec3::X);
    }

    #[test]
    fn ground_under_an_occupied_cell_is_ignored() {
        let occupied = |c: IVec3| c == IVec3::new(2, 0, 3);
        // Looking straight down onto that column still hits the block, not the ground.
        let hit = pick(Vec3::new(2.5, 8.0, 3.5), Vec3::NEG_Y, occupied).unwrap();
        assert_eq!(hit.cell, IVec3::new(2, 0, 3));
        assert_eq!(hit.face, Face::Up);
    }

    #[test]
    fn skyward_ray_misses_empty_world() {
        assert!(pick(Vec3::new(0.5, 2.0, 0.5), Vec3::Y, none).is_none());
    }
}
