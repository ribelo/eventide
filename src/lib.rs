#![feature(downcast_unchecked)]
pub mod eve;

pub mod prelude {
    #[cfg(feature = "tokio")]
    pub use crate::eve::AsyncTaskContext;
    pub use crate::eve::{AppContext, Effect, ModelContext, TaskContext};
}
