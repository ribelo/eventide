#![feature(downcast_unchecked)]
pub mod context;
pub mod dispatch;
pub mod model;
pub mod resource;
pub mod spawn;
pub mod syzygy;

pub mod prelude {
    pub use crate::context::{
        r#async::AsyncContext, thread::ThreadContext, Context, FromContext, IntoContext,
    };
    pub use crate::dispatch::{DispatchEffect, Messages, EffectSender};
    pub use crate::model::{ModelAccess, ModelModify};
    pub use crate::resource::{ResourceAccess, ResourceModify, Resources};
    pub use crate::spawn::SpawnThread;
    #[cfg(feature = "parallel")]
    pub use crate::spawn::{RayonPool, SpawnParallel};
    pub use crate::spawn::{SpawnAsync, TokioHandle};
    pub use crate::syzygy::Syzygy;
}
