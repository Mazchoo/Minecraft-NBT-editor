//! Hand-built geometry for mob heads.
//!
//! Vanilla draws heads with a block-entity renderer, from each mob's entity
//! texture; every head block shares the geometry-less `skull` model. The boxes
//! here are that renderer's head models, cut from the same textures.

use std::collections::BTreeMap;

use super::element::Element;
use super::entity_model::{self, AtlasTile, ModelBox, Placement};
use super::sign::{facing_yaw, rotation_yaw};
use crate::constants::{
    CREEPER_HEAD_TEXTURE, DRAGON_ATLAS_SIZE, DRAGON_HEAD_LIFT, DRAGON_HEAD_SCALE,
    DRAGON_HEAD_TEXTURE, HEAD_ATLAS_SIZE, HEAD_HAT_INFLATE, HEAD_TILE_PREFIX, PIGLIN_HEAD_TEXTURE,
    PLAYER_HEAD_TEXTURES, SKELETON_SKULL_TEXTURE, WITHER_SKELETON_SKULL_TEXTURE,
    ZOMBIE_HEAD_TEXTURE,
};
use crate::custom_block_keys::CustomBlockKey;

/// The mob a head comes from, which picks its model and texture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeadMob {
    Skeleton,
    WitherSkeleton,
    Zombie,
    /// A head without an owner, showing the default skin.
    Player,
    Creeper,
    Piglin,
    Dragon,
}

/// A head block: the mob, and whether it hangs on a wall.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeadKind {
    pub mob: HeadMob,
    /// On a wall, oriented by `facing`; otherwise on the floor, turned in
    /// 22.5 degree steps by `rotation`.
    pub wall: bool,
}

impl HeadKind {
    /// Recognises a head from its block name (`minecraft:creeper_wall_head`).
    pub fn from_name(name: &str) -> Option<Self> {
        let (mob, wall) = match CustomBlockKey::from_name(name)? {
            CustomBlockKey::SkeletonSkull => (HeadMob::Skeleton, false),
            CustomBlockKey::SkeletonWallSkull => (HeadMob::Skeleton, true),
            CustomBlockKey::WitherSkeletonSkull => (HeadMob::WitherSkeleton, false),
            CustomBlockKey::WitherSkeletonWallSkull => (HeadMob::WitherSkeleton, true),
            CustomBlockKey::ZombieHead => (HeadMob::Zombie, false),
            CustomBlockKey::ZombieWallHead => (HeadMob::Zombie, true),
            CustomBlockKey::PlayerHead => (HeadMob::Player, false),
            CustomBlockKey::PlayerWallHead => (HeadMob::Player, true),
            CustomBlockKey::CreeperHead => (HeadMob::Creeper, false),
            CustomBlockKey::CreeperWallHead => (HeadMob::Creeper, true),
            CustomBlockKey::PiglinHead => (HeadMob::Piglin, false),
            CustomBlockKey::PiglinWallHead => (HeadMob::Piglin, true),
            CustomBlockKey::DragonHead => (HeadMob::Dragon, false),
            CustomBlockKey::DragonWallHead => (HeadMob::Dragon, true),
            _ => return None,
        };
        Some(Self { mob, wall })
    }

    /// Boxes for this head, already turned to match `properties`.
    pub(crate) fn elements(self, properties: &BTreeMap<String, String>) -> Vec<Element> {
        let placement = self.mob.placement(self.wall);
        // The boxes are laid out facing south. A floor head's `rotation`
        // points its face the opposite way to a sign's: 0 is north.
        let yaw = if self.wall {
            facing_yaw(properties)
        } else {
            rotation_yaw(properties) + 180.0
        };
        let atlas = self.mob.atlas();
        self.mob
            .boxes()
            .iter()
            .map(|model_box| {
                placement.element(
                    model_box,
                    |face| entity_model::tile_key(HEAD_TILE_PREFIX, atlas, model_box.part, face),
                    |_| false,
                    yaw,
                )
            })
            .collect()
    }
}

impl HeadMob {
    const ALL: [Self; 7] = [
        Self::Skeleton,
        Self::WitherSkeleton,
        Self::Zombie,
        Self::Player,
        Self::Creeper,
        Self::Piglin,
        Self::Dragon,
    ];

