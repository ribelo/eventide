#![feature(downcast_unchecked)]
pub mod bind;

pub mod prelude {
    pub use crate::bind::{cx, capabilities::*, GlobalAppContext, AppContext, Effect, Model};
}
