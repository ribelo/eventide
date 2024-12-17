#[cfg(feature = "async")]
use std::future::Future;

#[cfg(feature = "async")]
use std::sync::Arc;

#[cfg(feature = "async")]
use crate::context::r#async::AsyncContext;
#[cfg(feature = "async")]
use crate::context::AsyncContextExecutor;
use crate::context::{thread::ThreadContext, Context, ContextExecutor, FromContext};

#[derive(Debug, thiserror::Error)]
#[error("Thread spawn failed")]
pub struct SpawnTaskError(#[from] std::io::Error);

pub trait SpawnThread: Context
where
    ThreadContext: FromContext<Self>,
{
    fn spawn<H, T, R>(&self, handler: H) -> crossbeam_channel::Receiver<R>
    where
        H: ContextExecutor<ThreadContext, T, R> + Send + Sync + 'static,
        R: Send + 'static,
    {
        let (tx, rx) = crossbeam_channel::bounded(1);
        let ctx = ThreadContext::from_context(self);
        std::thread::spawn(move || {
            let result = handler.call(&ctx);
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
pub trait SpawnAsync: Context
where
    AsyncContext: FromContext<Self>,
{
    fn tokio_rt(&self) -> Arc<tokio::runtime::Runtime>;
    fn spawn_task<H, T, Fut, R>(&self, handler: H) -> tokio::sync::oneshot::Receiver<R>
    where
        H: AsyncContextExecutor<AsyncContext, T, Fut, R> + Send + Sync + 'static,
        Fut: Future<Output = R> + Send + 'static,
        R: Send + 'static,
    {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let ctx = AsyncContext::from_context(self);

        self.tokio_rt().spawn(async move {
            let result = handler.call(&ctx).await;
            let _ = tx.send(result);
        });

        rx
    }
}

#[cfg(feature = "parallel")]
pub trait SpawnParallel: Context
where
    ThreadContext: FromContext<Self>,
{
    fn rayon_pool(&self) -> Arc<rayon::ThreadPool>;
    fn spawn_parallel<F, R>(&self, f: F) -> crossbeam_channel::Receiver<R>
    where
        F: FnOnce(ThreadContext) -> R + Send + Sync + 'static,
        R: Send + 'static,
    {
        let (tx, rx) = crossbeam_channel::bounded(1);
        let ctx = ThreadContext::from_context(self);

        self.rayon_pool().spawn(move || {
            let result = f(ctx);
            let _ = tx.send(result);
        });
        rx
    }
}
