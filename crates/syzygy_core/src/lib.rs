#![feature(downcast_unchecked)]
pub mod context;
pub mod dispatch;
pub mod event_bus;
pub mod model;
#[cfg(feature = "role")]
pub mod role;
pub mod resource;
pub mod spawn;
pub mod syzygy;

pub mod prelude {
    pub use crate::context::{Context, FromContext};
    pub use crate::event_bus::{EmitEvent, EventBus, Subscribe, Unsubscribe};
    pub use crate::model::{ModelAccess, ModelMut, Models};
    #[cfg(feature = "role")]
    pub use crate::role::{ImpliedBy, Role, RoleGuarded, RoleHolder, AnyRole, Root};
    pub use crate::resource::{ResourceAccess, Resources};
    pub use crate::spawn::SpawnThread;
    #[cfg(feature = "async")]
    pub use crate::spawn::SpawnAsync;
    #[cfg(feature = "parallel")]
    pub use crate::spawn::SpawnParallel;
    pub use crate::syzygy::Syzygy;
}
