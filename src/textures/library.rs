use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use image::RgbaImage;

use crate::config::Config;

use super::jar::{JarError, import_into_cache};
use super::texture::Texture;

/// In-memory dictionary of cached block textures, backed by the cache manifest.
pub struct TextureLibrary {
    cache_dir: PathBuf,
    textures: HashMap<String, Texture>,
    decoded: RefCell<HashMap<String, Arc<RgbaImage>>>,
    missing: Arc<RgbaImage>,
}

impl TextureLibrary {
    /// Loads the fallback image and, if present, deserialises the cache manifest.
    pub fn load() -> Self {
        let cache_dir = Config::texture_cache_dir().to_path_buf();
        let missing = load_missing_texture(Config::missing_texture_path());
        let textures = load_manifest(&Config::textures_manifest_path());
        Self {
            cache_dir,
            textures,
            decoded: RefCell::new(HashMap::new()),
            missing,
        }
    }

    #[cfg(test)]
    pub(crate) fn from_paths(
        cache_dir: impl Into<PathBuf>,
        missing_path: impl AsRef<Path>,
    ) -> Self {
        let cache_dir = cache_dir.into();
        let missing = load_missing_texture(missing_path.as_ref());
        let textures = load_manifest(&cache_dir.join(Config::TEXTURES_MANIFEST_FILE));
        Self {
            cache_dir,
            textures,
            decoded: RefCell::new(HashMap::new()),
            missing,
        }
    }

    /// Extracts block textures from a Minecraft client jar into the cache and rewrites the manifest.
    pub fn import_jar(&mut self, jar_path: &Path) -> Result<usize, JarError> {
        let textures = import_into_cache(jar_path, &self.cache_dir)?;
        let count = textures.len();
        self.apply_import(textures);
        Ok(count)
    }

    pub(crate) fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    pub(crate) fn apply_import(&mut self, textures: HashMap<String, Texture>) {
        self.textures = textures;
        self.decoded.borrow_mut().clear();
    }

    /// Decoded RGBA pixels for `key`, or the missing texture if the key or file cannot be loaded.
    ///
    /// Successful and fallback results are memoised so repeated queries do not re-read disk.
    #[allow(dead_code)] // queried by the renderer once textured meshes land
    pub fn get(&self, key: &str) -> Arc<RgbaImage> {
        if let Some(cached) = self.decoded.borrow().get(key) {
            return Arc::clone(cached);
        }
        let image = self.load_or_missing(key);
        self.decoded
            .borrow_mut()
            .insert(key.to_owned(), Arc::clone(&image));
        image
    }

    #[allow(dead_code)] // queried by the renderer once textured meshes land
    pub fn get_record(&self, key: &str) -> Option<&Texture> {
        self.textures.get(key)
    }

    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.textures.len()
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.textures.is_empty()
    }

    fn load_or_missing(&self, key: &str) -> Arc<RgbaImage> {
        let Some(texture) = self.textures.get(key) else {
            log::warn!("texture `{key}` is not in the library; using missing texture");
            return Arc::clone(&self.missing);
        };
        let path = self.cache_dir.join(texture.file_name());
        match image::open(&path) {
            Ok(img) => Arc::new(img.to_rgba8()),
            Err(err) => {
                log::warn!(
                    "texture `{key}` at {} could not be read: {err}; using missing texture",
                    path.display()
                );
                Arc::clone(&self.missing)
            }
        }
    }
}

fn load_manifest(path: &Path) -> HashMap<String, Texture> {
    match fs::read_to_string(path) {
        Ok(text) => match serde_json::from_str(&text) {
            Ok(map) => map,
            Err(err) => {
                log::error!("failed to parse texture manifest {}: {err}", path.display());
                HashMap::new()
            }
        },
        Err(err) if err.kind() == io::ErrorKind::NotFound => HashMap::new(),
        Err(err) => {
            log::error!("failed to read texture manifest {}: {err}", path.display());
            HashMap::new()
        }
    }
}

fn load_missing_texture(path: &Path) -> Arc<RgbaImage> {
    match image::open(path) {
        Ok(img) => Arc::new(img.to_rgba8()),
        Err(err) => {
            log::error!(
                "failed to load missing texture from {}: {err}",
                path.display()
            );
            Arc::new(generate_missing_texture())
        }
    }
}

