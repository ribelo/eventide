use crate::{dispatch::Effect, model::Model};

pub mod r#async;
pub mod thread;
pub mod effect;

pub trait Context: Sized {
    type Model: Model;
}

pub trait FromContext<T>: Context {
    fn from_context(context: &T) -> Self;
}
