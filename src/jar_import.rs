use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::block_models::{
    BlockModel, fill_elements_from, ingest_model, match_block_model, remove_stale_cache_files,
    write_manifest as write_models_manifest,
};
use crate::textures::jar::{
    for_each_jar_file, ingest_texture, match_block_texture,
    remove_stale_cache_files as remove_stale_textures, write_manifest as write_textures_manifest,
};
use crate::textures::{JarError, Texture};

/// Result of a single pass over a Minecraft client jar.
pub struct ImportedAssets {
    pub textures: HashMap<String, Texture>,
    pub models: HashMap<String, BlockModel>,
}

/// Opens the jar once and offers every entry to the texture matcher and the model matcher.
pub fn import_from_jar(
    jar_path: &Path,
    texture_cache_dir: &Path,
    model_cache_dir: &Path,
    textures_manifest: &Path,
    models_manifest: &Path,
) -> Result<ImportedAssets, JarError> {
    fs::create_dir_all(texture_cache_dir)?;
    fs::create_dir_all(model_cache_dir)?;

    let mut textures = HashMap::new();
    let mut models = HashMap::new();
    let mut has_elements = HashMap::new();

    for_each_jar_file(jar_path, |name, bytes| {
        if let Some((file_name, format)) = match_block_texture(name)
            && let Some((key, texture)) =
                ingest_texture(texture_cache_dir, file_name, format, &bytes)
        {
            textures.insert(key, texture);
        }
        if let Some(file_name) = match_block_model(name)
            && let Some((key, model, elements)) = ingest_model(model_cache_dir, file_name, &bytes)
        {
            has_elements.insert(key.clone(), elements);
            models.insert(key, model);
        }
    })?;

    fill_elements_from(&mut models, &has_elements);
    remove_stale_textures(texture_cache_dir, &textures);
    remove_stale_cache_files(model_cache_dir, &models);
    write_textures_manifest(textures_manifest, &textures)?;
    write_models_manifest(models_manifest, &models)?;

    Ok(ImportedAssets { textures, models })
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
        let textures_manifest = dir.join(crate::config::Config::TEXTURES_MANIFEST_FILE);
        let models_manifest = dir.join(crate::config::Config::MODELS_MANIFEST_FILE);
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

        fs::remove_dir_all(dir).ok();
    }
}
