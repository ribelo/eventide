#[cfg(feature = "async")]
use std::future::Future;

#[cfg(feature = "async")]
pub mod r#async;
pub mod event;
pub mod thread;

pub trait Context: Sized + Clone + 'static {}

pub trait FromContext<C>
where
    C: Context,
{
    fn from_context(cx: &C) -> Self;
}

pub trait ContextHandler<C, R>
where
    C: Context,
{
    fn call(self, cx: C) -> R;
}

impl<C, F, R> ContextHandler<C, R> for F
where
    C: Context,
    F: FnOnce(C) -> R,
{
    fn call(self, cx: C) -> R {
        (self)(cx)
    }
}

#[cfg(feature = "async")]
pub trait AsyncContextHandler<C, T, Fut, R>
where
    C: Context,
    Fut: Future<Output = R>,
{
    fn call(self, cx: C) -> Fut;
}

#[cfg(feature = "async")]
impl<C, F, Fut, R> AsyncContextHandler<C, (), Fut, R> for F
where
    C: Context,
    F: FnOnce() -> Fut,
    Fut: Future<Output = R>,
{
    fn call(self, _cx: C) -> Fut {
        (self)()
    }
}

#[cfg(feature = "async")]
impl<C, F, Fut, R> AsyncContextHandler<C, (C,), Fut, R> for F
where
    C: Context,
    F: FnOnce(C) -> Fut,
    Fut: Future<Output = R>,
{
    fn call(self, cx: C) -> Fut {
        (self)(cx.clone())
    }
}
