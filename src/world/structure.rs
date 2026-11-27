//! Reading a Minecraft structure `.nbt` file into a [`World`].

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io::{self, Read};
use std::path::Path;

use flate2::read::GzDecoder;
use glam::IVec3;
use serde::Deserialize;

use super::{BlockState, World};
use crate::constants::{AIR_BLOCK_NAMES, GZIP_MAGIC};

#[derive(Debug)]
pub enum StructureError {
    Io(io::Error),
    Nbt(fastnbt::error::Error),
    /// A block's `pos` does not have exactly three coordinates.
    InvalidPos {
        block: usize,
        len: usize,
    },
    /// A block's `state` is not an index into the palette.
    InvalidState {
        block: usize,
        state: i32,
        palette_len: usize,
    },
}

impl fmt::Display for StructureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "{err}"),
            Self::Nbt(err) => write!(f, "not a structure NBT file: {err}"),
            Self::InvalidPos { block, len } => {
                write!(f, "block {block} has a pos of {len} values, expected 3")
            }
            Self::InvalidState {
                block,
                state,
                palette_len,
            } => write!(
                f,
                "block {block} has state {state}, outside the palette of {palette_len} entries"
            ),
        }
    }
}

impl std::error::Error for StructureError {}

impl From<io::Error> for StructureError {
    fn from(err: io::Error) -> Self {
        Self::Io(err)
    }
}

impl From<fastnbt::error::Error> for StructureError {
    fn from(err: fastnbt::error::Error) -> Self {
        Self::Nbt(err)
    }
}

/// The tags of a structure file that the world keeps. Everything else
/// (`size`, `entities`, `DataVersion`, block entity `nbt`) is ignored.
#[derive(Deserialize)]
struct StructureFile {
    #[serde(default)]
    palette: Vec<PaletteEntry>,
    /// Written instead of `palette` by structures with several random
    /// variants. The first variant is used.
    #[serde(default)]
    palettes: Vec<Vec<PaletteEntry>>,
    #[serde(default)]
    blocks: Vec<StructureBlock>,
}

#[derive(Deserialize)]
struct PaletteEntry {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Properties", default)]
    properties: BTreeMap<String, String>,
}

#[derive(Deserialize)]
struct StructureBlock {
    pos: Vec<i32>,
    state: i32,
}

/// Reads the structure file at `path`.
pub fn load(path: &Path) -> Result<World, StructureError> {
    from_bytes(&fs::read(path)?)
}

