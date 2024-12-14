pub mod thread;
pub mod event;
#[cfg(feature = "async")]
pub mod r#async;

pub trait Context: Sized + 'static {}

pub trait FromContext<C>
where
    C: Context,
{
    fn from_context(cx: C) -> Self;
}

pub trait TryFromContext<C>
where
    C: Context,
{
    type Error;
    fn try_from_context(cx: C) -> Result<Self, Self::Error>
    where
        Self: Sized;
}
