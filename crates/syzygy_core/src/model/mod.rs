// #[cfg(feature = "sync")]
// mod sync;
#[cfg(feature = "unsync")]
mod unsync;

#[cfg(feature = "unsync")]
pub use unsync::{ModelAccess, ModelMut, Models, ModelsBuilder};

// #[cfg(feature = "sync")]
// pub use sync::{SyncModelAccess, SyncModelMut, SyncModels, SyncModelsBuilder};
