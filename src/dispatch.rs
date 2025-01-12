use std::collections::VecDeque;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use crossbeam_queue::SegQueue;
use derive_more::derive::{Deref, DerefMut, IntoIterator};
use tokio::sync::oneshot;

use crate::context::{Context, FromContext};
use crate::{model::Model, prelude::AsyncContext, syzygy::Syzygy};

pub trait EffectFn<M: Model>: FnOnce(&mut Syzygy<M>) -> Effects<M> + Send + Sync + 'static {}

impl<M, F> EffectFn<M> for F
where
    M: Model,
    F: FnOnce(&mut Syzygy<M>) -> Effects<M> + Send + Sync + 'static,
{
}

pub trait SpawnFn<M: Model, O>: FnOnce(AsyncContext<M>) -> O + Send + Sync + 'static {}

impl<M, O, F> SpawnFn<M, O> for F
where
    M: Model,
    F: FnOnce(AsyncContext<M>) -> O + Send + Sync + 'static,
{
}

#[async_trait]
pub trait TaskFn<M: Model, O: Send + Sync + 'static>: Send + Sync + 'static {
    async fn call(self: Box<Self>, cx: AsyncContext<M>) -> O;
}

#[async_trait]
impl<M, F, Fut, O> TaskFn<M, O> for F
where
    M: Model,
    F: FnOnce(AsyncContext<M>) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = O> + Send + 'static,
    O: Send + Sync + 'static,
{
    async fn call(self: Box<Self>, cx: AsyncContext<M>) -> O {
        (*self)(cx).await
    }
}

pub trait PerformFn<M: Model, O>: FnOnce(O) -> Effects<M> + Send + Sync + 'static {}

impl<M, O, F> PerformFn<M, O> for F
where
    M: Model,
    F: FnOnce(O) -> Effects<M> + Send + Sync + 'static,
{
}

pub struct ThreadTask<M: Model, O: Send + Sync + 'static> {
    inner: Option<Box<dyn SpawnFn<M, O>>>,
}

impl<M: Model, O: Send + Sync + 'static> ThreadTask<M, O> {
    pub fn new(f: impl SpawnFn<M, O>) -> Self {
        Self {
            inner: Some(Box::new(f)),
        }
    }
    pub fn and_then<F, T>(self, f: F) -> ThreadTask<M, T>
    where
        F: FnOnce(O) -> T + Send + Sync + 'static,
        T: Send + Sync + 'static,
    {
        let inner = self.inner.map(|task| {
            let f = Box::new(f);
            Box::new(move |ctx: AsyncContext<M>| {
                let result = (task)(ctx);
                (*f)(result)
            }) as Box<dyn SpawnFn<M, T>>
        });

        ThreadTask { inner }
    }
}

impl<M: Model, O: Send + Sync + 'static> ThreadTask<M, O> {
    pub fn perform(mut self, f: impl PerformFn<M, O>) -> impl EffectFn<M> {
        move |ctx: &mut Syzygy<M>| {
            let async_ctx = AsyncContext::from_context(ctx);
            let queue = async_ctx.effects_queue.clone();
            let task = self.inner.take().unwrap();
            std::thread::spawn(move || {
                let result = (task)(async_ctx);
                let effects = (f)(result);
                queue.lock().unwrap().push_back(effects);
            });
            Effects::none()
        }
    }
}

pub struct AsyncTask<M: Model, O: Send + Sync + 'static> {
    inner: Option<Box<dyn TaskFn<M, O>>>,
}

impl<M: Model, O: Send + Sync + 'static> AsyncTask<M, O> {
    pub fn new(f: impl TaskFn<M, O>) -> Self {
        Self {
            inner: Some(Box::new(f)),
        }
    }

    pub fn and_then<F, Fut, T>(self, f: F) -> AsyncTask<M, T>
    where
        F: FnOnce(O) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = T> + Send + 'static,
        T: Send + Sync + 'static,
    {
        let inner = self.inner.map(|task| {
            let f = Box::new(f);
            Box::new(move |ctx: AsyncContext<M>| {
                let fut = async move {
                    let result = task.call(ctx).await;
                    (*f)(result).await
                };
                Box::pin(fut) as Pin<Box<dyn Future<Output = T> + Send>>
            }) as Box<dyn TaskFn<M, T>>
        });

        AsyncTask { inner }
    }
}

impl<M: Model, O: Send + Sync + 'static> AsyncTask<M, O> {
    pub fn perform(mut self, f: impl PerformFn<M, O>) -> impl EffectFn<M> {
        move |ctx: &mut Syzygy<M>| {
            let async_ctx = AsyncContext::from_context(ctx);
            let queue = async_ctx.effects_queue.clone();
            let task = self.inner.take().unwrap();
            tokio::spawn(async move {
                let result = task.call(async_ctx).await;
                let effects = (f)(result);
                queue.lock().unwrap().push_back(effects);
            });
            Effects::none()
        }
    }
}

#[derive(Deref, DerefMut, IntoIterator)]
pub struct Effects<M: Model> {
    pub(crate) items: Vec<Box<dyn EffectFn<M>>>,
}

impl<E: EffectFn<M>, M: Model> From<E> for Effects<M> {
    fn from(effect: E) -> Self {
        let mut effects = Effects::default();
        effects.push(Box::new(effect));
        effects
    }
}

impl<M: Model> Default for Effects<M> {
    fn default() -> Self {
        Self {
            items: Vec::default(),
        }
    }
}

