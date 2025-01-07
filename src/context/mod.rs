use crate::{dispatch::Effect, model::Model};

pub mod r#async;
pub mod thread;

pub trait Context: Sized {
    type Model: Model;
    type Effect: Effect;
}

pub trait FromContext<T>: Context {
    fn from_context(context: &T) -> Self;
}

pub trait IntoContext<T> {
    fn into_context(self) -> T;
}

impl<C, T> IntoContext<C> for T
where
    C: FromContext<T>
{
    fn into_context(self) -> C {
        C::from_context(&self)
    }
}
