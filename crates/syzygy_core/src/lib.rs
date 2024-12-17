#![feature(downcast_unchecked)]
pub mod context;
pub mod effect_bus;
pub mod event_bus;
pub mod model;
pub mod resource;
pub mod spawn;
pub mod syzygy;

pub mod prelude {
    pub use crate::context::{Context, FromContext};
    pub use crate::event_bus::{EmitEvent, EventBus, Subscribe, Unsubscribe};
    pub use crate::model::{Model, ModelAccess, ModelModify, ModelMut, Models};
    pub use crate::resource::{ResourceAccess, Resources};
    #[cfg(feature = "async")]
    pub use crate::spawn::SpawnAsync;
    #[cfg(feature = "parallel")]
    pub use crate::spawn::SpawnParallel;
    pub use crate::spawn::SpawnThread;
    pub use crate::syzygy::Syzygy;
}
