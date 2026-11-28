//! Hand-built geometry and baked surface for the end portal and end gateway.
//!
//! Vanilla draws both with a block-entity renderer and a shader that layers
//! the star texture over the end sky in screen space. The editor bakes those
//! layers once, at import, into one block-sized texture ([`bake_surface`]).
//! The end portal is a slab whose top and bottom alone are drawn; the end
//! gateway is a full cube.

use glam::Vec3;
use image::{Rgba, RgbaImage};

use super::block::Face;
use super::element::{Element, ElementFace, default_uv};
use crate::config::Config;
use crate::constants::{
    END_PORTAL_BOTTOM, END_PORTAL_LAYER_COLOURS, END_PORTAL_STAR_POOL, END_PORTAL_SURFACE_TEXTURE,
    END_PORTAL_TOP,
};
use crate::custom_block_keys::CustomBlockKey;

/// Which block draws the end portal surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndPortalKind {
    Portal,
    Gateway,
}

impl EndPortalKind {
    pub fn from_name(name: &str) -> Option<Self> {
        match CustomBlockKey::from_name(name)? {
            CustomBlockKey::EndPortal => Some(Self::Portal),
            CustomBlockKey::EndGateway => Some(Self::Gateway),
            _ => None,
        }
    }

    /// The surface, unshaded as vanilla draws it.
    pub(crate) fn elements(self) -> Vec<Element> {
        let (from, to, drawn): (_, _, &[Face]) = match self {
            Self::Portal => (
                Vec3::new(0.0, END_PORTAL_BOTTOM, 0.0),
                Vec3::new(16.0, END_PORTAL_TOP, 16.0),
                &[Face::Down, Face::Up],
            ),
            Self::Gateway => (Vec3::ZERO, Vec3::splat(16.0), &Face::ALL),
        };
        let mut faces = [None, None, None, None, None, None];
        for &face in drawn {
            let cullface = matches!(self, Self::Gateway).then_some(face);
            faces[face.index()] = Some(ElementFace::new(
                END_PORTAL_SURFACE_TEXTURE.to_owned(),
                default_uv(face, from, to),
                0,
                cullface,
                None,
            ));
        }
        vec![Element::new(from, to, None, faces, false)]
    }
}

/// The surface texture, [`Config::TEXTURE_SIZE`] square: the sky weighted by
/// the first layer colour, plus one window of `stars` per further colour.
/// Each surface texel takes the brightest texel of an
/// [`END_PORTAL_STAR_POOL`]-wide square of its window. The star texture
/// repeats, so any window is as good as another.
pub(crate) fn bake_surface(sky: &RgbaImage, stars: &RgbaImage) -> RgbaImage {
    let size = Config::TEXTURE_SIZE;
    let pool = END_PORTAL_STAR_POOL;
    let span = size * pool;
    let (sky_colour, layer_colours) = END_PORTAL_LAYER_COLOURS
        .split_first()
        .expect("the backdrop colour comes first");
    RgbaImage::from_fn(size, size, |x, y| {
        let backdrop = sample(sky, x * sky.width() / size, y * sky.height() / size);
        let mut colour = weighted(backdrop, sky_colour);
        for (layer, weight) in (1u32..).zip(layer_colours) {
            // Spread the windows over the star texture.
            let window = [layer * 17 * span, layer * layer * 31 * span];
            let star = (0..pool * pool)
                .map(|i| {
                    sample(
                        stars,
                        window[0] + x * pool + i % pool,
                        window[1] + y * pool + i / pool,
                    )
                })
                .max_by_key(|texel| texel.0[..3].iter().map(|&c| u32::from(c)).sum::<u32>())
                .unwrap_or(Rgba([0; 4]));
            let added = weighted(star, weight);
            for (channel, add) in colour.iter_mut().zip(added) {
                *channel += add;
            }
        }
        Rgba([
            to_byte(colour[0]),
            to_byte(colour[1]),
            to_byte(colour[2]),
            u8::MAX,
        ])
    })
}

/// The texel at (`x`, `y`), repeating the image.
fn sample(image: &RgbaImage, x: u32, y: u32) -> Rgba<u8> {
    *image.get_pixel(x % image.width().max(1), y % image.height().max(1))
}

fn weighted(texel: Rgba<u8>, weight: &[f32; 3]) -> [f32; 3] {
    [0, 1, 2].map(|i| f32::from(texel.0[i]) * weight[i])
}

fn to_byte(value: f32) -> u8 {
    value.round().clamp(0.0, 255.0) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portal_is_a_slab_drawn_top_and_bottom() {
        assert_eq!(
            EndPortalKind::from_name("minecraft:end_portal"),
            Some(EndPortalKind::Portal)
        );
        assert_eq!(
            EndPortalKind::from_name("minecraft:end_gateway"),
            Some(EndPortalKind::Gateway)
        );
        assert_eq!(EndPortalKind::from_name("minecraft:end_portal_frame"), None);
        let elements = EndPortalKind::Portal.elements();
        assert_eq!(elements.len(), 1);
        assert_eq!(elements[0].from().y, END_PORTAL_BOTTOM);
        assert_eq!(elements[0].to().y, END_PORTAL_TOP);
        let drawn: Vec<Face> = Face::ALL
            .into_iter()
            .filter(|face| elements[0].faces()[face.index()].is_some())
            .collect();
        assert_eq!(drawn, vec![Face::Down, Face::Up]);
    }

    #[test]
    fn gateway_is_a_full_cube_culled_by_neighbours() {
        let elements = EndPortalKind::Gateway.elements();
        for face in Face::ALL {
            let drawn = elements[0].faces()[face.index()].as_ref().unwrap();
            assert_eq!(drawn.texture(), END_PORTAL_SURFACE_TEXTURE);
            assert_eq!(drawn.cullface(), Some(face));
        }
    }

    #[test]
    fn surface_is_dark_sky_lit_by_stars() {
        let sky = RgbaImage::from_pixel(8, 8, Rgba([100, 100, 100, 255]));
        let dark = RgbaImage::from_pixel(8, 8, Rgba([0, 0, 0, 255]));
        let surface = bake_surface(&sky, &dark);
        assert_eq!(surface.width(), Config::TEXTURE_SIZE);
        let [r, g, b] = END_PORTAL_LAYER_COLOURS[0].map(|w| to_byte(100.0 * w));
        assert_eq!(*surface.get_pixel(3, 5), Rgba([r, g, b, 255]));

        let bright = RgbaImage::from_pixel(8, 8, Rgba([255, 255, 255, 255]));
        let lit = bake_surface(&sky, &bright);
        assert!(lit.get_pixel(3, 5).0[2] > surface.get_pixel(3, 5).0[2]);
    }
}
