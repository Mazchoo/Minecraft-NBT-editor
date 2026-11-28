use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};

use image::RgbaImage;

use crate::config::Config;
use crate::constants::ENTITY_TEXTURE_DIR;
use crate::models::entity_model::{EntityAtlas, entity_atlas};
use crate::textures::{ImageFormat, Texture, inspect_alpha};

use super::{JarError, for_each_jar_file};

/// Extracts `assets/{namespace}/textures/block/*.{png,bmp,jpg}` into `cache_dir`,
/// plus the tiles cut from the entity atlases the editor draws blocks from.
///
/// Each block texture is written under its original file name. The returned map is keyed
/// by file stem (e.g. `glass`).
pub fn extract_from_jar(
    jar_path: &Path,
    cache_dir: &Path,
) -> Result<HashMap<String, Texture>, JarError> {
    fs::create_dir_all(cache_dir)?;
    let mut textures = HashMap::new();
    for_each_jar_file(jar_path, |name, bytes| {
        if let Some((path, atlas)) = match_entity_atlas(name) {
            textures.extend(ingest_entity_atlas(cache_dir, &path, &atlas, &bytes));
        }
        let Some((file_name, format)) = match_block_texture(name) else {
            return;
        };
        if let Some((key, texture)) = ingest_texture(cache_dir, file_name, format, &bytes) {
            textures.insert(key, texture);
        }
    })?;
    remove_stale_cache_files(cache_dir, &textures);
    Ok(textures)
}

/// Writes a matched texture into `cache_dir` and returns `(stem, record)`.
pub(crate) fn ingest_texture(
    cache_dir: &Path,
    file_name: String,
    format: ImageFormat,
    bytes: &[u8],
) -> Option<(String, Texture)> {
    let dest = cache_dir.join(&file_name);
    if let Err(err) = fs::write(&dest, bytes) {
        log::warn!("failed to write {}: {err}", dest.display());
        return None;
    }
    let has_alpha = inspect_alpha(bytes, format);
    let key = file_stem(&file_name);
    Some((key, Texture::new(file_name, format, has_alpha)))
}

/// Extracts textures, writes the manifest, and returns the new library map.
pub(crate) fn import_into_cache(
    jar_path: &Path,
    cache_dir: &Path,
) -> Result<HashMap<String, Texture>, JarError> {
    let textures = extract_from_jar(jar_path, cache_dir)?;
    write_manifest(
        &cache_dir.join(crate::constants::TEXTURES_MANIFEST_FILE),
        &textures,
    )?;
    Ok(textures)
}

/// Matches `assets/{namespace}/textures/block/{file}.{png|bmp|jpg}`.
pub(crate) fn match_block_texture(entry_name: &str) -> Option<(String, ImageFormat)> {
    match_texture(entry_name, &["block"])
}

/// Matches `assets/{namespace}/textures/entity/{path}.png` when it is an
/// atlas the editor cuts into tiles, returning the path under `textures/`
/// (`entity/chest/normal_left`) and the atlas.
pub(crate) fn match_entity_atlas(entry_name: &str) -> Option<(String, EntityAtlas)> {
    let name = entry_name.replace('\\', "/");
    let mut parts = name.splitn(4, '/');
    let (assets, namespace, textures, rest) =
        (parts.next()?, parts.next()?, parts.next()?, parts.next()?);
    if assets != "assets" || namespace.is_empty() || textures != "textures" {
        return None;
    }
    let path = rest.strip_suffix(".png")?;
    if !path.starts_with(ENTITY_TEXTURE_DIR) || path.contains("..") {
        return None;
    }
    let atlas = entity_atlas(path)?;
    Some((path.to_owned(), atlas))
}

/// Matches an image file directly inside `assets/{namespace}/textures/{dirs}/`.
fn match_texture(entry_name: &str, dirs: &[&str]) -> Option<(String, ImageFormat)> {
    let name = if entry_name.contains('\\') {
        Cow::Owned(entry_name.replace('\\', "/"))
    } else {
        Cow::Borrowed(entry_name)
    };
    let mut parts = name.split('/');
    let assets = parts.next()?;
    let namespace = parts.next()?;
    let textures = parts.next()?;
    if assets != "assets" || textures != "textures" {
        return None;
    }
    for dir in dirs {
        if parts.next()? != *dir {
            return None;
        }
    }
    let file = parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    if namespace.is_empty() || file.is_empty() || file.contains("..") {
        return None;
    }
    let (_, ext) = file.rsplit_once('.')?;
    let format = ImageFormat::from_extension(ext)?;
    Some((file.to_owned(), format))
}

