use crate::{role::RoleHolder, syzygy::Syzygy};

#[cfg(feature = "async")]
pub mod r#async;
pub mod event;
pub mod thread;

pub trait Context: RoleHolder + Sized + Clone + 'static {}

pub trait FromContext<C>: Context
where
    C: FromContext<Syzygy>,
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
