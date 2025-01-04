pub mod r#async;
pub mod thread;

pub trait Context: Sized {}

pub trait IntoContext<T>: Sized {
    fn into_context(&self) -> T;
}