/// Cuts the entity texture at `path` into one tile per drawn box face and
/// writes each into `cache_dir`, at the top-left of a square the size of a
/// block texture so a tile's texels line up with model units. Returns
/// `(key, record)` per tile.
pub(crate) fn ingest_entity_atlas(
    cache_dir: &Path,
    path: &str,
    layout: &EntityAtlas,
    bytes: &[u8],
) -> Vec<(String, Texture)> {
    let atlas = match image::load_from_memory_with_format(bytes, image::ImageFormat::Png) {
        Ok(image) => image.to_rgba8(),
        Err(err) => {
            log::warn!("failed to decode entity texture {path}: {err}");
            return Vec::new();
        }
    };
    let scale = (atlas.width() / layout.size).max(1);
    let side = Config::TEXTURE_SIZE * scale;
    let mut textures = Vec::new();
    for tile in &layout.tiles {
        let [x, y, width, height] = tile.region.map(|v| v * scale);
        if x + width > atlas.width() || y + height > atlas.height() {
            log::warn!("entity texture {path} is too small for {}", tile.key);
            continue;
        }
        let mut canvas = RgbaImage::new(side, side);
        let face = image::imageops::crop_imm(&atlas, x, y, width, height).to_image();
        image::imageops::replace(&mut canvas, &face, 0, 0);
        let mut encoded = Vec::new();
        if let Err(err) = canvas.write_to(&mut Cursor::new(&mut encoded), image::ImageFormat::Png) {
            log::warn!("failed to encode {}: {err}", tile.key);
            continue;
        }
        textures.extend(ingest_texture(
            cache_dir,
            format!("{}.png", tile.key),
            ImageFormat::Png,
            &encoded,
        ));
    }
    textures
}

pub(crate) fn remove_stale_cache_files(cache_dir: &Path, textures: &HashMap<String, Texture>) {
    let keep: HashSet<&str> = textures.values().map(Texture::file_name).collect();
    let entries = match fs::read_dir(cache_dir) {
        Ok(entries) => entries,
        Err(err) => {
            log::warn!(
                "could not list cache directory {}: {err}",
                cache_dir.display()
            );
            return;
        }
    };
    for entry in entries {
        let Ok(entry) = entry else {
            continue;
        };
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if name == crate::constants::TEXTURES_MANIFEST_FILE || keep.contains(name) {
            continue;
        }
        if let Err(err) = fs::remove_file(&path) {
            log::warn!(
                "failed to remove stale cache file {}: {err}",
                path.display()
            );
        }
    }
}

fn file_stem(file_name: &str) -> String {
    Path::new(file_name)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.is_empty())
        .unwrap_or(file_name)
        .to_owned()
}

