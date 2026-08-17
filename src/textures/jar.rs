use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use crate::config::Config;

use super::image_format::ImageFormat;
use super::texture::{Texture, inspect_alpha};

/// Failure opening a jar, creating the cache, or writing the manifest.
#[derive(Debug)]
pub enum JarError {
    Io(io::Error),
    Zip(zip::result::ZipError),
    Json(serde_json::Error),
}

impl std::fmt::Display for JarError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => write!(f, "{err}"),
            Self::Zip(err) => write!(f, "{err}"),
            Self::Json(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for JarError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(err) => Some(err),
            Self::Zip(err) => Some(err),
            Self::Json(err) => Some(err),
        }
    }
}

impl From<io::Error> for JarError {
    fn from(err: io::Error) -> Self {
        Self::Io(err)
    }
}

impl From<zip::result::ZipError> for JarError {
    fn from(err: zip::result::ZipError) -> Self {
        Self::Zip(err)
    }
}

impl From<serde_json::Error> for JarError {
    fn from(err: serde_json::Error) -> Self {
        Self::Json(err)
    }
}

/// Extracts `assets/{namespace}/textures/block/*.{png,bmp,jpg}` into `cache_dir`.
///
/// Each matching entry is written under its original file name. The returned map is keyed
/// by file stem (e.g. `glass`).
pub fn extract_from_jar(
    jar_path: &Path,
    cache_dir: &Path,
) -> Result<HashMap<String, Texture>, JarError> {
    fs::create_dir_all(cache_dir)?;
    let file = File::open(jar_path)?;
    let mut archive = zip::ZipArchive::new(file)?;
    let mut textures = HashMap::new();

    for index in 0..archive.len() {
        let mut entry = match archive.by_index(index) {
            Ok(entry) => entry,
            Err(err) => {
                log::warn!("skipping jar entry {index}: {err}");
                continue;
            }
        };
        if !entry.is_file() {
            continue;
        }
        let Some((file_name, format)) = match_block_texture(entry.name()) else {
            continue;
        };

        let mut bytes = Vec::new();
        if let Err(err) = entry.read_to_end(&mut bytes) {
            log::warn!("failed to read {file_name} from jar: {err}");
            continue;
        }
        drop(entry);

        let dest = cache_dir.join(&file_name);
        if let Err(err) = fs::write(&dest, &bytes) {
            log::warn!("failed to write {}: {err}", dest.display());
            continue;
        }

        let has_alpha = inspect_alpha(&bytes, format);
        let key = file_stem(&file_name);
        textures.insert(key, Texture::new(file_name, format, has_alpha));
    }

    Ok(textures)
}

/// Matches `assets/{namespace}/textures/block/{file}.{png|bmp|jpg}`.
fn match_block_texture(entry_name: &str) -> Option<(String, ImageFormat)> {
    let name = entry_name.replace('\\', "/");
    let mut parts = name.split('/');
    let assets = parts.next()?;
    let namespace = parts.next()?;
    let textures = parts.next()?;
    let block = parts.next()?;
    let file = parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    if assets != "assets" || textures != "textures" || block != "block" {
        return None;
    }
    if namespace.is_empty() || file.is_empty() || file.contains("..") {
        return None;
    }
    let (_, ext) = file.rsplit_once('.')?;
    let format = ImageFormat::from_extension(ext)?;
    Some((file.to_owned(), format))
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
    cache_dir: &Path,
    textures: &HashMap<String, Texture>,
) -> Result<PathBuf, JarError> {
    fs::create_dir_all(cache_dir)?;
    let path = cache_dir.join(Config::TEXTURES_MANIFEST_FILE);
    let file = File::create(&path)?;
    serde_json::to_writer_pretty(file, textures)?;
    Ok(path)
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

        let textures = extract_from_jar(&jar_path, &cache_dir).unwrap();
        assert_eq!(textures.len(), 2);
        assert!(textures["glass"].has_alpha());
        assert!(!textures["stone"].has_alpha());
        assert_eq!(textures["glass"].file_name(), "glass.png");
        assert!(cache_dir.join("glass.png").is_file());
        assert!(cache_dir.join("stone.png").is_file());
        assert!(!cache_dir.join("stick.png").exists());
        assert!(!cache_dir.join("glass.json").exists());

        fs::remove_dir_all(dir).ok();
    }
}
