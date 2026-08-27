use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use crate::config::Config;
use crate::textures::JarError;

use super::block_model::BlockModel;
use super::jar::import_into_cache;
use super::model_json::{ModelElement, ModelFace, ModelFile, ModelRotation, asset_stem};

/// Guards against parent cycles in a modified jar.
const PARENT_CHAIN_LIMIT: usize = 32;

/// In-memory dictionary of cached block models, backed by the cache manifest.
pub struct ModelLibrary {
    cache_dir: PathBuf,
    manifest_path: PathBuf,
    models: HashMap<String, BlockModel>,
    /// Sorted cache keys, rebuilt whenever the dictionary is replaced.
    names: Vec<String>,
    /// Last observed mtime of [`Self::manifest_path`]. `None` if the file is missing.
    manifest_mtime: Option<SystemTime>,
    resolved: RefCell<HashMap<ResolveKey, Arc<ResolvedBlock>>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ResolveKey {
    name: String,
    properties: BTreeMap<String, String>,
}

/// One palette entry after parent-chain merge and texture binding.
#[derive(Debug, Clone)]
#[allow(dead_code)] // consumed by `Block::from_resolved` once the viewport draws blocks
pub struct ResolvedBlock {
    pub properties: BTreeMap<String, String>,
    /// Immediate parent model stem, used to classify [`crate::models::BlockType`].
    pub parent_stem: Option<String>,
    /// The chain's `particle` texture, if the merged dictionary has one.
    pub particle_texture: Option<String>,
    pub parts: Vec<ResolvedPart>,
}

/// Geometry and orientation for one model that makes up a block.
#[derive(Debug, Clone)]
#[allow(dead_code)] // consumed by `Block::from_resolved` once the viewport draws blocks
pub struct ResolvedPart {
    /// Model stem this part resolved to.
    pub model: String,
    pub elements: Vec<ResolvedElement>,
    /// True when the chain has no geometry (block-entity placeholder).
    pub block_entity: bool,
}

/// An element whose face textures are already `TextureLibrary` keys (or empty).
#[derive(Debug, Clone)]
#[allow(dead_code)] // consumed by `Block::from_resolved` once the viewport draws blocks
pub struct ResolvedElement {
    pub from: [f32; 3],
    pub to: [f32; 3],
    pub rotation: Option<ModelRotation>,
    pub faces: BTreeMap<String, ResolvedFace>,
    pub shade: bool,
}

#[derive(Debug, Clone)]
#[allow(dead_code)] // consumed by `Block::from_resolved` once the viewport draws blocks
pub struct ResolvedFace {
    /// `TextureLibrary` key, or `None` if the slot did not resolve.
    pub texture: Option<String>,
    pub uv: Option<[f32; 4]>,
    pub rotation: Option<u32>,
    pub cullface: Option<String>,
    pub tintindex: Option<i32>,
}

impl ModelLibrary {
    /// Loads the cache manifest if present. Missing or invalid manifests start empty.
    pub fn load() -> Self {
        Self::from_paths(
            Config::model_cache_dir().to_path_buf(),
            Config::models_manifest_path(),
        )
    }

    fn from_paths(cache_dir: PathBuf, manifest_path: PathBuf) -> Self {
        let models = load_manifest(&manifest_path);
        let names = sorted_names(&models);
        let manifest_mtime = file_mtime(&manifest_path);
        Self {
            cache_dir,
            manifest_path,
            models,
            names,
            manifest_mtime,
            resolved: RefCell::new(HashMap::new()),
        }
    }

    #[cfg(test)]
    pub(crate) fn from_cache_dir(cache_dir: impl Into<PathBuf>) -> Self {
        let cache_dir = cache_dir.into();
        let manifest_path = cache_dir.join(Config::MODELS_MANIFEST_FILE);
        Self::from_paths(cache_dir, manifest_path)
    }

    /// Extracts block models from a Minecraft client jar into the cache and rewrites the manifest.
    #[allow(dead_code)]
    pub fn import_jar(&mut self, jar_path: &Path) -> Result<usize, JarError> {
        let models = import_into_cache(jar_path, &self.cache_dir)?;
        let count = models.len();
        self.apply_import(models);
        Ok(count)
    }

