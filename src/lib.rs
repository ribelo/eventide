#![feature(downcast_unchecked)]
pub mod bind;

pub mod prelude {
    pub use crate::bind::{ctx, capabilities::*, AppContext, Effect};
}
