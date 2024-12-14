use std::{future::Future, pin::Pin, sync::Arc};

use crossbeam_channel::{Receiver, Sender};

#[cfg(feature = "async")]
use crate::context::r#async::AsyncContext;
use crate::context::thread::ThreadContext;

type TaskFn = Box<dyn FnOnce(ThreadContext) + Send + Sync>;

pub struct Task {
    f: TaskFn,
}

impl Task {
    pub fn handle(self, cx: ThreadContext) {
        (self.f)(cx);
    }
}

#[derive(Debug, Clone)]
pub struct TaskQueue {
    tx: Sender<Task>,
    rx: Receiver<Task>,
}

impl Default for TaskQueue {
    fn default() -> Self {
        let (tx, rx) = crossbeam_channel::unbounded();
        Self { tx, rx }
    }
}

#[derive(Debug, thiserror::Error)]
#[error("Thread spawn failed")]
pub struct SpawnTaskError(#[from] std::io::Error);

impl TaskQueue {
    pub fn push<F, R>(&self, f: F) -> crossbeam_channel::Receiver<R>
    where
        F: FnOnce(ThreadContext) -> R + Send + Sync + 'static,
        R: Send + 'static,
    {
        let (tx, rx) = crossbeam_channel::bounded(1);
        let task = Box::new(move |ctx| {
            std::thread::spawn(move || {
                let result = f(ctx);
                let _ = tx.send(result);
            });
        });
        let _ = self.tx.send(Task { f: task });
        rx
    }

    #[must_use]
    pub fn pop(&self) -> Option<Task> {
        self.rx.try_recv().ok()
    }
}

pub trait SpawnThread {
    fn task_queue(&self) -> &TaskQueue;
    fn spawn<F, R>(&self, f: F) -> crossbeam_channel::Receiver<R>
    where
        F: FnOnce(ThreadContext) -> R + Send + Sync + 'static,
        R: Send + 'static,
    {
        self.task_queue().push(f)
    }
}

#[cfg(feature = "async")]
type AsyncTaskFn =
    Box<dyn FnOnce(AsyncContext) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;

#[cfg(feature = "async")]
pub struct AsyncTask {
    f: AsyncTaskFn,
}

#[cfg(feature = "async")]
impl AsyncTask {
    pub async fn handle(self, cx: AsyncContext) {
        (self.f)(cx).await;
    }
}

#[cfg(feature = "async")]
#[derive(Debug, Clone)]
pub struct AsyncTaskQueue {
    tx: Sender<AsyncTask>,
    rx: Receiver<AsyncTask>,
}

#[cfg(feature = "async")]
impl Default for AsyncTaskQueue {
    fn default() -> Self {
        let (tx, rx) = crossbeam_channel::unbounded();
        Self { tx, rx }
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
impl AsyncTaskQueue {
    pub fn push<F, Fut, R>(&self, f: F) -> tokio::sync::oneshot::Receiver<R>
    where
        F: FnOnce(AsyncContext) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = R> + Send + 'static,
        R: Send + 'static,
    {
        let (tx, rx) = tokio::sync::oneshot::channel();

        let task = Box::new(move |ctx: AsyncContext| {
            Box::pin(async move {
                let result = f(ctx).await;
                let _ = tx.send(result);
            }) as Pin<Box<dyn Future<Output = ()> + Send>>
        });

        let _ = self.tx.send(AsyncTask { f: task });
        rx
    }

    #[must_use]
    pub(crate) fn pop(&self) -> Option<AsyncTask> {
        self.rx.try_recv().ok()
    }
}

#[cfg(feature = "async")]
pub trait SpawnAsync {
    fn async_task_queue(&self) -> &AsyncTaskQueue;
    fn task<F, Fut, R>(&self, f: F) -> tokio::sync::oneshot::Receiver<R>
    where
        F: FnOnce(AsyncContext) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = R> + Send + 'static,
        R: Send + 'static,
    {
        self.async_task_queue().push(f)
    }
}

#[cfg(feature = "parallel")]
#[derive(Debug, Clone)]
pub struct ParallelTaskQueue {
    pool: Arc<rayon::ThreadPool>,
    tx: Sender<Task>,
    rx: Receiver<Task>,
}

#[cfg(feature = "parallel")]
impl Default for ParallelTaskQueue {
    fn default() -> Self {
        let num_threads = std::thread::available_parallelism()
            .map(std::num::NonZero::get)
            .unwrap_or(4);
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(num_threads)
            .build()
            .unwrap();
        let (tx, rx) = crossbeam_channel::unbounded();
        Self {
            pool: Arc::new(pool),
            tx,
            rx,
        }
    }
}

#[cfg(feature = "parallel")]
impl ParallelTaskQueue {
    pub fn push<F, R>(&self, f: F) -> crossbeam_channel::Receiver<R>
    where
        F: FnOnce(ThreadContext) -> R + Send + Sync + 'static,
        R: Send + 'static,
    {
        let (tx, rx) = crossbeam_channel::bounded(1);
        let pool = Arc::clone(&self.pool);
        let task = Box::new(move |ctx| {
            let pool = pool;
            pool.spawn(move || {
                let result = f(ctx);
                let _ = tx.send(result);
            });
        });
        let _ = self.tx.send(Task { f: task });
        rx
    }

    #[must_use]
    pub fn pop(&self) -> Option<Task> {
        self.rx.try_recv().ok()
    }
}

#[cfg(feature = "parallel")]
pub trait SpawnParallel {
    fn parallel_task_queue(&self) -> &ParallelTaskQueue;
    fn spawn_parallel<F, R>(&self, f: F) -> crossbeam_channel::Receiver<R>
    where
        F: FnOnce(ThreadContext) -> R + Send + Sync + 'static,
        R: Send + 'static,
    {
        self.parallel_task_queue().push(f)
    }
}
