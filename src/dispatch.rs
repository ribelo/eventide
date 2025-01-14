use std::cell::RefCell;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use arrayvec::ArrayVec;
use async_trait::async_trait;
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

pub trait PerformFn<M: Model, I>: FnOnce(I) -> Effects<M> + Send + Sync + 'static {}

impl<F, M, I> PerformFn<M, I> for F
where
    M: Model,
    F: FnOnce(I) -> Effects<M> + Send + Sync + 'static,
{
}

pub trait SyncAndThenFn<I, O>: FnOnce(I) -> O + Send + Sync + 'static {}

impl<F, I, O> SyncAndThenFn<I, O> for F where F: FnOnce(I) -> O + Send + Sync + 'static {}

#[async_trait]
pub trait AsyncAndThenFn<I, O: Send + Sync + 'static>: Send + Sync + 'static {
    async fn call(self: Box<Self>, input: I) -> O;
}

#[async_trait]
impl<F, I, Fut, O> AsyncAndThenFn<I, O> for F
where
    F: FnOnce(I) -> Fut + Send + Sync + 'static,
    I: Send + 'static,
    Fut: Future<Output = O> + Send + Sync + 'static,
    O: Send + Sync + 'static,
{
    async fn call(self: Box<Self>, input: I) -> O {
        (*self)(input).await
    }
}

pub trait SyncProcessFn<M: Model>:
    FnOnce(AsyncContext<M>, crossbeam_channel::Sender<Effects<M>>) + Send + Sync + 'static
{
}

impl<M, F> SyncProcessFn<M> for F
where
    M: Model,
    F: FnOnce(AsyncContext<M>, crossbeam_channel::Sender<Effects<M>>) + Send + Sync + 'static,
{
}

#[async_trait]
pub trait AsyncProcessFn<M: Model>: Send + Sync + 'static {
    async fn call(self: Box<Self>, cx: AsyncContext<M>, tx: crossbeam_channel::Sender<Effects<M>>);
}

#[async_trait]
impl<M, F, Fut> AsyncProcessFn<M> for F
where
    M: Model,
    F: FnOnce(AsyncContext<M>, crossbeam_channel::Sender<Effects<M>>) -> Fut
        + Send
        + Sync
        + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    async fn call(self: Box<Self>, cx: AsyncContext<M>, tx: crossbeam_channel::Sender<Effects<M>>) {
        (*self)(cx, tx).await
    }
}

pub fn recurent_effects<M: Model>(rx: crossbeam_channel::Receiver<Effects<M>>) -> Effects<M> {
    match rx.try_recv() {
        Ok(effects) => effects,
        Err(crossbeam_channel::TryRecvError::Empty) => {
            Effects::none().effect(move |_| recurent_effects(rx))
        }
        Err(crossbeam_channel::TryRecvError::Disconnected) => Effects::none(),
    }
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
    pub fn and_then<T>(self, f: impl SyncAndThenFn<O, T>) -> ThreadTask<M, T>
    where
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
            let task = self.inner.take().unwrap();
            let (tx, rx) = crossbeam_channel::bounded(1);
            std::thread::spawn(move || {
                let result = (task)(async_ctx);
                let effects = (f)(result);
                let _ = tx.send(effects);
            });
            recurent_effects(rx)
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

    pub fn and_then<T>(self, f: impl AsyncAndThenFn<O, T>) -> AsyncTask<M, T>
    where
        T: Send + Sync + 'static,
    {
        let inner = self.inner.map(|task| {
            let f = Box::new(f);
            Box::new(move |ctx: AsyncContext<M>| {
                let fut = async move {
                    let result = task.call(ctx).await;
                    f.call(result).await
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
            let task = self.inner.take().unwrap();
            let (tx, rx) = crossbeam_channel::bounded(1);
            tokio::spawn(async move {
                let result = task.call(async_ctx).await;
                let effects = (f)(result);
                let _ = tx.send(effects);
            });
            recurent_effects(rx)
        }
    }
}

#[derive(Deref, DerefMut, IntoIterator)]
pub struct Effects<M: Model> {
    pub(crate) items: ArrayVec<Box<dyn EffectFn<M>>, 16>,
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
            items: ArrayVec::new(),
        }
    }
}

impl<M: Model> Effects<M> {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    // #[must_use]
    // pub fn with_capacity(capacity: usize) -> Self {
    //     Self {
    //         items: ArrayVec::with_capacity(capacity),
    //     }
    // }

    #[must_use]
    pub fn none() -> Self {
        Self::default()
    }

    pub fn push_effect<F>(&mut self, effect_fn: F)
    where
        F: EffectFn<M>,
    {
        self.items.push(Box::new(effect_fn));
    }

    #[must_use]
    pub fn effect<F>(mut self, f: F) -> Self
    where
        F: EffectFn<M>,
    {
        self.push_effect(f);
        self
    }

    pub fn push_spawn<O>(&mut self, task: impl SpawnFn<M, O>, perf: impl PerformFn<M, O>)
    where
        O: Send + Sync + 'static,
    {
        self.items
            .push(Box::new(ThreadTask::new(task).perform(perf)));
    }

    #[must_use]
    pub fn spawn<F, O>(self, task: F) -> UnfinishedThreadEffects<M, O>
    where
        F: SpawnFn<M, O>,
        O: Send + Sync + 'static,
    {
        UnfinishedThreadEffects {
            task: ThreadTask::new(task),
            items: self.items,
        }
    }

    pub fn push_task<O>(&mut self, task: impl TaskFn<M, O>, perf: impl PerformFn<M, O>)
    where
        O: Send + Sync + 'static,
    {
        self.items
            .push(Box::new(AsyncTask::new(task).perform(perf)));
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

#[derive(Deref)]
pub struct UnfinishedAsyncEffects<M: Model, O: Send + Sync + 'static> {
    task: AsyncTask<M, O>,
    #[deref]
    items: ArrayVec<Box<dyn EffectFn<M>>, 16>,
}

impl<M: Model, O: Send + Sync + 'static> UnfinishedAsyncEffects<M, O> {
    #[must_use]
    pub fn and_then<F, Fut, T>(self, f: impl AsyncAndThenFn<O, T>) -> UnfinishedAsyncEffects<M, T>
    where
        T: Send + Sync + 'static,
    {
        let task = self.task.and_then(f);
        UnfinishedAsyncEffects {
            task,
            items: self.items,
        }
    }

    #[must_use]
    pub fn perform(self, f: impl PerformFn<M, O>) -> Effects<M> {
        let mut effects = Effects { items: self.items };
        effects.push(Box::new(self.task.perform(f)));
        effects
    }

    #[must_use]
    pub fn done(mut self) -> Effects<M> {
        let mut effects = Effects { items: self.items };
        effects.items.push(Box::new(move |ctx: &mut Syzygy<M>| {
            let async_ctx = AsyncContext::from_context(ctx);
            let task = self.task.inner.take().unwrap();
            tokio::spawn(async move {
                let _ = task.call(async_ctx).await;
            });
            Effects::none()
        }));
        effects
    }
}

pub struct UnfinishedThreadEffects<M: Model, O: Send + Sync + 'static> {
    task: ThreadTask<M, O>,
    items: ArrayVec<Box<dyn EffectFn<M>>, 16>,
}

impl<M: Model, O: Send + Sync + 'static> UnfinishedThreadEffects<M, O> {
    pub fn perform(mut self, f: impl PerformFn<M, O>) -> Effects<M> {
        let mut effects = Effects { items: self.items };
        effects.items.push(Box::new(move |ctx: &mut Syzygy<M>| {
            let async_ctx = AsyncContext::from_context(ctx);
            let task = self.task.inner.take().unwrap();
            let (tx, rx) = crossbeam_channel::bounded(1);
            std::thread::spawn(move || {
                let result = (task)(async_ctx);
                let effects = (f)(result);
                let _ = tx.send(effects);
            });
            recurent_effects(rx)
        }));
        effects
    }
}

