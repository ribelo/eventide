#[cfg(feature = "async")]
use std::future::Future;
#[cfg(feature = "parallel")]
use std::sync::Arc;

#[cfg(any(feature = "async", feature = "parallel"))]
use std::ops::Deref;

#[cfg(feature = "async")]
use crate::context::r#async::AsyncContext;
use crate::context::thread::ThreadContext;
use crate::model::Model;
use crate::{effect_bus::DispatchEffect, resource::ResourceAccess};

#[derive(Debug, thiserror::Error)]
#[error("Thread spawn failed")]
pub struct SpawnTaskError(#[from] std::io::Error);

pub trait SpawnThread<M: Model>:
    Into<ThreadContext<M>> + DispatchEffect<M> + ResourceAccess + 'static
{
    fn spawn<H, R>(&self, handler: H) -> crossbeam_channel::Receiver<R>
    where
        H: FnOnce(ThreadContext<M>) -> R + Send + Sync + 'static,
        R: Send + 'static,
    {
        let (tx, rx) = crossbeam_channel::bounded(1);
        let ctx = self.clone().into();
        std::thread::spawn(move || {
            let result = (handler)(ctx);
            let _ = tx.send(result);
        });

        rx
    }
}

#[cfg(feature = "async")]
#[derive(Debug, thiserror::Error)]
pub enum AsyncTaskError {
    #[error("Task spawn failed")]
    SpawnError,
    #[error("Channel send failed")]
    SendError,
    #[error("Channel receive failed")]
    ReceiveError,
}

#[cfg(feature = "async")]
pub trait SpawnAsync<M: Model>:
    Into<AsyncContext<M>> + DispatchEffect<M> + ResourceAccess + 'static
{
    fn tokio_handle(&self) -> &TokioHandle;
    fn spawn_task<H, Fut, R>(&self, handler: H) -> tokio::sync::oneshot::Receiver<R>
    where
        H: FnOnce(AsyncContext<M>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = R> + Send + 'static,
        R: Send + 'static,
    {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let ctx = self.clone().into();

        self.tokio_handle().spawn(async move {
            let result = (handler)(ctx).await;
            let _ = tx.send(result);
        });

        rx
    }
}

#[cfg(feature = "parallel")]
pub trait SpawnParallel<M: Model>:
    Into<ThreadContext<M>> + DispatchEffect<M> + ResourceAccess + 'static
{
    fn rayon_pool(&self) -> &RayonPool;
    fn spawn_parallel<H, R>(&self, handler: H) -> crossbeam_channel::Receiver<R>
    where
        H: FnOnce(ThreadContext<M>) -> R + Send + Sync + 'static,
        R: Send + 'static,
    {
        let (tx, rx) = crossbeam_channel::bounded(1);
        let ctx = self.clone().into();

        self.rayon_pool().spawn(move || {
            let result = (handler)(ctx);
            let _ = tx.send(result);
        });
        rx
    }
}

#[cfg(feature = "async")]
#[derive(Debug, Clone)]
pub struct TokioHandle(tokio::runtime::Handle);

#[cfg(feature = "async")]
impl Deref for TokioHandle {
    type Target = tokio::runtime::Handle;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[cfg(feature = "async")]
impl From<tokio::runtime::Handle> for TokioHandle {
    fn from(handle: tokio::runtime::Handle) -> Self {
        Self(handle)
    }
}

#[cfg(feature = "parallel")]
#[derive(Debug, Clone)]
pub struct RayonPool(Arc<rayon::ThreadPool>);

#[cfg(feature = "parallel")]
impl Deref for RayonPool {
    type Target = rayon::ThreadPool;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[cfg(feature = "parallel")]
impl From<rayon::ThreadPool> for RayonPool {
    fn from(pool: rayon::ThreadPool) -> Self {
        Self(Arc::new(pool))
    }
}
