// #[cfg(feature = "sync")]
#[cfg(feature = "unsync")]
mod unsync;
// #[cfg(feature = "sync")]
// mod sync;

#[cfg(feature = "unsync")]
pub use unsync::{Model, ModelAccess, ModelModify, ModelMut, Models, ModelsBuilder};

// #[cfg(feature = "sync")]
// pub use sync::{SyncModelAccess, SyncModelMut, SyncModels, SyncModelsBuilder};