pub struct EffectsIter<M: Model, I>
where
    I: Iterator<Item = Box<dyn EffectFn<M>>>,
{
    inner: I,
}

#[derive(Deref, DerefMut, IntoIterator)]
pub struct EffectsQueue<M: Model> {
    queue: RefCell<Vec<Box<dyn EffectFn<M>>>>,
}

impl<M: Model> std::fmt::Debug for EffectsQueue<M>
where
    M: Model,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EffectsQueue")
            .field("queue", &"<queue>")
            .finish()
    }
}

impl<M: Model> Default for EffectsQueue<M> {
    fn default() -> Self {
        Self {
            queue: RefCell::new(Vec::with_capacity(32)),
        }
    }
}

impl<M: Model> EffectsQueue<M> {
    pub fn replace(&mut self, effects: Vec<Box<dyn EffectFn<M>>>) {
        let mut queue = self.queue.borrow_mut();
        queue.clear();
        queue.extend(effects);
    }
}

pub trait DispatchEffect: Context {
    fn effects_queue(&self) -> &EffectsQueue<Self::Model>;
    #[inline]
    fn trigger<F>(&self, effect: F)
    where
        F: FnOnce(&mut Syzygy<Self::Model>) + Send + Sync + 'static,
    {
        let mut effects = Effects::new();
        effects.push(Box::new(move |ctx: &mut Syzygy<Self::Model>| {
            effect(ctx);
            Effects::none()
        }));
        self.effects_queue().borrow_mut().extend(effects);
    }

    #[inline]
    fn dispatch(&self, effects: impl Into<Effects<Self::Model>>) {
        let effects = effects.into();
        self.effects_queue().borrow_mut().extend(effects);
    }

    #[must_use]
    #[inline]
    fn dispatch_sync(&self, effect: impl EffectFn<Self::Model>) -> oneshot::Receiver<()> {
        let (tx, rx) = oneshot::channel();
        let mut effects = Effects::new();
        let wrapped_effect = move |ctx: &mut Syzygy<Self::Model>| {
            let result = (effect)(ctx);
            let _ = tx.send(());
            result
        };
        effects.push(Box::new(wrapped_effect));
        self.effects_queue().borrow_mut().extend(effects);
        rx
    }

    // #[inline]
    // fn dispatch_task<O>(&self, effects: impl Into<AsyncTask<Self::Model, O>>)
    // where
    //     O: Send + Sync + 'static
    // {
    //     let effects = effects.into();
    //     self.effects_queue().borrow_mut().extend(effects);
    // }
}