    /// The mob whose head is drawn from the texture at `path` under
    /// `textures/` (`entity/creeper/creeper`).
    pub(crate) fn from_texture(path: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|mob| mob.textures().contains(&path))
    }

    /// Texture paths this head may be drawn from, under `textures/`.
    fn textures(self) -> &'static [&'static str] {
        match self {
            Self::Skeleton => &[SKELETON_SKULL_TEXTURE],
            Self::WitherSkeleton => &[WITHER_SKELETON_SKULL_TEXTURE],
            Self::Zombie => &[ZOMBIE_HEAD_TEXTURE],
            Self::Player => &PLAYER_HEAD_TEXTURES,
            Self::Creeper => &[CREEPER_HEAD_TEXTURE],
            Self::Piglin => &[PIGLIN_HEAD_TEXTURE],
            Self::Dragon => &[DRAGON_HEAD_TEXTURE],
        }
    }

    /// Names the texture in tile keys.
    fn atlas(self) -> &'static str {
        entity_model::atlas_stem(self.textures()[0])
    }

    pub(crate) fn atlas_size(self) -> u32 {
        match self {
            Self::Dragon => DRAGON_ATLAS_SIZE,
            _ => HEAD_ATLAS_SIZE,
        }
    }

    /// Tiles to cut from this head's texture.
    pub(crate) fn tiles(self) -> Vec<AtlasTile> {
        entity_model::tiles(HEAD_TILE_PREFIX, self.atlas(), &self.boxes(), |_, _| false)
    }

    /// Vanilla's head model, in entity units: Y down, face on -Z.
    fn boxes(self) -> Vec<ModelBox> {
        let head = ModelBox::new("head", [-4.0, -8.0, -4.0], [8, 8, 8], [0, 0]);
        match self {
            Self::Skeleton | Self::WitherSkeleton | Self::Creeper => vec![head],
            Self::Zombie | Self::Player => vec![
                head,
                ModelBox::new("hat", [-4.0, -8.0, -4.0], [8, 8, 8], [32, 0])
                    .inflated(HEAD_HAT_INFLATE),
            ],
            // The ears hang at 30 degrees, which one element cannot turn
            // along with the head's yaw; they are left out.
            Self::Piglin => vec![
                ModelBox::new("head", [-5.0, -8.0, -4.0], [10, 8, 8], [0, 0]),
                ModelBox::new("snout", [-2.0, -4.0, -5.0], [4, 4, 1], [31, 1]),
                ModelBox::new("left_tusk", [2.0, -2.0, -5.0], [1, 2, 1], [2, 4]),
                ModelBox::new("right_tusk", [-3.0, -2.0, -5.0], [1, 2, 1], [2, 0]),
            ],
            Self::Dragon => vec![
                ModelBox::new("upper_lip", [-6.0, -1.0, -24.0], [12, 5, 16], [176, 44]),
                ModelBox::new("upper_head", [-8.0, -8.0, -10.0], [16, 16, 16], [112, 30]),
                ModelBox::new("scale", [-5.0, -12.0, -4.0], [2, 4, 6], [0, 0]).mirrored(),
                ModelBox::new("nostril", [-5.0, -3.0, -22.0], [2, 2, 4], [112, 0]).mirrored(),
                ModelBox::new("scale", [3.0, -12.0, -4.0], [2, 4, 6], [0, 0]),
                ModelBox::new("nostril", [3.0, -3.0, -22.0], [2, 2, 4], [112, 0]),
                // The jaw hangs from the head at (0, 4, -8), closed.
                ModelBox::new("jaw", [-6.0, 4.0, -24.0], [12, 4, 16], [176, 65]),
            ],
        }
    }

    /// Vanilla's head pose, turned to face south: the model is flipped to
    /// Y up and its face brought round to +Z. A wall head sits a quarter
    /// block up, pushed a quarter block back against the wall.
    fn placement(self, wall: bool) -> Placement {
        let (scale, lift) = match self {
            Self::Dragon => (DRAGON_HEAD_SCALE, DRAGON_HEAD_LIFT),
            _ => (1.0, 0.0),
        };
        let offset = if wall {
            [8.0, lift + 4.0, 4.0]
        } else {
            [8.0, lift, 8.0]
        };
        Placement::new([0, 1, 2], [scale, -scale, -scale], offset)
    }
}

#[cfg(test)]
mod tests {
    use glam::Vec3;

