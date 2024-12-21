#![feature(downcast_unchecked)]
pub mod context;
pub mod effect_bus;
pub mod model;
pub mod resource;
pub mod spawn;
pub mod syzygy;

pub mod prelude {
    pub use crate::context::Context;
    pub use crate::effect_bus::{DispatchEffect, EffectBus, EffectError};
    pub use crate::model::{ModelAccess, ModelModify};
    pub use crate::resource::{ResourceAccess, Resources};
    #[cfg(feature = "async")]
    pub use crate::spawn::SpawnAsync;
    #[cfg(feature = "parallel")]
    pub use crate::spawn::SpawnParallel;
    pub use crate::spawn::SpawnThread;
    pub use crate::syzygy::Syzygy;
}
