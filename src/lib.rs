#![feature(downcast_unchecked)]
#![feature(min_specialization)]
pub mod context;
pub mod dispatch;
pub mod model;
pub mod resource;
pub mod syzygy;

pub mod prelude {
    pub use crate::context::{Context, FromContext, IntoContext, r#async::AsyncContext};
    pub use crate::dispatch::DispatchEffect;
    pub use crate::model::{ModelAccess, ModelModify};
    pub use crate::resource::{ResourceAccess, ResourceModify, Resources};
    #[cfg(feature = "parallel")]
    pub use crate::spawn::{RayonPool, SpawnParallel};
    pub use crate::syzygy::Syzygy;
}
