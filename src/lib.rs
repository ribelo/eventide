#![feature(downcast_unchecked)]
pub mod context;
pub mod dispatch;
pub mod model;
pub mod resource;
pub mod syzygy;

pub mod prelude {
    pub use crate::context::{
        r#async::AsyncContext, Context, FromContext, IntoContext,
    };
    pub use crate::dispatch::{DispatchEffect, Effects, EffectsQueue};
    pub use crate::model::{ModelAccess, ModelModify};
    pub use crate::resource::{ResourceAccess, ResourceModify, Resources};
    #[cfg(feature = "parallel")]
    pub use crate::spawn::{RayonPool, SpawnParallel};
    pub use crate::syzygy::Syzygy;
}
