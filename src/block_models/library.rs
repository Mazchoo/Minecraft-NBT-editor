use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use crate::config::Config;
use crate::constants::{AXIS_PROPERTY, FACING_PROPERTY};
use crate::textures::JarError;

use super::block_definition::{AppliedModel, BlockDefinition};
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
    definitions_path: PathBuf,
    /// Blockstate definitions by block stem.
    definitions: HashMap<String, BlockDefinition>,
    /// Sorted block stems for the picker, rebuilt with the dictionaries.
    block_names: Vec<String>,
    /// Last observed mtime of [`Self::definitions_path`].
    definitions_mtime: Option<SystemTime>,
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
    /// The chain block's texture, set only for hanging signs, which draw chains
    /// their own model does not describe.
    pub chain_texture: Option<String>,
    /// True when the block's definition does not orient it by `axis` or
    /// `facing`, so those properties turn it by the fixed table instead.
    pub property_rotation: bool,
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
    /// Rotation the blockstate gives this model, in degrees.
    pub x: u16,
    pub y: u16,
    pub uvlock: bool,
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
            Config::blockstates_manifest_path(),
        )
    }

    fn from_paths(cache_dir: PathBuf, manifest_path: PathBuf, definitions_path: PathBuf) -> Self {
        let models = load_manifest(&manifest_path);
        let names = sorted_names(&models);
        let manifest_mtime = file_mtime(&manifest_path);
        let definitions = try_load_definitions(&definitions_path).unwrap_or_default();
        let definitions_mtime = file_mtime(&definitions_path);
        let mut library = Self {
            cache_dir,
            manifest_path,
            models,
            names,
            manifest_mtime,
            definitions_path,
            definitions,
            block_names: Vec::new(),
            definitions_mtime,
            resolved: RefCell::new(HashMap::new()),
        };
        library.rebuild_block_names();
        library
    }

    #[cfg(test)]
    pub(crate) fn from_cache_dir(cache_dir: impl Into<PathBuf>) -> Self {
        let cache_dir = cache_dir.into();
        let manifest_path = cache_dir.join(crate::constants::MODELS_MANIFEST_FILE);
        let definitions_path = cache_dir.join(crate::constants::BLOCKSTATES_MANIFEST_FILE);
        Self::from_paths(cache_dir, manifest_path, definitions_path)
    }

    /// Extracts block models from a Minecraft client jar into the cache and rewrites the manifest.
    #[allow(dead_code)]
    pub fn import_jar(&mut self, jar_path: &Path) -> Result<usize, JarError> {
        let models = import_into_cache(jar_path, &self.cache_dir)?;
        let count = models.len();
        self.replace_models(models);
        self.manifest_mtime = file_mtime(&self.manifest_path);
        Ok(count)
    }

    pub(crate) fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    pub(crate) fn apply_import(
        &mut self,
        models: HashMap<String, BlockModel>,
        definitions: HashMap<String, BlockDefinition>,
    ) {
        self.replace_models(models);
        self.manifest_mtime = file_mtime(&self.manifest_path);
        self.replace_definitions(definitions);
        self.definitions_mtime = file_mtime(&self.definitions_path);
    }

    /// Reloads the dictionaries when `_models.cache.json` or
    /// `_blockstates.cache.json` appears, disappears, or is rewritten.
    ///
    /// A parse or I/O error leaves the current dictionary in place so a partial write
    /// (for example during jar import) does not empty the sidebar.
    pub fn reload_if_changed(&mut self) -> bool {
        let models_changed = self.reload_models_if_changed();
        let definitions_changed = self.reload_definitions_if_changed();
        models_changed || definitions_changed
    }

    fn reload_definitions_if_changed(&mut self) -> bool {
        let mtime = file_mtime(&self.definitions_path);
        if mtime == self.definitions_mtime {
            return false;
        }
        let definitions = match try_load_definitions(&self.definitions_path) {
            Ok(definitions) => definitions,
            Err(ManifestError::NotFound) => HashMap::new(),
            Err(ManifestError::Failed) => return false,
        };
        self.definitions_mtime = mtime;
        self.replace_definitions(definitions);
        true
    }

    fn reload_models_if_changed(&mut self) -> bool {
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

    /// Sorted block stems for the picker, one per block whatever its
    /// variants. A cache imported before blockstates were kept has no
    /// definitions and lists its model stems instead.
    pub fn block_names(&self) -> &[String] {
        &self.block_names
    }

    #[cfg(test)]
    pub fn get_record(&self, key: &str) -> Option<&BlockModel> {
        self.models.get(key)
    }

    /// True when models are cached but blockstates are not: the cache was
    /// imported before blockstates were kept, and the jar must be set again
    /// for blocks to find their models by name.
    pub fn needs_reimport(&self) -> bool {
        self.definitions.is_empty() && !self.models.is_empty()
    }

    /// Whether `block` is one of [`Self::block_names`].
    pub fn has_block(&self, block: &str) -> bool {
        if self.definitions.is_empty() {
            self.models.contains_key(block)
        } else {
            self.definitions.contains_key(block)
        }
    }

    /// Every state `block` can be placed in, as property maps, and the index
    /// of the one a newly picked block starts in. A block without a
    /// definition has only its default state.
    pub fn variants(&self, block: &str) -> (Vec<BTreeMap<String, String>>, usize) {
        let Some(definition) = self.definitions.get(block) else {
            return (vec![BTreeMap::new()], 0);
        };
        let mut variants = definition.variants(Config::MAX_BLOCK_VARIANTS);
        if variants.is_empty() {
            variants.push(BTreeMap::new());
        }
        let start = definition.default_variant(&variants);
        (variants, start)
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
        self.rebuild_block_names();
        self.resolved.borrow_mut().clear();
    }

    fn replace_definitions(&mut self, definitions: HashMap<String, BlockDefinition>) {
        self.definitions = definitions;
        self.rebuild_block_names();
        self.resolved.borrow_mut().clear();
    }

    fn rebuild_block_names(&mut self) {
        self.block_names = if self.definitions.is_empty() {
            self.names.clone()
        } else {
            let mut names: Vec<String> = self.definitions.keys().cloned().collect();
            names.sort();
            names
        };
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
        let (applied, property_rotation): (Vec<AppliedModel>, bool) =
            match self.definitions.get(&stem) {
                Some(definition) => (
                    definition.select(properties).into_iter().cloned().collect(),
                    !definition.mentions(AXIS_PROPERTY) && !definition.mentions(FACING_PROPERTY),
                ),
                None => (
                    vec![AppliedModel::unrotated(self.undefined_model_stem(&stem))],
                    true,
                ),
            };

        let mut block = ResolvedBlock {
            properties: properties.clone(),
            parent_stem: None,
            particle_texture: None,
            chain_texture: None,
            property_rotation,
            parts: Vec::with_capacity(applied.len()),
        };
        for (index, model) in applied.iter().enumerate() {
            let (part, parent_stem, particle_texture) = self.resolve_part(model);
            if index == 0 {
                block.particle_texture = particle_texture;
                // Only a block made of one model can take a full-cube fast path.
                if applied.len() == 1 {
                    block.parent_stem = parent_stem;
                }
            }
            block.parts.push(part);
        }
        if stem.ends_with("_hanging_sign")
            && block.parts.first().is_some_and(|part| part.block_entity)
        {
            block.chain_texture = self.chain_texture();
        }
        block
    }

    /// Model for a block with no blockstate definition: the model of the
    /// same name. Wall signs and banners have none of their own and share
    /// the standing sign's model and the one `banner` model.
    fn undefined_model_stem(&self, stem: &str) -> String {
        match wall_sign_model_stem(stem).or_else(|| banner_model_stem(stem)) {
            Some(shared) if !self.models.contains_key(stem) => shared,
            _ => stem.to_owned(),
        }
    }

    /// Resolves one model a blockstate applies. Returns the part with the
    /// model's parent stem and `particle` texture. An unknown model or a
    /// broken chain gives a full cube with the missing texture.
    fn resolve_part(
        &self,
        applied: &AppliedModel,
    ) -> (ResolvedPart, Option<String>, Option<String>) {
        let stem = applied.model.as_str();
        let part = |elements, block_entity| ResolvedPart {
            model: stem.to_owned(),
            elements,
            block_entity,
            x: applied.x,
            y: applied.y,
            uvlock: applied.uvlock,
        };
        let Some(record) = self.models.get(stem) else {
            log::warn!("model `{stem}` is not in the library; using fallback cube");
            return (fallback_part(), None, None);
        };
        let Some(merged) = self.merge_textures(stem) else {
            log::warn!("model `{stem}` parent chain exceeded depth limit; using fallback cube");
            return (fallback_part(), None, None);
        };
        let particle_texture = merged.get("particle").cloned();
        let parent_stem = record.parent().map(str::to_owned);

        let Some(elements_from) = record.elements_from() else {
            return (part(Vec::new(), true), parent_stem, particle_texture);
        };
        match self.read_and_bind_elements(elements_from, &merged) {
            Ok(elements) => (part(elements, false), parent_stem, particle_texture),
            Err(err) => {
                log::warn!(
                    "model `{stem}` geometry from `{elements_from}` could not be read: {err}; using fallback cube"
                );
                (fallback_part(), None, None)
            }
        }
    }

    /// Texture of the chain block, whose model is `chain` before 1.21.9 and
    /// `iron_chain` after.
    fn chain_texture(&self) -> Option<String> {
        ["chain", "iron_chain"]
            .into_iter()
            .find_map(|stem| self.merge_textures(stem)?.remove("particle"))
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

/// Model stem a wall-mounted sign shares with its free-standing counterpart:
/// `oak_wall_sign` -> `oak_sign`, `oak_wall_hanging_sign` -> `oak_hanging_sign`.
fn wall_sign_model_stem(stem: &str) -> Option<String> {
    if let Some(wood) = stem.strip_suffix("_wall_hanging_sign") {
        Some(format!("{wood}_hanging_sign"))
    } else {
        stem.strip_suffix("_wall_sign")
            .map(|wood| format!("{wood}_sign"))
    }
}

/// Model stem shared by all banners: `red_banner` and `red_wall_banner` -> `banner`.
fn banner_model_stem(stem: &str) -> Option<String> {
    stem.ends_with("_banner").then(|| "banner".to_owned())
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

/// A full cube with the missing texture on every face.
fn fallback_part() -> ResolvedPart {
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
    ResolvedPart {
        model: "missing".into(),
        elements: vec![ResolvedElement {
            from: [0.0, 0.0, 0.0],
            to: [16.0, 16.0, 16.0],
            rotation: None,
            faces,
            shade: true,
        }],
        block_entity: false,
        x: 0,
        y: 0,
        uvlock: false,
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

fn try_load_definitions(path: &Path) -> Result<HashMap<String, BlockDefinition>, ManifestError> {
    match fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text).map_err(|err| {
            log::error!(
                "failed to parse blockstate manifest {}: {err}",
                path.display()
            );
            ManifestError::Failed
        }),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Err(ManifestError::NotFound),
        Err(err) => {
            log::error!(
                "failed to read blockstate manifest {}: {err}",
                path.display()
            );
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
            dir.join(crate::constants::MODELS_MANIFEST_FILE),
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
            library.block_names(),
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
        assert!(library.block_names().contains(&"stone".to_owned()));

        fs::write(
            dir.join(crate::constants::MODELS_MANIFEST_FILE),
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
            .open(dir.join(crate::constants::MODELS_MANIFEST_FILE))
            .unwrap()
            .set_modified(later)
            .unwrap();

        assert!(library.reload_if_changed());
        assert_eq!(library.block_names(), &["dirt".to_owned()]);
        assert!(!library.reload_if_changed());

        fs::remove_file(dir.join(crate::constants::MODELS_MANIFEST_FILE)).unwrap();
        assert!(library.reload_if_changed());
        assert!(library.block_names().is_empty());

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
    fn resolve_wall_signs_share_the_standing_sign_model() {
        let dir = temp_dir("model-library-signs");
        fs::write(
            dir.join(crate::constants::MODELS_MANIFEST_FILE),
            r##"{
                "oak_sign": {
                    "file_name": "oak_sign.json",
                    "parent": null,
                    "textures": { "particle": "oak_planks" },
                    "elements_from": null
                },
                "oak_hanging_sign": {
                    "file_name": "oak_hanging_sign.json",
                    "parent": null,
                    "textures": { "particle": "stripped_oak_log" },
                    "elements_from": null
                },
                "chain": {
                    "file_name": "chain.json",
                    "parent": null,
                    "textures": { "all": "chain", "particle": "#all" },
                    "elements_from": "chain"
                }
            }"##,
        )
        .unwrap();
        let library = ModelLibrary::from_cache_dir(&dir);
        let empty = BTreeMap::new();

        let wall = library.resolve("minecraft:oak_wall_sign", &empty);
        assert_eq!(wall.parts[0].model, "oak_sign");
        assert!(wall.parts[0].block_entity);
        assert_eq!(wall.particle_texture.as_deref(), Some("oak_planks"));
        assert_eq!(wall.chain_texture, None);

        let hanging = library.resolve("minecraft:oak_wall_hanging_sign", &empty);
        assert_eq!(hanging.parts[0].model, "oak_hanging_sign");
        assert_eq!(
            hanging.particle_texture.as_deref(),
            Some("stripped_oak_log")
        );
        assert_eq!(hanging.chain_texture.as_deref(), Some("chain"));
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
        assert!(
            cache_dir
                .join(crate::constants::MODELS_MANIFEST_FILE)
                .is_file()
        );
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

    fn write_definitions(dir: &Path) {
        fs::write(
            dir.join(crate::constants::BLOCKSTATES_MANIFEST_FILE),
            r#"{
                "pebble": {"variants": [
                    {"properties": {"axis": "x"}, "apply": {"model": "stone", "x": 90, "y": 90}},
                    {"properties": {"axis": "y"}, "apply": {"model": "stone"}}
                ]},
                "pair": {"multipart": [
                    {"when": null, "apply": {"model": "stone"}},
                    {"when": {"properties": {"on": ["true"]}},
                     "apply": {"model": "chest", "y": 180, "uvlock": true}}
                ]}
            }"#,
        )
        .unwrap();
    }

    fn props(name: &str, value: &str) -> BTreeMap<String, String> {
        BTreeMap::from([(name.to_owned(), value.to_owned())])
    }

    #[test]
    fn block_resolves_through_its_definition_to_differently_named_models() {
        let dir = temp_dir("model-library-definitions");
        write_model_cache(&dir);
        write_definitions(&dir);
        let library = ModelLibrary::from_cache_dir(&dir);

        let upright = library.resolve("minecraft:pebble", &props("axis", "y"));
        assert_eq!(upright.parts.len(), 1);
        assert_eq!(upright.parts[0].model, "stone");
        assert_eq!(upright.parent_stem.as_deref(), Some("cube_all"));
        assert_eq!(upright.particle_texture.as_deref(), Some("stone"));
        // The definition reads `axis`, so it orients the block itself.
        assert!(!upright.property_rotation);

        let sideways = library.resolve("minecraft:pebble", &props("axis", "x"));
        assert_eq!((sideways.parts[0].x, sideways.parts[0].y), (90, 90));
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn multipart_block_resolves_one_part_per_case_that_holds() {
        let dir = temp_dir("model-library-multipart");
        write_model_cache(&dir);
        write_definitions(&dir);
        let library = ModelLibrary::from_cache_dir(&dir);

        let off = library.resolve("minecraft:pair", &BTreeMap::new());
        assert_eq!(off.parts.len(), 1);
        assert!(off.property_rotation);

        let on = library.resolve("minecraft:pair", &props("on", "true"));
        let models: Vec<&str> = on.parts.iter().map(|part| part.model.as_str()).collect();
        assert_eq!(models, ["stone", "chest"]);
        assert_eq!(on.parent_stem, None);
        assert!(on.parts[1].block_entity);
        assert_eq!(on.parts[1].y, 180);
        assert!(on.parts[1].uvlock);
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn picker_lists_blocks_not_models_once_definitions_exist() {
        let dir = temp_dir("model-library-picker");
        write_model_cache(&dir);
        let without = ModelLibrary::from_cache_dir(&dir);
        assert!(without.has_block("stone"));
        assert_eq!(without.variants("stone"), (vec![BTreeMap::new()], 0));

        write_definitions(&dir);
        let library = ModelLibrary::from_cache_dir(&dir);
        assert_eq!(library.block_names(), ["pair", "pebble"]);
        assert!(library.has_block("pebble"));
        assert!(!library.has_block("stone"));

        // The unrotated variant is the one a newly picked block starts in.
        let (variants, start) = library.variants("pebble");
        assert_eq!(variants, [props("axis", "x"), props("axis", "y")]);
        assert_eq!(start, 1);
        let (variants, start) = library.variants("pair");
        assert_eq!(variants, [props("on", "false"), props("on", "true")]);
        assert_eq!(start, 0);
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn definitions_reload_when_their_manifest_appears() {
        let dir = temp_dir("model-library-definitions-reload");
        write_model_cache(&dir);
        let mut library = ModelLibrary::from_cache_dir(&dir);
        assert!(!library.reload_if_changed());
        write_definitions(&dir);
        assert!(library.reload_if_changed());
        assert_eq!(library.block_names(), ["pair", "pebble"]);
        assert!(!library.reload_if_changed());
        fs::remove_dir_all(dir).ok();
    }
}
