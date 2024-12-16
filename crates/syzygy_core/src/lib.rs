#![feature(downcast_unchecked)]
pub mod context;
pub mod dispatch;
pub mod event_bus;
pub mod model;
pub mod resource;
pub mod spawn;
pub mod syzygy;

pub mod prelude {
    pub use crate::context::{Context, BorrowFromContext};
    pub use crate::event_bus::{EmitEvent, EventBus, Subscribe, Unsubscribe};
    pub use crate::model::{ModelAccess, ModelModify, Models, Model, ModelMut};
    pub use crate::resource::{ResourceAccess, Resources};
    pub use crate::spawn::SpawnThread;
    #[cfg(feature = "async")]
    pub use crate::spawn::SpawnAsync;
    #[cfg(feature = "parallel")]
    pub use crate::spawn::SpawnParallel;
    pub use crate::syzygy::Syzygy;
}
