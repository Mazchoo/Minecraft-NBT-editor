//! Importing textures, block models and blockstates from a Minecraft client jar.

pub(crate) mod models;
pub(crate) mod textures;

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::Path;

use crate::block_models::{BlockDefinition, BlockModel};
use crate::textures::Texture;

use self::models::{
    fill_elements_from, ingest_blockstate, ingest_model, match_block_model, match_blockstate,
    remove_stale_cache_files, write_blockstates_manifest, write_manifest as write_models_manifest,
};
use self::textures::{
    ingest_chest_atlas, ingest_texture, match_block_texture, match_chest_atlas,
    remove_stale_cache_files as remove_stale_textures, write_manifest as write_textures_manifest,
};

/// Result of a single pass over a Minecraft client jar.
pub struct ImportedAssets {
    pub textures: HashMap<String, Texture>,
    pub models: HashMap<String, BlockModel>,
    pub definitions: HashMap<String, BlockDefinition>,
}

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

/// Opens `jar_path` and yields each file entry's path and bytes.
pub(crate) fn for_each_jar_file(
    jar_path: &Path,
    mut visit: impl FnMut(&str, Vec<u8>),
) -> Result<(), JarError> {
    let file = File::open(jar_path)?;
    let mut archive = zip::ZipArchive::new(file)?;
    for index in 0..archive.len() {
        let Some(name) = archive.name_for_index(index).map(str::to_owned) else {
            continue;
        };
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
        let mut bytes = Vec::with_capacity(entry.size() as usize);
        if let Err(err) = entry.read_to_end(&mut bytes) {
            log::warn!("failed to read {name} from jar: {err}");
            continue;
        }
        drop(entry);
        visit(&name, bytes);
    }
    Ok(())
}

