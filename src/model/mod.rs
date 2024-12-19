// #[cfg(feature = "sync")]
#[cfg(feature = "sync")]
mod sync;
#[cfg(feature = "unsync")]
mod unsync;

#[cfg(all(feature = "sync", feature = "unsync"))]
compile_error!("Features 'sync' and 'unsync' cannot be enabled simultaneously");

#[cfg(feature = "unsync")]
pub use unsync::{UnsyncModelAccess as ModelAccess, UnsyncModelModify as ModelModify};

#[cfg(feature = "sync")]
pub use sync::{
    SyncModel as Model, SyncModelAccess as ModelAccess, SyncModelModify as ModelModify,
    SyncModelMut as ModelMut, SyncModels as Models, SyncModelsBuilder as ModelsBuilder,
};
