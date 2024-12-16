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
        C: FromContext<Self>,
        F: FnOnce(C) -> R,
        Self:
    {
        f(C::from_context(self.clone()))
    }
}

pub trait FromContext<C>: Context
where
    C: Context,
{
    fn from_context(cx: C) -> Self;
}

// impl<C: Context> FromContext<C> for C {
//     fn from_context(cx: C) -> C {
//         cx
//     }
// }

pub trait IntoContext<C>: Context
where
    C: Context,
{
    fn into_context(self) -> C;
}

impl<T, U> IntoContext<U> for T
where
    T: Context,
    U: FromContext<T>,
{
    fn into_context(self) -> U {
        U::from_context(self)
    }
}