pub(crate) fn write_manifest(
    path: &Path,
    textures: &HashMap<String, Texture>,
) -> Result<PathBuf, JarError> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }
    let mut file = File::create(path)?;
    serde_json::to_writer_pretty(&mut file, textures)?;
    file.flush()?;
    file.sync_all()?;
    Ok(path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};
    use std::io::{Cursor, Write};
    use zip::ZipWriter;
    use zip::write::SimpleFileOptions;

    fn encode_png(color: [u8; 4]) -> Vec<u8> {
        let img = RgbaImage::from_pixel(2, 2, Rgba(color));
        let mut bytes = Vec::new();
        img.write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)
            .unwrap();
        bytes
    }

    fn temp_dir(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "minecraft-nbt-editor-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn match_block_texture_accepts_client_layout() {
        let (name, format) =
            match_block_texture("assets/minecraft/textures/block/glass.png").unwrap();
        assert_eq!(name, "glass.png");
        assert_eq!(format, ImageFormat::Png);
        assert!(match_block_texture("assets/minecraft/models/block/glass.json").is_none());
        assert!(match_block_texture("assets/minecraft/textures/block/sub/glass.png").is_none());
        assert!(match_block_texture("assets/minecraft/textures/item/stick.png").is_none());
        assert!(match_block_texture("assets/minecraft/textures/block/glass.json").is_none());
        let (name, format) =
            match_block_texture("assets\\minecraft\\textures\\block\\glass.png").unwrap();
        assert_eq!(name, "glass.png");
        assert_eq!(format, ImageFormat::Png);
    }

    #[test]
    fn match_entity_atlas_accepts_only_atlases_the_editor_cuts() {
        let path = |name: &str| match_entity_atlas(name).map(|(path, _)| path);
        assert_eq!(
            path("assets/minecraft/textures/entity/chest/normal_left.png").as_deref(),
            Some("entity/chest/normal_left")
        );
        assert_eq!(
            path("assets/minecraft/textures/entity/player/wide/steve.png").as_deref(),
            Some("entity/player/wide/steve")
        );
        assert!(path("assets/minecraft/textures/entity/creeper/creeper.png").is_some());
        assert!(path("assets/minecraft/textures/entity/skeleton/stray.png").is_none());
        assert!(path("assets/minecraft/textures/entity/conduit/wind.png.mcmeta").is_none());
        assert!(path("assets/minecraft/textures/block/chest.png").is_none());
        assert!(match_block_texture("assets/minecraft/textures/entity/chest/normal.png").is_none());
    }

    #[test]
    fn chest_atlas_is_cut_into_face_tiles() {
        let dir = temp_dir("chest-atlas");
        // Every texel records its own position in the atlas.
        let atlas = RgbaImage::from_fn(64, 64, |x, y| Rgba([x as u8, y as u8, 0, 255]));
        let mut bytes = Vec::new();
        atlas
            .write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)
            .unwrap();

        let (path, layout) =
            match_entity_atlas("assets/minecraft/textures/entity/chest/normal.png").unwrap();
        let textures: HashMap<String, Texture> = ingest_entity_atlas(&dir, &path, &layout, &bytes)
            .into_iter()
            .collect();
        assert_eq!(textures.len(), 15);
        let record = &textures["chest_normal_lid_south"];
        assert_eq!(record.file_name(), "chest_normal_lid_south.png");
        let tile = image::open(dir.join(record.file_name()))
            .unwrap()
            .to_rgba8();
        assert_eq!(tile.dimensions(), (16, 16));
        // The 14x5 face sits at the top-left; the rest is transparent.
        assert_eq!(tile.get_pixel(0, 0).0, [42, 14, 0, 255]);
        assert_eq!(tile.get_pixel(13, 4).0, [55, 18, 0, 255]);
        assert_eq!(tile.get_pixel(14, 0).0[3], 0);
        assert_eq!(tile.get_pixel(0, 5).0[3], 0);

        assert!(ingest_entity_atlas(&dir, &path, &layout, b"not a png").is_empty());
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn extract_writes_block_textures_and_skips_other_entries() {
        let dir = temp_dir("jar-extract");
        let jar_path = dir.join("client.jar");
        let cache_dir = dir.join("cache");

        let glass = encode_png([255, 255, 255, 128]);
        let stone = encode_png([128, 128, 128, 255]);

        {
            let file = File::create(&jar_path).unwrap();
            let mut zip = ZipWriter::new(file);
            let options =
                SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
            zip.start_file("assets/minecraft/textures/block/glass.png", options)
                .unwrap();
            zip.write_all(&glass).unwrap();
            zip.start_file("assets/minecraft/textures/block/stone.png", options)
                .unwrap();
            zip.write_all(&stone).unwrap();
            zip.start_file("assets/minecraft/models/block/glass.json", options)
                .unwrap();
            zip.write_all(b"{}").unwrap();
            zip.start_file("assets/minecraft/textures/item/stick.png", options)
                .unwrap();
            zip.write_all(&stone).unwrap();
            zip.finish().unwrap();
        }

        fs::create_dir_all(&cache_dir).unwrap();
        fs::write(cache_dir.join("leftover.png"), b"stale").unwrap();

        let textures = extract_from_jar(&jar_path, &cache_dir).unwrap();
        assert_eq!(textures.len(), 2);
        assert!(textures["glass"].has_alpha());
        assert!(!textures["stone"].has_alpha());
        assert_eq!(textures["glass"].file_name(), "glass.png");
        assert!(cache_dir.join("glass.png").is_file());
        assert!(cache_dir.join("stone.png").is_file());
        assert!(!cache_dir.join("stick.png").exists());
        assert!(!cache_dir.join("glass.json").exists());
        assert!(!cache_dir.join("leftover.png").exists());

        fs::remove_dir_all(dir).ok();
    }
}