    use super::*;
    use crate::models::block::Face;

    fn props(key: &str, value: &str) -> BTreeMap<String, String> {
        BTreeMap::from([(key.to_owned(), value.to_owned())])
    }

    /// Bounds of an element after its own rotation.
    fn bounds(element: &Element) -> (Vec3, Vec3) {
        let (from, to) = (element.from(), element.to());
        let mut min = Vec3::splat(f32::MAX);
        let mut max = Vec3::splat(f32::MIN);
        for i in 0..8 {
            let mut p = Vec3::new(
                if i & 1 == 0 { from.x } else { to.x },
                if i & 2 == 0 { from.y } else { to.y },
                if i & 4 == 0 { from.z } else { to.z },
            );
            if let Some(rotation) = element.rotation() {
                p = rotation.apply(p);
            }
            min = min.min(p);
            max = max.max(p);
        }
        (min, max)
    }

    fn close(a: Vec3, b: [f32; 3]) -> bool {
        (a - Vec3::from_array(b)).abs().max_element() < 1e-3
    }

    #[test]
    fn kind_comes_from_the_block_name() {
        assert_eq!(
            HeadKind::from_name("minecraft:creeper_wall_head"),
            Some(HeadKind {
                mob: HeadMob::Creeper,
                wall: true
            })
        );
        assert_eq!(
            HeadKind::from_name("minecraft:wither_skeleton_skull"),
            Some(HeadKind {
                mob: HeadMob::WitherSkeleton,
                wall: false
            })
        );
        assert_eq!(HeadKind::from_name("minecraft:skull"), None);
        assert_eq!(HeadMob::from_texture("entity/steve"), Some(HeadMob::Player));
        assert_eq!(HeadMob::from_texture("entity/skeleton/stray"), None);
    }

    #[test]
    fn floor_head_is_a_half_block_cube_with_its_face_to_the_north_at_rotation_0() {
        let kind = HeadKind::from_name("minecraft:creeper_head").unwrap();
        let elements = kind.elements(&props("rotation", "0"));
        assert_eq!(elements.len(), 1);
        let (min, max) = bounds(&elements[0]);
        assert!(close(min, [4.0, 0.0, 4.0]) && close(max, [12.0, 8.0, 12.0]));
        // Laid out facing south, then swung half a turn.
        let face = elements[0].faces()[Face::South.index()].as_ref().unwrap();
        assert_eq!(face.texture(), "head_creeper_head_north");
        assert_eq!(elements[0].rotation().unwrap().angle(), 180.0);
        let top = elements[0].faces()[Face::Up.index()].as_ref().unwrap();
        assert_eq!(top.texture(), "head_creeper_head_down");
    }

    #[test]
    fn wall_head_sits_against_the_wall_behind_it() {
        let kind = HeadKind::from_name("minecraft:zombie_wall_head").unwrap();
        let elements = kind.elements(&props("facing", "east"));
        assert_eq!(elements.len(), 2);
        let (min, max) = bounds(&elements[0]);
        assert!(close(min, [0.0, 4.0, 4.0]) && close(max, [8.0, 12.0, 12.0]));
        let face = elements[0].faces()[Face::South.index()].as_ref().unwrap();
        assert_eq!(face.texture(), "head_zombie_head_north");
        // The face is the south side swung round to the east.
        assert_eq!(elements[0].rotation().unwrap().angle(), 90.0);
        let (hat_min, hat_max) = bounds(&elements[1]);
        assert!(close(hat_min, [-0.25, 3.75, 3.75]) && close(hat_max, [8.25, 12.25, 12.25]));
    }

    #[test]
    fn dragon_head_snout_reaches_out_of_the_block() {
        let kind = HeadKind::from_name("minecraft:dragon_head").unwrap();
        let elements = kind.elements(&props("rotation", "8"));
        assert_eq!(elements.len(), 7);
        let (min, max) = bounds(&elements[1]);
        assert!(close(min, [2.0, -0.01, 3.5]) && close(max, [14.0, 11.99, 15.5]));
        let (_, lip_max) = bounds(&elements[0]);
        assert!(lip_max.z > 16.0, "rotation 8 faces south");
        let face = elements[1].faces()[Face::South.index()].as_ref().unwrap();
        assert_eq!(face.texture(), "head_dragon_upper_head_north");
    }
}