/// Opens the jar once and offers every entry to the texture, model and blockstate matchers.
pub fn import_from_jar(
    jar_path: &Path,
    texture_cache_dir: &Path,
    model_cache_dir: &Path,
    textures_manifest: &Path,
    models_manifest: &Path,
    blockstates_manifest: &Path,
) -> Result<ImportedAssets, JarError> {
    fs::create_dir_all(texture_cache_dir)?;
    fs::create_dir_all(model_cache_dir)?;

    let mut textures = HashMap::new();
    let mut models = HashMap::new();
    let mut has_elements = HashMap::new();
    let mut definitions = HashMap::new();

    for_each_jar_file(jar_path, |name, bytes| {
        if let Some((file_name, format)) = match_block_texture(name)
            && let Some((key, texture)) =
                ingest_texture(texture_cache_dir, file_name, format, &bytes)
        {
            textures.insert(key, texture);
        }
        if let Some(stem) = match_chest_atlas(name) {
            textures.extend(ingest_chest_atlas(texture_cache_dir, &stem, &bytes));
        }
        if let Some(file_name) = match_block_model(name)
            && let Some((key, model, elements)) = ingest_model(model_cache_dir, file_name, &bytes)
        {
            has_elements.insert(key.clone(), elements);
            models.insert(key, model);
        }
        if let Some(file_name) = match_blockstate(name)
            && let Some((key, definition)) = ingest_blockstate(&file_name, &bytes)
        {
            definitions.insert(key, definition);
        }
    })?;

    // Textures are complete after the walk. Write that manifest before model
    // parent-chain work and stale-file scans, which can take a long time.
    write_textures_manifest(textures_manifest, &textures)?;
    remove_stale_textures(texture_cache_dir, &textures);

    write_blockstates_manifest(blockstates_manifest, &definitions)?;

    fill_elements_from(&mut models, &has_elements);
    remove_stale_cache_files(model_cache_dir, &models);
    write_models_manifest(models_manifest, &models)?;

    Ok(ImportedAssets {
        textures,
        models,
        definitions,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};
    use std::io::{Cursor, Write};
    use std::path::PathBuf;
    use zip::ZipWriter;
    use zip::write::SimpleFileOptions;

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

    fn encode_png(color: [u8; 4]) -> Vec<u8> {
        let img = RgbaImage::from_pixel(2, 2, Rgba(color));
        let mut bytes = Vec::new();
        img.write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)
            .unwrap();
        bytes
    }

    #[test]
    fn import_from_jar_fills_both_caches_in_one_pass() {
        let dir = temp_dir("jar-import-both");
        let jar_path = dir.join("client.jar");
        let texture_cache = dir.join("textures");
        let model_cache = dir.join("models");
        let textures_manifest = dir.join(crate::constants::TEXTURES_MANIFEST_FILE);
        let models_manifest = dir.join(crate::constants::MODELS_MANIFEST_FILE);
        let blockstates_manifest = dir.join(crate::constants::BLOCKSTATES_MANIFEST_FILE);
        let png = encode_png([10, 20, 30, 255]);

        {
            let file = fs::File::create(&jar_path).unwrap();
            let mut zip = ZipWriter::new(file);
            let options =
                SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
            zip.start_file("assets/minecraft/textures/block/stone.png", options)
                .unwrap();
            zip.write_all(&png).unwrap();
            zip.start_file("assets/minecraft/models/block/stone.json", options)
                .unwrap();
            zip.write_all(
                br#"{"parent":"minecraft:block/cube_all","textures":{"all":"minecraft:block/stone"}}"#,
            )
            .unwrap();
            zip.start_file("assets/minecraft/blockstates/stone.json", options)
                .unwrap();
            zip.write_all(br#"{"variants":{"":{"model":"minecraft:block/stone"}}}"#)
                .unwrap();
            zip.start_file("assets/minecraft/models/block/cube.json", options)
                .unwrap();
            zip.write_all(
                br##"{"elements":[{"from":[0,0,0],"to":[16,16,16],"faces":{"up":{"texture":"#up"}}}]}"##,
            )
            .unwrap();
            zip.finish().unwrap();
        }

        let imported = import_from_jar(
            &jar_path,
            &texture_cache,
            &model_cache,
            &textures_manifest,
            &models_manifest,
            &blockstates_manifest,
        )
        .unwrap();
        assert_eq!(imported.textures.len(), 1);
        assert!(imported.textures.contains_key("stone"));
        assert_eq!(imported.models.len(), 2);
        assert_eq!(imported.models["stone"].parent(), Some("cube_all"));
        assert_eq!(imported.models["cube"].elements_from(), Some("cube"));
        assert!(texture_cache.join("stone.png").is_file());
        assert!(model_cache.join("stone.json").is_file());
        assert!(textures_manifest.is_file());
        assert!(models_manifest.is_file());
        assert_eq!(imported.definitions.len(), 1);
        assert!(imported.definitions.contains_key("stone"));
        assert!(blockstates_manifest.is_file());

        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn import_from_jar_replaces_previous_cache() {
        let dir = temp_dir("jar-import-replace");
        let jar_path = dir.join("client.jar");
        let texture_cache = dir.join("textures");
        let model_cache = dir.join("models");
        let textures_manifest = dir.join(crate::constants::TEXTURES_MANIFEST_FILE);
        let models_manifest = dir.join(crate::constants::MODELS_MANIFEST_FILE);
        let blockstates_manifest = dir.join(crate::constants::BLOCKSTATES_MANIFEST_FILE);
        let png = encode_png([10, 20, 30, 255]);

        fs::create_dir_all(&texture_cache).unwrap();
        fs::create_dir_all(&model_cache).unwrap();
        fs::write(texture_cache.join("leftover.png"), b"stale").unwrap();
        fs::write(model_cache.join("leftover.json"), b"stale").unwrap();
        fs::write(
            &textures_manifest,
            r#"{"leftover":{"file_name":"leftover.png","format":"png","has_alpha":false}}"#,
        )
        .unwrap();
        fs::write(
            &models_manifest,
            r#"{"leftover":{"file_name":"leftover.json","parent":null,"textures":{},"elements_from":null}}"#,
        )
        .unwrap();

        {
            let file = fs::File::create(&jar_path).unwrap();
            let mut zip = ZipWriter::new(file);
            let options =
                SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
            zip.start_file("assets/minecraft/textures/block/stone.png", options)
                .unwrap();
            zip.write_all(&png).unwrap();
            zip.start_file("assets/minecraft/models/block/stone.json", options)
                .unwrap();
            zip.write_all(
                br#"{"parent":"minecraft:block/cube_all","textures":{"all":"minecraft:block/stone"}}"#,
            )
            .unwrap();
            zip.finish().unwrap();
        }

        let imported = import_from_jar(
            &jar_path,
            &texture_cache,
            &model_cache,
            &textures_manifest,
            &models_manifest,
            &blockstates_manifest,
        )
        .unwrap();
        assert!(!imported.textures.contains_key("leftover"));
        assert!(!imported.models.contains_key("leftover"));
        assert!(!texture_cache.join("leftover.png").exists());
        assert!(!model_cache.join("leftover.json").exists());
        assert!(texture_cache.join("stone.png").is_file());
        assert!(model_cache.join("stone.json").is_file());

        let textures_json: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&textures_manifest).unwrap()).unwrap();
        assert!(textures_json.get("leftover").is_none());
        assert!(textures_json.get("stone").is_some());

        let models_json: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&models_manifest).unwrap()).unwrap();
        assert!(models_json.get("leftover").is_none());
        assert!(models_json.get("stone").is_some());

        fs::remove_dir_all(dir).ok();
    }
}
