use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use crate::block_models::{BlockDefinition, BlockModel, asset_stem};

use super::JarError;

/// Maximum parent-chain length when computing `elements_from`.
const PARENT_CHAIN_LIMIT: usize = 32;

/// Extracts `assets/{namespace}/models/block/*.json` into `cache_dir`.
///
/// Each matching entry is written under its original file name. `elements_from` is filled
/// in a second pass after every model has been seen.
pub fn extract_from_jar(
    jar_path: &Path,
    cache_dir: &Path,
) -> Result<HashMap<String, BlockModel>, JarError> {
    fs::create_dir_all(cache_dir)?;
    let file = File::open(jar_path)?;
    let mut archive = zip::ZipArchive::new(file)?;
    let mut models = HashMap::new();
    let mut has_elements = HashMap::new();

    for index in 0..archive.len() {
        let Some(file_name) = archive.name_for_index(index).and_then(match_block_model) else {
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
            log::warn!("failed to read {file_name} from jar: {err}");
            continue;
        }
        drop(entry);

        if let Some((key, model, elements)) = ingest_model(cache_dir, file_name, &bytes) {
            has_elements.insert(key.clone(), elements);
            models.insert(key, model);
        }
    }

    fill_elements_from(&mut models, &has_elements);
    remove_stale_cache_files(cache_dir, &models);
    Ok(models)
}

/// Extracts models, writes the manifest, and returns the new library map.
pub(crate) fn import_into_cache(
    jar_path: &Path,
    cache_dir: &Path,
) -> Result<HashMap<String, BlockModel>, JarError> {
    let models = extract_from_jar(jar_path, cache_dir)?;
    write_manifest(
        &cache_dir.join(crate::constants::MODELS_MANIFEST_FILE),
        &models,
    )?;
    Ok(models)
}

/// Matches `assets/{namespace}/models/block/{file}.json`. Nested paths are rejected.
pub(crate) fn match_block_model(entry_name: &str) -> Option<String> {
    match_json_asset(entry_name, &["models", "block"])
}

/// Matches `assets/{namespace}/blockstates/{file}.json`. Nested paths are rejected.
pub(crate) fn match_blockstate(entry_name: &str) -> Option<String> {
    match_json_asset(entry_name, &["blockstates"])
}

/// Matches `assets/{namespace}/{folders...}/{file}.json` and returns the file name.
fn match_json_asset(entry_name: &str, folders: &[&str]) -> Option<String> {
    let name = if entry_name.contains('\\') {
        Cow::Owned(entry_name.replace('\\', "/"))
    } else {
        Cow::Borrowed(entry_name)
    };
    let mut parts = name.split('/');
    let assets = parts.next()?;
    let namespace = parts.next()?;
    for folder in folders {
        if parts.next()? != *folder {
            return None;
        }
    }
    let file = parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    if assets != "assets" || namespace.is_empty() || file.is_empty() || file.contains("..") {
        return None;
    }
    let (_, ext) = file.rsplit_once('.')?;
    if !ext.eq_ignore_ascii_case("json") {
        return None;
    }
    Some(file.to_owned())
}

/// Parses a blockstate JSON and returns `(block stem, definition)`.
pub(crate) fn ingest_blockstate(
    file_name: &str,
    bytes: &[u8],
) -> Option<(String, BlockDefinition)> {
    match BlockDefinition::from_json(bytes) {
        Ok(definition) => Some((file_stem(file_name), definition)),
        Err(err) => {
            log::warn!("failed to parse blockstate {file_name}: {err}");
            None
        }
    }
}