/// Parses a structure file, gzipped or not.
pub fn from_bytes(bytes: &[u8]) -> Result<World, StructureError> {
    let file: StructureFile = if bytes.starts_with(&GZIP_MAGIC) {
        let mut raw = Vec::new();
        GzDecoder::new(bytes).read_to_end(&mut raw)?;
        fastnbt::from_bytes(&raw)?
    } else {
        fastnbt::from_bytes(bytes)?
    };

    let entries = if file.palette.is_empty() {
        file.palettes.into_iter().next().unwrap_or_default()
    } else {
        file.palette
    };
    let palette: Vec<BlockState> = entries
        .into_iter()
        .map(|entry| BlockState {
            name: entry.name,
            properties: entry.properties,
        })
        .collect();

    let mut blocks = Vec::with_capacity(file.blocks.len());
    for (index, block) in file.blocks.iter().enumerate() {
        let &[x, y, z] = block.pos.as_slice() else {
            return Err(StructureError::InvalidPos {
                block: index,
                len: block.pos.len(),
            });
        };
        let state = usize::try_from(block.state)
            .ok()
            .filter(|&state| state < palette.len())
            .ok_or(StructureError::InvalidState {
                block: index,
                state: block.state,
                palette_len: palette.len(),
            })?;
        if AIR_BLOCK_NAMES.contains(&palette[state].name.as_str()) {
            continue;
        }
        blocks.push((IVec3::new(x, y, z), state as u32));
    }

    Ok(World::from_structure(palette, blocks))
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use fastnbt::nbt;
    use flate2::Compression;
    use flate2::write::GzEncoder;

    use super::*;

    fn gzip(bytes: &[u8]) -> Vec<u8> {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(bytes).unwrap();
        encoder.finish().unwrap()
    }

    fn sample() -> Vec<u8> {
        fastnbt::to_bytes(&nbt!({
            "DataVersion": 3700,
            "size": [2, 2, 1],
            "palette": [
                { "Name": "minecraft:quartz_block" },
                { "Name": "minecraft:oak_log", "Properties": { "axis": "x" } },
                { "Name": "minecraft:air" },
            ],
            "blocks": [
                { "pos": [0, 0, 0], "state": 0 },
                { "pos": [1, 0, 0], "state": 1 },
                { "pos": [0, 1, 0], "state": 2 },
                { "pos": [1, 1, 0], "state": 0, "nbt": { "id": "minecraft:chest" } },
            ],
            "entities": [],
        }))
        .unwrap()
    }

    #[test]
    fn reads_palette_and_blocks() {
        let world = from_bytes(&sample()).unwrap();
        let mut log_x = BlockState::default_of("oak_log");
        log_x.properties.insert("axis".into(), "x".into());

        assert_eq!(world.palette().len(), 3);
        assert_eq!(
            world.get(IVec3::ZERO),
            Some(&BlockState::default_of("quartz_block"))
        );
        assert_eq!(world.get(IVec3::X), Some(&log_x));
        assert_eq!(
            world.get(IVec3::new(1, 1, 0)),
            Some(&BlockState::default_of("quartz_block"))
        );
        assert_eq!(world.bounds(), Some((IVec3::ZERO, IVec3::new(1, 1, 0))));
    }

    #[test]
    fn air_cells_are_left_empty() {
        let world = from_bytes(&sample()).unwrap();
        assert!(!world.is_occupied(IVec3::Y));
        assert_eq!(world.blocks().count(), 3);
    }

    #[test]
    fn reads_gzipped_files() {
        let world = from_bytes(&gzip(&sample())).unwrap();
        assert_eq!(world.blocks().count(), 3);
    }

    #[test]
    fn imported_world_needs_a_mesh_rebuild() {
        let mut world = from_bytes(&sample()).unwrap();
        assert!(world.take_dirty());
    }

    #[test]
    fn falls_back_to_the_first_of_several_palettes() {
        let bytes = fastnbt::to_bytes(&nbt!({
            "palettes": [
                [{ "Name": "minecraft:stone" }],
                [{ "Name": "minecraft:granite" }],
            ],
            "blocks": [{ "pos": [0, 0, 0], "state": 0 }],
        }))
        .unwrap();
        let world = from_bytes(&bytes).unwrap();
        assert_eq!(
            world.get(IVec3::ZERO),
            Some(&BlockState::default_of("stone"))
        );
    }

    #[test]
    fn rejects_state_outside_the_palette() {
        let bytes = fastnbt::to_bytes(&nbt!({
            "palette": [{ "Name": "minecraft:stone" }],
            "blocks": [{ "pos": [0, 0, 0], "state": 1 }],
        }))
        .unwrap();
        assert!(matches!(
            from_bytes(&bytes),
            Err(StructureError::InvalidState {
                block: 0,
                state: 1,
                palette_len: 1
            })
        ));
    }

    #[test]
    fn rejects_malformed_pos() {
        let bytes = fastnbt::to_bytes(&nbt!({
            "palette": [{ "Name": "minecraft:stone" }],
            "blocks": [{ "pos": [0, 0], "state": 0 }],
        }))
        .unwrap();
        assert!(matches!(
            from_bytes(&bytes),
            Err(StructureError::InvalidPos { block: 0, len: 2 })
        ));
    }

    #[test]
    fn rejects_non_nbt_data() {
        assert!(matches!(
            from_bytes(b"not nbt"),
            Err(StructureError::Nbt(_))
        ));
    }
}