impl<M: Model> Effects<M> {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            items: Vec::with_capacity(capacity),
        }
    }

    #[must_use]
    pub fn none() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn effect<F>(mut self, effect: F) -> Self
    where
        F: EffectFn<M>,
    {
        self.items.push(Box::new(effect));
        self
    }

    #[must_use]
    pub fn spawn<F, O, P>(mut self, task: F, perf: P) -> Self
    where
        F: SpawnFn<M, O>,
        O: Send + Sync + 'static,
        P: PerformFn<M, O>,
    {
        self.items
            .push(Box::new(ThreadTask::new(task).perform(perf)));
        self
    }

    #[must_use]
    pub fn task<F, Fut, O>(self, task: F) -> UnfinishedAsyncEffects<M, O>
    where
        F: FnOnce(AsyncContext<M>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = O> + Send + 'static,
        O: Send + Sync + 'static,
    {
        UnfinishedAsyncEffects {
            task: AsyncTask::new(task),
            items: self.items,
        }
    }
}

pub struct UnfinishedAsyncEffects<M: Model, O: Send + Sync + 'static> {
    task: AsyncTask<M, O>,
    items: Vec<Box<dyn EffectFn<M>>>,
}

impl<M: Model, O: Send + Sync + 'static> UnfinishedAsyncEffects<M, O> {
    #[must_use]
    pub fn perform(mut self, f: impl PerformFn<M, O>) -> Effects<M> {
        let mut effects = Effects { items: self.items };
        effects.items.push(Box::new(move |ctx: &mut Syzygy<M>| {
            let async_ctx = AsyncContext::from_context(ctx);
            let queue = async_ctx.effects_queue.clone();
            let task = self.task.inner.take().unwrap();
            tokio::spawn(async move {
                let result = task.call(async_ctx).await;
                let effects = (f)(result);
                queue.lock().unwrap().push_back(effects);
            });
            Effects::none()
        }));
        effects
    }

    #[must_use]
    pub fn done(mut self) -> Effects<M> {
        let mut effects = Effects { items: self.items };
        effects.items.push(Box::new(move |ctx: &mut Syzygy<M>| {
            let async_ctx = AsyncContext::from_context(ctx);
            let queue = async_ctx.effects_queue.clone();
            let task = self.task.inner.take().unwrap();
            tokio::spawn(async move {
                let _ = task.call(async_ctx).await;
                queue.lock().unwrap().push_back(Effects::none());
            });
            Effects::none()
        }));
        effects
    }
}

pub struct UnfinishedThreadEffects<M: Model, O: Send + Sync + 'static> {
    task: ThreadTask<M, O>,
    items: Vec<Box<dyn EffectFn<M>>>,
}

impl<M: Model, O: Send + Sync + 'static> UnfinishedThreadEffects<M, O> {
    pub fn perform(mut self, f: impl PerformFn<M, O>) -> Effects<M> {
        let mut effects = Effects { items: self.items };
        effects.items.push(Box::new(move |ctx: &mut Syzygy<M>| {
            let async_ctx = AsyncContext::from_context(ctx);
            let queue = async_ctx.effects_queue.clone();
            let task = self.task.inner.take().unwrap();
            std::thread::spawn(move || {
                let result = (task)(async_ctx);
                let effects = (f)(result);
                queue.lock().unwrap().push_back(effects);
            });
            Effects::none()
        }));
        effects
    }
}

#[derive(Deref, DerefMut, IntoIterator)]
pub struct EffectsQueue<M: Model> {
    queue: Arc<Mutex<VecDeque<Effects<M>>>>,
}

impl<M: Model> std::fmt::Debug for EffectsQueue<M> where M: Model {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EffectsQueue")
            .field("queue", &"<queue>")
            .finish()
    }
}

impl<M: Model> Clone for EffectsQueue<M> {
    fn clone(&self) -> Self {
        Self {
            queue: Arc::clone(&self.queue)
        }
    }
}

impl<M: Model> Default for EffectsQueue<M> {
    fn default() -> Self {
        Self {
            queue: Arc::new(Mutex::new(VecDeque::with_capacity(1024))),
        }
    }
}

impl<M: Model> EffectsQueue<M> {
    #[must_use]
    #[inline]
    pub(crate) fn next_batch(&mut self) -> Option<Effects<M>> {
        self.lock().unwrap().pop_front()
    }
}

pub trait DispatchEffect: Context {
    fn effects_queue(&self) -> &EffectsQueue<Self::Model>;
    #[inline]
    fn trigger<F>(&self, effect: F)
    where
        F: FnOnce(&mut Syzygy<Self::Model>) + Send + Sync + 'static,
    {
        let mut effects = Effects::with_capacity(1);
        effects.push(Box::new(move |ctx: &mut Syzygy<Self::Model>| {
            effect(ctx);
            Effects::none()
        }));
        self.effects_queue().lock().unwrap().push_back(effects);
    }

    #[must_use]
    #[inline]
    fn dispatch(&self, effects: impl Into<Effects<Self::Model>>) {
        self.effects_queue().lock().unwrap().push_back(effects.into());
    }

    #[must_use]
    #[inline]
    fn dispatch_sync(&self, effect: impl EffectFn<Self::Model>) -> oneshot::Receiver<()> {
        let (tx, rx) = oneshot::channel();
        let mut effects = Effects::with_capacity(1);
        let wrapped_effect = move |ctx: &mut Syzygy<Self::Model>| {
            let result = (effect)(ctx);
            let _ = tx.send(());
            result
        };
        effects.push(Box::new(wrapped_effect));
        self.effects_queue().lock().unwrap().push_back(effects);
        rx
    }
}