    pub(crate) fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    pub(crate) fn apply_import(&mut self, models: HashMap<String, BlockModel>) {
        self.replace_models(models);
        self.manifest_mtime = file_mtime(&self.manifest_path);
    }

    /// Reloads the dictionary when `_models.cache.json` appears, disappears, or is rewritten.
    ///
    /// A parse or I/O error leaves the current dictionary in place so a partial write
    /// (for example during jar import) does not empty the sidebar.
    pub fn reload_if_changed(&mut self) -> bool {
        let mtime = file_mtime(&self.manifest_path);
        if mtime == self.manifest_mtime {
            return false;
        }
        match try_load_manifest(&self.manifest_path) {
            Ok(models) => {
                self.manifest_mtime = mtime;
                self.replace_models(models);
                true
            }
            Err(ManifestError::NotFound) => {
                self.manifest_mtime = None;
                self.replace_models(HashMap::new());
                true
            }
            Err(ManifestError::Failed) => false,
        }
    }

    /// Sorted model stems from the cache, used as the block picker list.
    pub fn names(&self) -> &[String] {
        &self.names
    }

    pub fn get_record(&self, key: &str) -> Option<&BlockModel> {
        self.models.get(key)
    }

    pub fn len(&self) -> usize {
        self.models.len()
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.models.is_empty()
    }

    fn replace_models(&mut self, models: HashMap<String, BlockModel>) {
        self.models = models;
        self.names = sorted_names(&self.models);
        self.resolved.borrow_mut().clear();
    }

    /// Resolves one NBT palette entry into geometry. Always returns a usable block;
    /// unknown names and broken chains fall back to a full cube with a missing texture.
    #[allow(dead_code)] // consumed by `Block::from_resolved` once the viewport draws blocks
    pub fn resolve(&self, name: &str, properties: &BTreeMap<String, String>) -> Arc<ResolvedBlock> {
        let key = ResolveKey {
            name: name.to_owned(),
            properties: properties.clone(),
        };
        if let Some(cached) = self.resolved.borrow().get(&key) {
            return Arc::clone(cached);
        }
        let resolved = Arc::new(self.resolve_uncached(name, properties));
        self.resolved
            .borrow_mut()
            .insert(key, Arc::clone(&resolved));
        resolved
    }

    fn resolve_uncached(&self, name: &str, properties: &BTreeMap<String, String>) -> ResolvedBlock {
        let stem = asset_stem(name);
        let Some(record) = self.models.get(&stem) else {
            log::warn!("model `{stem}` is not in the library; using fallback cube");
            return fallback_cube(properties.clone());
        };

        let Some(merged) = self.merge_textures(&stem) else {
            log::warn!("model `{stem}` parent chain exceeded depth limit; using fallback cube");
            return fallback_cube(properties.clone());
        };

        let particle_texture = merged.get("particle").cloned();
        let parent_stem = record.parent().map(str::to_owned);

        let Some(elements_from) = record.elements_from() else {
            return ResolvedBlock {
                properties: properties.clone(),
                parent_stem,
                particle_texture,
                parts: vec![ResolvedPart {
                    model: stem,
                    elements: Vec::new(),
                    block_entity: true,
                }],
            };
        };

        match self.read_and_bind_elements(elements_from, &merged) {
            Ok(elements) => ResolvedBlock {
                properties: properties.clone(),
                parent_stem,
                particle_texture,
                parts: vec![ResolvedPart {
                    model: stem,
                    elements,
                    block_entity: false,
                }],
            },
            Err(err) => {
                log::warn!(
                    "model `{stem}` geometry from `{elements_from}` could not be read: {err}; using fallback cube"
                );
                fallback_cube(properties.clone())
            }
        }
    }

