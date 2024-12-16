#[cfg(feature = "role")]
use crate::role::RoleHolder;
use crate::syzygy::Syzygy;

#[cfg(feature = "async")]
pub mod r#async;
pub mod event;
pub mod thread;

pub trait Context: Sized + Clone + 'static {
    fn execute<C, F, R>(&self, f: F) -> R
    where
        F: FnOnce(C) -> R,
        C: FromContext<Self>,
        Self: FromContext<Syzygy>,
    {
        f(C::from_context(self.clone()))
    }
}

pub trait FromContext<C>: Context
where
    C: FromContext<Syzygy>,
{
    fn from_context(cx: C) -> Self;
}

pub trait IntoContext<C>: Context
where
    C: FromContext<Syzygy>,
{
    fn into_context(self) -> C;
}

impl<T, U> IntoContext<U> for T
where
    T: Context + FromContext<Syzygy>,
    U: FromContext<T> + FromContext<Syzygy>,
{
    fn into_context(self) -> U {
        U::from_context(self)
    }
}