fn generate_missing_texture() -> RgbaImage {
    const SIZE: u32 = 16;
    const MAGENTA: image::Rgba<u8> = image::Rgba([248, 0, 248, 255]);
    const BLACK: image::Rgba<u8> = image::Rgba([0, 0, 0, 255]);
    let mut img = RgbaImage::new(SIZE, SIZE);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let color = if (x / 8 + y / 8) % 2 == 0 {
                MAGENTA
            } else {
                BLACK
            };
            img.put_pixel(x, y, color);
        }
    }
    img
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};
    use std::io::Cursor;

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

    fn write_png(path: &Path, color: [u8; 4]) {
        let img = RgbaImage::from_pixel(2, 2, Rgba(color));
        let mut bytes = Vec::new();
        img.write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)
            .unwrap();
        fs::write(path, bytes).unwrap();
    }

    #[test]
    fn load_reads_manifest_and_get_returns_pixels() {
        let dir = temp_dir("library-load");
        let missing_path = dir.join("missing.png");
        write_png(&missing_path, [255, 0, 255, 255]);
        write_png(&dir.join("stone.png"), [80, 80, 80, 255]);
        fs::write(
            dir.join(Config::TEXTURES_MANIFEST_FILE),
            r#"{
                "stone": { "file_name": "stone.png", "format": "png", "has_alpha": false }
            }"#,
        )
        .unwrap();

        let library = TextureLibrary::from_paths(&dir, &missing_path);
        assert_eq!(library.len(), 1);
        assert!(!library.get_record("stone").unwrap().has_alpha());

        let pixels = library.get("stone");
        assert_eq!(pixels.get_pixel(0, 0).0, [80, 80, 80, 255]);
        assert!(Arc::ptr_eq(&pixels, &library.get("stone")));

        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn get_falls_back_to_missing_texture() {
        let dir = temp_dir("library-missing");
        let missing_path = dir.join("missing.png");
        write_png(&missing_path, [1, 2, 3, 255]);

        let library = TextureLibrary::from_paths(&dir, &missing_path);
        assert!(library.is_empty());
        let missing = library.get("glass");
        assert_eq!(missing.get_pixel(0, 0).0, [1, 2, 3, 255]);
        assert!(Arc::ptr_eq(&missing, &library.get("glass")));

        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn missing_file_is_replaced_by_generated_checkerboard() {
        let dir = temp_dir("library-generated");
        let library = TextureLibrary::from_paths(&dir, dir.join("does-not-exist.png"));
        let img = library.get("anything");
        assert_eq!(img.width(), 16);
        assert_eq!(img.height(), 16);
        assert_eq!(img.get_pixel(0, 0).0, [248, 0, 248, 255]);
        assert_eq!(img.get_pixel(8, 0).0, [0, 0, 0, 255]);
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn import_jar_writes_manifest() {
        use std::io::{Cursor, Write};
        use zip::ZipWriter;
        use zip::write::SimpleFileOptions;

        let dir = temp_dir("library-import");
        let missing_path = dir.join("missing.png");
        write_png(&missing_path, [1, 2, 3, 255]);
        let jar_path = dir.join("client.jar");
        let cache_dir = dir.join("cache");
        fs::create_dir_all(&cache_dir).unwrap();

        let mut png = Vec::new();
        RgbaImage::from_pixel(2, 2, Rgba([10, 20, 30, 255]))
            .write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        {
            let file = fs::File::create(&jar_path).unwrap();
            let mut zip = ZipWriter::new(file);
            let options =
                SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
            zip.start_file("assets/minecraft/textures/block/stone.png", options)
                .unwrap();
            zip.write_all(&png).unwrap();
            zip.finish().unwrap();
        }

        let mut library = TextureLibrary::from_paths(&cache_dir, &missing_path);
        assert_eq!(library.import_jar(&jar_path).unwrap(), 1);
        assert_eq!(library.len(), 1);
        assert!(cache_dir.join(Config::TEXTURES_MANIFEST_FILE).is_file());
        assert_eq!(library.get("stone").get_pixel(0, 0).0, [10, 20, 30, 255]);

        fs::remove_dir_all(dir).ok();
    }
}
