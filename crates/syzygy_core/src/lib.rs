#![feature(downcast_unchecked)]
pub mod context;
pub mod dispatch;
pub mod event_bus;
pub mod model;
pub mod resource;
pub mod spawn;
pub mod syzygy;
pub mod permission;

pub mod prelude {
    pub use crate::event_bus::{EmitEvent, EventBus, Subscribe, Unsubscribe};
    pub use crate::model::{ModelAccess, ModelMut, Models};
    pub use crate::resource::{ResourceAccess, Resources};
}
