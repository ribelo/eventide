use crate::{effects::Effect, model::Model};

pub mod r#async;
pub mod thread;

pub trait Context: Sized {
    type Model: Model;
    type Effect: Effect<Self::Model>;
}

pub trait FromContext<T>: Context {
    fn from_context(context: &T) -> Self;
}
