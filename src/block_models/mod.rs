mod block_model;
mod jar;
mod library;
mod model_json;

pub use block_model::BlockModel;
pub(crate) use jar::{
    fill_elements_from, ingest_model, match_block_model, remove_stale_cache_files, write_manifest,
};
pub use library::ModelLibrary;
#[allow(unused_imports)]
// public module API; used once the mesh builder consumes resolved models
pub use library::{ResolvedBlock, ResolvedElement, ResolvedFace, ResolvedPart};
