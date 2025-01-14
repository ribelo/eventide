use crate::model::Model;

pub mod r#async;

pub trait Context: Sized {
    type Model: Model;
}

pub trait FromContext<T>: Context {
    fn from_context(context: &T) -> Self;
}

pub trait IntoContext<T>: Context {
    fn into_context(self) -> T;
}
