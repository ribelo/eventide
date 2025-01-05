use std::future::Future;
#[cfg(feature = "parallel")]
use std::sync::Arc;

use std::ops::Deref;

use tokio::sync::oneshot;

use crate::context::r#async::AsyncContext;
use crate::context::thread::ThreadContext;
use crate::context::{Context, FromContext};
use crate::effects::Effect;
use crate::model::{Model, ModelAccess};
use crate::{effects::SendEffect, resource::ResourceAccess};

#[derive(Debug, thiserror::Error)]
#[error("Thread spawn failed")]
pub struct SpawnTaskError(#[from] std::io::Error);

pub trait SpawnThread: ModelAccess + ResourceAccess + SendEffect {
    fn spawn<H, R>(&self, handler: H) -> oneshot::Receiver<R>
    where
        H: FnOnce(ThreadContext<Self::Model, Self::Effect>) -> R + Send + Sync + 'static,
        R: Send + 'static,
    {
        let (tx, rx) = oneshot::channel();
        let ctx = ThreadContext::from_context(self);
        std::thread::spawn(move || {
            let result = (handler)(ctx);
            let _ = tx.send(result);
        });

        rx
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AsyncTaskError {
    #[error("Task spawn failed")]
    SpawnError,
    #[error("Channel send failed")]
    SendError,
    #[error("Channel receive failed")]
    ReceiveError,
}

pub trait SpawnAsync: ModelAccess + ResourceAccess + SendEffect {
    fn tokio_handle(&self) -> &TokioHandle;
    fn spawn_task<H, Fut, R>(&self, handler: H) -> tokio::sync::oneshot::Receiver<R>
    where
        H: FnOnce(AsyncContext<Self::Model, Self::Effect>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = R> + Send + 'static,
        R: Send + 'static,
    {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let ctx = AsyncContext::from_context(self);

        self.tokio_handle().spawn(async move {
            let result = (handler)(ctx).await;
            let _ = tx.send(result);
        });

        rx
    }
}

#[cfg(feature = "parallel")]
pub trait SpawnParallel<M: Model, E: Effect<M>>:
    Into<ThreadContext<M, E>> + SendEffect<M, E> + ResourceAccess + 'static
{
    fn rayon_pool(&self) -> &RayonPool;
    fn spawn_parallel<H, R>(&self, handler: H) -> crossbeam_channel::Receiver<R>
    where
        H: FnOnce(ThreadContext<M, E>) -> R + Send + Sync + 'static,
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

#[derive(Debug, Clone)]
pub struct TokioHandle(tokio::runtime::Handle);

impl Deref for TokioHandle {
    type Target = tokio::runtime::Handle;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

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
