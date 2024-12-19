#[cfg(feature = "async")]
pub mod r#async;
pub mod event;
pub mod thread;

pub trait Context: Sized {}

pub trait FromContext<'a, C>
where
    C: Context,
{
    fn from_context(cx: &'a C) -> Self;
}