/// Parses a model JSON, writes it to the cache, and returns `(stem, record, has_elements)`.
pub(crate) fn ingest_model(
    cache_dir: &Path,
    file_name: String,
    bytes: &[u8],
) -> Option<(String, BlockModel, bool)> {
    let parsed: ModelHeader = match serde_json::from_slice(bytes) {
        Ok(parsed) => parsed,
        Err(err) => {
            log::warn!("failed to parse model {file_name}: {err}");
            return None;
        }
    };

    let dest = cache_dir.join(&file_name);
    if let Err(err) = fs::write(&dest, bytes) {
        log::warn!("failed to write {}: {err}", dest.display());
        return None;
    }

    let has_elements = parsed.has_elements();
    let parent = parsed
        .parent
        .as_deref()
        .map(asset_stem)
        .filter(|stem| !stem.is_empty());
    let textures = parsed
        .textures
        .into_iter()
        .map(|(slot, value)| (slot, asset_stem(&value)))
        .collect();
    let key = file_stem(&file_name);
    let model = BlockModel::new(file_name, parent, textures, None);
    Some((key, model, has_elements))
}

/// Walks each model's parent chain and records the nearest ancestor that declares `elements`.
pub(crate) fn fill_elements_from(
    models: &mut HashMap<String, BlockModel>,
    has_elements: &HashMap<String, bool>,
) {
    let keys: Vec<String> = models.keys().cloned().collect();
    for key in keys {
        let source = nearest_with_elements(&key, models, has_elements);
        if let Some(model) = models.get_mut(&key) {
            model.set_elements_from(source);
        }
    }
}

fn nearest_with_elements(
    start: &str,
    models: &HashMap<String, BlockModel>,
    has_elements: &HashMap<String, bool>,
) -> Option<String> {
    let mut current = start;
    let mut seen = HashSet::new();
    for _ in 0..PARENT_CHAIN_LIMIT {
        if !seen.insert(current.to_owned()) {
            return None;
        }
        if has_elements.get(current).copied().unwrap_or(false) {
            return Some(current.to_owned());
        }
        current = models.get(current)?.parent()?;
    }
    None
}

pub(crate) fn remove_stale_cache_files(cache_dir: &Path, models: &HashMap<String, BlockModel>) {
    let keep: HashSet<&str> = models.values().map(BlockModel::file_name).collect();
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
        if name == crate::constants::MODELS_MANIFEST_FILE
            || name == crate::constants::BLOCKSTATES_MANIFEST_FILE
            || keep.contains(name)
        {
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
    models: &HashMap<String, BlockModel>,
) -> Result<PathBuf, JarError> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }
    let mut file = File::create(path)?;
    let ordered: BTreeMap<_, _> = models.iter().collect();
    serde_json::to_writer_pretty(&mut file, &ordered)?;
    file.flush()?;
    file.sync_all()?;
    Ok(path.to_path_buf())
}

/// Writes every blockstate definition into one manifest, keyed by block stem.
pub(crate) fn write_blockstates_manifest(
    path: &Path,
    definitions: &HashMap<String, BlockDefinition>,
) -> Result<(), JarError> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }
    let mut file = File::create(path)?;
    let ordered: BTreeMap<_, _> = definitions.iter().collect();
    serde_json::to_writer(&mut file, &ordered)?;
    file.flush()?;
    file.sync_all()?;
    Ok(())
}

/// Lightweight header used at extraction time so malformed `elements` do not drop the model.
#[derive(serde::Deserialize)]
struct ModelHeader {
    #[serde(default)]
    parent: Option<String>,
    #[serde(default)]
    textures: BTreeMap<String, String>,
    #[serde(default)]
    elements: Option<serde_json::Value>,
}

