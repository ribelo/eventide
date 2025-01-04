#[cfg(feature = "async")]
pub mod r#async;
pub mod thread;

pub trait Context: Sized {}
