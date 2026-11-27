mod block_definition;
mod block_model;
mod library;
mod model_json;

pub use block_definition::BlockDefinition;
pub use block_model::BlockModel;
pub use library::ModelLibrary;
#[allow(unused_imports)]
// public module API; consumed by `models` once the viewport draws blocks
pub use library::{ResolvedBlock, ResolvedElement, ResolvedFace, ResolvedPart};
#[allow(unused_imports)]
pub use model_json::ModelRotation;
pub(crate) use model_json::asset_stem;