    /// Child-first texture merge. Returns `None` when the parent chain exceeds the depth cap.
    fn merge_textures(&self, stem: &str) -> Option<BTreeMap<String, String>> {
        let mut chain = Vec::new();
        let mut current = Some(stem);
        let mut seen = HashSet::new();
        for _ in 0..PARENT_CHAIN_LIMIT {
            let Some(key) = current else {
                return Some(collapse_slots(merge_chain(&chain)));
            };
            if !seen.insert(key.to_owned()) {
                log::warn!("model `{stem}` has a parent cycle");
                return None;
            }
            let Some(model) = self.models.get(key) else {
                return Some(collapse_slots(merge_chain(&chain)));
            };
            chain.push(model);
            current = model.parent();
        }
        if current.is_some() {
            None
        } else {
            Some(collapse_slots(merge_chain(&chain)))
        }
    }

    fn read_and_bind_elements(
        &self,
        elements_from: &str,
        merged: &BTreeMap<String, String>,
    ) -> Result<Vec<ResolvedElement>, String> {
        let record = self
            .models
            .get(elements_from)
            .ok_or_else(|| format!("`{elements_from}` is not in the library"))?;
        let path = self.cache_dir.join(record.file_name());
        let text = fs::read_to_string(&path).map_err(|err| err.to_string())?;
        let file: ModelFile = serde_json::from_str(&text).map_err(|err| err.to_string())?;
        Ok(file
            .elements
            .into_iter()
            .map(|element| bind_element(element, merged))
            .collect())
    }
}

fn merge_chain(chain: &[&BlockModel]) -> BTreeMap<String, String> {
    let mut merged = BTreeMap::new();
    for model in chain {
        for (slot, value) in model.textures() {
            merged.entry(slot.clone()).or_insert_with(|| value.clone());
        }
    }
    merged
}

fn collapse_slots(merged: BTreeMap<String, String>) -> BTreeMap<String, String> {
    let keys: Vec<String> = merged.keys().cloned().collect();
    let mut resolved = BTreeMap::new();
    for key in keys {
        if let Some(value) = follow_slot(&merged, &key) {
            resolved.insert(key, value);
        }
    }
    resolved
}

fn follow_slot(merged: &BTreeMap<String, String>, slot: &str) -> Option<String> {
    let mut current = merged.get(slot)?.as_str();
    let mut seen = HashSet::new();
    seen.insert(slot.to_owned());
    loop {
        if let Some(indirection) = current.strip_prefix('#') {
            if !seen.insert(indirection.to_owned()) {
                return None;
            }
            current = merged.get(indirection)?;
        } else {
            return Some(current.to_owned());
        }
    }
}

fn bind_element(element: ModelElement, merged: &BTreeMap<String, String>) -> ResolvedElement {
    let faces = element
        .faces
        .into_iter()
        .map(|(name, face)| (name, bind_face(face, merged)))
        .collect();
    ResolvedElement {
        from: element.from,
        to: element.to,
        rotation: element.rotation,
        faces,
        shade: element.shade,
    }
}

fn bind_face(face: ModelFace, merged: &BTreeMap<String, String>) -> ResolvedFace {
    let texture = resolve_texture_ref(&face.texture, merged);
    ResolvedFace {
        texture,
        uv: face.uv,
        rotation: face.rotation,
        cullface: face.cullface,
        tintindex: face.tintindex,
    }
}

fn resolve_texture_ref(value: &str, merged: &BTreeMap<String, String>) -> Option<String> {
    if let Some(slot) = value.strip_prefix('#') {
        follow_slot(merged, slot)
    } else if value.is_empty() {
        None
    } else {
        Some(asset_stem(value))
    }
}

fn fallback_cube(properties: BTreeMap<String, String>) -> ResolvedBlock {
    let face = ResolvedFace {
        texture: None,
        uv: None,
        rotation: None,
        cullface: None,
        tintindex: None,
    };
    let mut faces = BTreeMap::new();
    for name in ["down", "up", "north", "south", "west", "east"] {
        faces.insert(name.to_owned(), face.clone());
    }
    ResolvedBlock {
        properties,
        parent_stem: None,
        particle_texture: None,
        parts: vec![ResolvedPart {
            model: "missing".into(),
            elements: vec![ResolvedElement {
                from: [0.0, 0.0, 0.0],
                to: [16.0, 16.0, 16.0],
                rotation: None,
                faces,
                shade: true,
            }],
            block_entity: false,
        }],
    }
}

fn sorted_names(models: &HashMap<String, BlockModel>) -> Vec<String> {
    let mut names: Vec<String> = models.keys().cloned().collect();
    names.sort();
    names
}

fn file_mtime(path: &Path) -> Option<SystemTime> {
    fs::metadata(path).ok()?.modified().ok()
}

enum ManifestError {
    NotFound,
    Failed,
}

fn try_load_manifest(path: &Path) -> Result<HashMap<String, BlockModel>, ManifestError> {
    match fs::read_to_string(path) {
        Ok(text) => match serde_json::from_str(&text) {
            Ok(map) => Ok(map),
            Err(err) => {
                log::error!("failed to parse model manifest {}: {err}", path.display());
                Err(ManifestError::Failed)
            }
        },
        Err(err) if err.kind() == io::ErrorKind::NotFound => Err(ManifestError::NotFound),
        Err(err) => {
            log::error!("failed to read model manifest {}: {err}", path.display());
            Err(ManifestError::Failed)
        }
    }
}

fn load_manifest(path: &Path) -> HashMap<String, BlockModel> {
    try_load_manifest(path).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

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

    fn write_model_cache(dir: &Path) {
        fs::write(
            dir.join("cube.json"),
            r##"{
                "elements": [{
                    "from": [0, 0, 0],
                    "to": [16, 16, 16],
                    "faces": {
                        "down":  { "texture": "#down", "cullface": "down" },
                        "up":    { "texture": "#up", "cullface": "up" },
                        "north": { "texture": "#north" },
                        "south": { "texture": "#south" },
                        "west":  { "texture": "#west" },
                        "east":  { "texture": "#east" }
                    }
                }]
            }"##,
        )
        .unwrap();
        fs::write(
            dir.join(Config::MODELS_MANIFEST_FILE),
            r##"{
                "cube": {
                    "file_name": "cube.json",
                    "parent": null,
                    "textures": {},
                    "elements_from": "cube"
                },
                "cube_all": {
                    "file_name": "cube_all.json",
                    "parent": "cube",
                    "textures": {
                        "down": "#all", "east": "#all", "north": "#all", "particle": "#all",
                        "south": "#all", "up": "#all", "west": "#all"
                    },
                    "elements_from": "cube"
                },
                "stone": {
                    "file_name": "stone.json",
                    "parent": "cube_all",
                    "textures": { "all": "stone" },
                    "elements_from": "cube"
                },
                "chest": {
                    "file_name": "chest.json",
                    "parent": null,
                    "textures": { "particle": "oak_planks" },
                    "elements_from": null
                }
            }"##,
        )
        .unwrap();
    }

    #[test]
    fn load_reads_manifest() {
        let dir = temp_dir("model-library-load");
        write_model_cache(&dir);
        let library = ModelLibrary::from_cache_dir(&dir);
        assert_eq!(library.len(), 4);
        assert_eq!(
            library.names(),
            &[
                "chest".to_owned(),
                "cube".to_owned(),
                "cube_all".to_owned(),
                "stone".to_owned()
            ]
        );
        assert_eq!(
            library.get_record("stone").unwrap().parent(),
            Some("cube_all")
        );
        assert_eq!(
            library.get_record("cube_all").unwrap().elements_from(),
            Some("cube")
        );
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn reload_if_changed_picks_up_a_rewritten_manifest() {
        let dir = temp_dir("model-library-reload");
        write_model_cache(&dir);
        let mut library = ModelLibrary::from_cache_dir(&dir);
        assert!(!library.reload_if_changed());
        assert!(library.names().contains(&"stone".to_owned()));

        fs::write(
            dir.join(Config::MODELS_MANIFEST_FILE),
            r##"{
                "dirt": {
                    "file_name": "dirt.json",
                    "parent": "cube_all",
                    "textures": { "all": "dirt" },
                    "elements_from": "cube"
                }
            }"##,
        )
        .unwrap();
        let later = SystemTime::now() + std::time::Duration::from_secs(2);
        fs::File::options()
            .write(true)
            .open(dir.join(Config::MODELS_MANIFEST_FILE))
            .unwrap()
            .set_modified(later)
            .unwrap();

        assert!(library.reload_if_changed());
        assert_eq!(library.names(), &["dirt".to_owned()]);
        assert!(!library.reload_if_changed());

        fs::remove_file(dir.join(Config::MODELS_MANIFEST_FILE)).unwrap();
        assert!(library.reload_if_changed());
        assert!(library.names().is_empty());

        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn resolve_merges_texture_slots_and_binds_faces() {
        let dir = temp_dir("model-library-resolve");
        write_model_cache(&dir);
        let library = ModelLibrary::from_cache_dir(&dir);
        let empty = BTreeMap::new();
        let resolved = library.resolve("minecraft:stone", &empty);
        assert_eq!(resolved.parts.len(), 1);
        assert!(!resolved.parts[0].block_entity);
        let faces = &resolved.parts[0].elements[0].faces;
        assert_eq!(faces["up"].texture.as_deref(), Some("stone"));
        assert_eq!(faces["down"].texture.as_deref(), Some("stone"));
        assert_eq!(faces["north"].texture.as_deref(), Some("stone"));
        assert_eq!(resolved.parent_stem.as_deref(), Some("cube_all"));
        assert_eq!(resolved.particle_texture.as_deref(), Some("stone"));
        assert!(Arc::ptr_eq(
            &resolved,
            &library.resolve("minecraft:stone", &empty)
        ));
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn resolve_marks_block_entity_when_no_geometry() {
        let dir = temp_dir("model-library-entity");
        write_model_cache(&dir);
        let library = ModelLibrary::from_cache_dir(&dir);
        let resolved = library.resolve("minecraft:chest", &BTreeMap::new());
        assert_eq!(resolved.parts.len(), 1);
        assert!(resolved.parts[0].block_entity);
        assert!(resolved.parts[0].elements.is_empty());
        assert_eq!(resolved.particle_texture.as_deref(), Some("oak_planks"));
        assert_eq!(resolved.parent_stem, None);
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn resolve_unknown_name_returns_fallback_cube() {
        let dir = temp_dir("model-library-missing");
        let library = ModelLibrary::from_cache_dir(&dir);
        let resolved = library.resolve("minecraft:nope", &BTreeMap::new());
        assert_eq!(resolved.parts[0].model, "missing");
        assert!(!resolved.parts[0].block_entity);
        assert_eq!(resolved.parts[0].elements[0].from, [0.0, 0.0, 0.0]);
        assert_eq!(resolved.parts[0].elements[0].to, [16.0, 16.0, 16.0]);
        assert!(
            resolved.parts[0].elements[0]
                .faces
                .values()
                .all(|face| face.texture.is_none())
        );
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn import_jar_writes_manifest() {
        use std::io::Write;
        use zip::ZipWriter;
        use zip::write::SimpleFileOptions;

        let dir = temp_dir("model-library-import");
        let jar_path = dir.join("client.jar");
        let cache_dir = dir.join("cache");
        fs::create_dir_all(&cache_dir).unwrap();

        {
            let file = fs::File::create(&jar_path).unwrap();
            let mut zip = ZipWriter::new(file);
            let options =
                SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
            zip.start_file("assets/minecraft/models/block/stone.json", options)
                .unwrap();
            zip.write_all(br#"{"parent":"minecraft:block/cube_all","textures":{"all":"minecraft:block/stone"}}"#)
                .unwrap();
            zip.finish().unwrap();
        }

        let mut library = ModelLibrary::from_cache_dir(&cache_dir);
        assert_eq!(library.import_jar(&jar_path).unwrap(), 1);
        assert_eq!(library.len(), 1);
        assert!(cache_dir.join(Config::MODELS_MANIFEST_FILE).is_file());
        assert_eq!(
            library.get_record("stone").unwrap().parent(),
            Some("cube_all")
        );
        assert_eq!(
            library.get_record("stone").unwrap().textures()["all"],
            "stone"
        );

        fs::remove_dir_all(dir).ok();
    }
}