impl ModelHeader {
    fn has_elements(&self) -> bool {
        match &self.elements {
            Some(serde_json::Value::Array(items)) => !items.is_empty(),
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
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

    #[test]
    fn match_block_model_accepts_client_layout() {
        assert_eq!(
            match_block_model("assets/minecraft/models/block/glass.json").as_deref(),
            Some("glass.json")
        );
        assert!(match_block_model("assets/minecraft/textures/block/glass.png").is_none());
        assert!(match_block_model("assets/minecraft/models/block/sub/glass.json").is_none());
        assert!(match_block_model("assets/minecraft/models/item/stick.json").is_none());
        assert!(match_block_model("assets/minecraft/models/block/glass.png").is_none());
        assert_eq!(
            match_block_model("assets\\minecraft\\models\\block\\glass.json").as_deref(),
            Some("glass.json")
        );
    }

    #[test]
    fn extract_writes_block_models_and_computes_elements_from() {
        let dir = temp_dir("model-jar-extract");
        let jar_path = dir.join("client.jar");
        let cache_dir = dir.join("cache");

        let cube = r##"{
            "elements": [{
                "from": [0, 0, 0],
                "to": [16, 16, 16],
                "faces": { "up": { "texture": "#up" } }
            }]
        }"##;
        let cube_all = r##"{
            "parent": "minecraft:block/cube",
            "textures": {
                "down": "#all", "east": "#all", "north": "#all", "particle": "#all",
                "south": "#all", "up": "#all", "west": "#all"
            }
        }"##;
        let quartz = r#"{
            "parent": "minecraft:block/cube_column",
            "textures": {
                "end": "minecraft:block/quartz_pillar_top",
                "side": "minecraft:block/quartz_pillar"
            }
        }"#;
        let cube_column = r##"{
            "parent": "minecraft:block/cube",
            "textures": { "down": "#end", "up": "#end", "side": "#side" }
        }"##;
        let chest = r#"{
            "textures": { "particle": "minecraft:block/oak_planks" }
        }"#;

        {
            let file = File::create(&jar_path).unwrap();
            let mut zip = ZipWriter::new(file);
            let options =
                SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
            zip.start_file("assets/minecraft/models/block/cube.json", options)
                .unwrap();
            zip.write_all(cube.as_bytes()).unwrap();
            zip.start_file("assets/minecraft/models/block/cube_all.json", options)
                .unwrap();
            zip.write_all(cube_all.as_bytes()).unwrap();
            zip.start_file("assets/minecraft/models/block/cube_column.json", options)
                .unwrap();
            zip.write_all(cube_column.as_bytes()).unwrap();
            zip.start_file("assets/minecraft/models/block/quartz_pillar.json", options)
                .unwrap();
            zip.write_all(quartz.as_bytes()).unwrap();
            zip.start_file("assets/minecraft/models/block/chest.json", options)
                .unwrap();
            zip.write_all(chest.as_bytes()).unwrap();
            zip.start_file("assets/minecraft/textures/block/stone.png", options)
                .unwrap();
            zip.write_all(&[0]).unwrap();
            zip.start_file("assets/minecraft/models/block/nested/skip.json", options)
                .unwrap();
            zip.write_all(b"{}").unwrap();
            zip.finish().unwrap();
        }

        fs::create_dir_all(&cache_dir).unwrap();
        fs::write(cache_dir.join("leftover.json"), b"stale").unwrap();

        let models = extract_from_jar(&jar_path, &cache_dir).unwrap();
        assert_eq!(models.len(), 5);
        assert_eq!(models["cube"].elements_from(), Some("cube"));
        assert_eq!(models["cube_all"].parent(), Some("cube"));
        assert_eq!(models["cube_all"].elements_from(), Some("cube"));
        assert_eq!(models["cube_all"].textures()["down"], "#all");
        assert_eq!(models["quartz_pillar"].parent(), Some("cube_column"));
        assert_eq!(
            models["quartz_pillar"].textures()["end"],
            "quartz_pillar_top"
        );
        assert_eq!(models["quartz_pillar"].elements_from(), Some("cube"));
        assert_eq!(models["chest"].parent(), None);
        assert_eq!(models["chest"].elements_from(), None);
        assert_eq!(models["chest"].textures()["particle"], "oak_planks");
        assert!(cache_dir.join("cube_all.json").is_file());
        assert!(!cache_dir.join("stone.png").exists());
        assert!(!cache_dir.join("skip.json").exists());
        assert!(!cache_dir.join("leftover.json").exists());

        fs::remove_dir_all(dir).ok();
    }
}
