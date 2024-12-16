#[cfg(feature = "async")]
use std::future::Future;

#[cfg(feature = "async")]
use std::sync::Arc;

#[cfg(feature = "async")]
use crate::context::r#async::AsyncContext;
use crate::context::{thread::ThreadContext, Context, FromContext};

#[derive(Debug, thiserror::Error)]
#[error("Thread spawn failed")]
pub struct SpawnTaskError(#[from] std::io::Error);

pub trait SpawnThread<'a>: Context
where
    ThreadContext<'a>: FromContext<Self>,
{
    fn spawn<F, R>(&self, f: F) -> crossbeam_channel::Receiver<R>
    where
        F: FnOnce(ThreadContext) -> R + Send + Sync + 'static,
        R: Send + 'static,
    {
        let (tx, rx) = crossbeam_channel::bounded(1);
        let ctx = ThreadContext::from_context(self);
        std::thread::spawn(move || {
            let result = f(ctx);
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
    fn spawn_task<F, Fut, R>(&self, f: F) -> tokio::sync::oneshot::Receiver<R>
    where
        F: FnOnce(AsyncContext) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = R> + Send + 'static,
        R: Send + 'static,
    {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let ctx = AsyncContext::from_context(self);

        self.tokio_rt().spawn(async {
            let result = f(ctx).await;
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
