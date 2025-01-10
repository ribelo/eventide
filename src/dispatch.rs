use std::collections::VecDeque;
use std::marker::PhantomData;
use std::pin::Pin;

use bon::Builder;
use derive_more::derive::{Deref, DerefMut, From, IntoIterator};
use tokio::sync::mpsc;

use crate::prelude::ThreadContext;
use crate::{context::Context, model::Model, prelude::AsyncContext, syzygy::Syzygy};

pub type Cmd<M> = Box<dyn FnOnce(&mut Syzygy<M>) -> Effects<M> + Send + Sync + 'static>;

pub type Task<M> = Box<dyn FnOnce(ThreadContext<M>) -> Effects<M> + Send + Sync + 'static>;

pub type AsyncTask<M> = Box<
    dyn FnOnce(AsyncContext<M>) -> Pin<Box<dyn Future<Output = Effects<M>> + Send + 'static>>
        + Send
        + Sync
        + 'static,
>;

#[derive(From)]
pub enum Effect<M: Model> {
    Cmd(Cmd<M>),
    Task(Task<M>),
    AsyncTask(AsyncTask<M>),
}

impl<M: Model> std::fmt::Debug for Effect<M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cmd(_) => f.debug_tuple("Command").field(&"<cmd>").finish(),
            Self::Task(_) => f.debug_tuple("Task").field(&"<task>").finish(),
            Self::AsyncTask(_) => f.debug_tuple("AsyncTask").field(&"<async_task>").finish(),
        }
    }
}

#[derive(Debug, Builder, Deref, DerefMut, IntoIterator)]
pub struct Effects<M: Model> {
    pub(crate) items: VecDeque<Effect<M>>,
}

impl<M: Model> Default for Effects<M> {
    fn default() -> Self {
        Self {
            items: VecDeque::default(),
        }
    }
}

impl<M: Model> Effects<M> {
    #[must_use]
    pub fn none() -> Self {
        Self::default()
    }
    #[must_use]
    pub fn and_then_cmd<F>(mut self, cmd: F) -> Self
    where
        F: FnOnce(&mut Syzygy<M>) -> Effects<M> + Send + Sync + 'static,
    {
        let effect = Effect::Cmd(Box::new(cmd));
        self.items.push_back(effect);
        self
    }
    #[must_use]
    pub fn and_then_task<F>(mut self, task: F) -> Self
    where
        F: FnOnce(ThreadContext<M>) -> Effects<M> + Send + Sync + 'static,
    {
        let effect = Effect::Task(Box::new(task));
        self.items.push_back(effect);
        self
    }
    #[must_use]
    pub fn and_then_async_task<F, Fut>(mut self, async_task: F) -> Self
    where
        F: FnOnce(AsyncContext<M>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Effects<M>> + Send + 'static,
    {
        let async_task = Box::new(move |ctx| {
            Box::pin(async_task(ctx)) as Pin<Box<dyn Future<Output = Effects<M>> + Send>>
        });
        let effect = Effect::AsyncTask(Box::new(async_task));
        self.items.push_back(effect);
        self
    }
}

impl<M: Model> From<Vec<Effect<M>>> for Effects<M> {
    fn from(items: Vec<Effect<M>>) -> Self {
        Effects { items: VecDeque::from(items) }
    }
}

impl<M: Model> From<Effect<M>> for Effects<M> {
    fn from(effect: Effect<M>) -> Self {
        Effects { items: VecDeque::from([effect]) }
    }
}

impl<M: Model> From<Cmd<M>> for Effects<M> {
    fn from(cmd: Cmd<M>) -> Self {
        Effects { items: VecDeque::from([Effect::from(cmd)]) }
    }
}

impl<M: Model> From<Task<M>> for Effects<M> {
    fn from(task: Task<M>) -> Self {
        Effects { items: VecDeque::from([Effect::from(task)]) }
    }
}

impl<M: Model> From<AsyncTask<M>> for Effects<M> {
    fn from(async_task: AsyncTask<M>) -> Self {
        Effects { items: VecDeque::from([Effect::from(async_task)]) }
    }
}

pub struct EffectSender<M: Model> {
    pub(crate) tx: mpsc::UnboundedSender<Effects<M>>,
    pub(crate) effect_hook: Option<Box<dyn Fn() + Send + Sync>>,
    phantom: PhantomData<M>,
}

impl<M: Model> std::fmt::Debug for EffectSender<M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let hook_str = self.effect_hook.as_ref().map_or("None", |_| "<fn>");

        f.debug_struct("EffectSender")
            .field("tx", &self.tx)
            .field("effect_hook", &hook_str)
            .field("phantom", &self.phantom)
            .finish()
    }
}

impl<M: Model> Clone for EffectSender<M> {
    fn clone(&self) -> Self {
        Self {
            tx: self.tx.clone(),
            effect_hook: None,
            phantom: PhantomData,
        }
    }
}

pub struct EffectReceiver<M: Model> {
    pub(crate) rx: mpsc::UnboundedReceiver<Effects<M>>,
    phantom: PhantomData<M>,
}

impl<M: Model> std::fmt::Debug for EffectReceiver<M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EffectBus")
            .field("rx", &self.rx)
            .field("middlewares", &"<middlewares>")
            .field("phantom", &self.phantom)
            .finish()
    }
}

#[derive(Debug, Builder)]
pub struct EffectQueue<M: Model> {
    pub(crate) effect_sender: EffectSender<M>,
    pub(crate) effect_receiver: EffectReceiver<M>,
}

impl<M: Model> Default for EffectQueue<M> {
    fn default() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        let effect_sender = EffectSender {
            tx,
            effect_hook: None,
            phantom: PhantomData,
        };
        let effect_receiver = EffectReceiver {
            rx,
            // middlewares: None,
            phantom: PhantomData,
        };
        Self {
            effect_sender,
            effect_receiver,
        }
    }
}

impl<M: Model> EffectSender<M> {
    #[inline]
    pub fn dispatch(&self, msgs: Effects<M>) {
        self.tx
            .send(msgs)
            .expect("Effect bus channel unexpectedly closed");
        if let Some(hook) = &self.effect_hook {
            (hook)();
        }
    }
}

impl<M: Model> EffectReceiver<M> {
    #[must_use]
    #[inline]
    pub(crate) fn next_batch(&mut self) -> Option<Effects<M>> {
        match self.rx.try_recv() {
            Ok(effect) => Some(effect),
            Err(_) => None,
        }
    }
}

pub trait DispatchEffect: Context {
    fn effect_sender(&self) -> &EffectSender<Self::Model>;
    #[inline]
    fn dispatch<F>(&self, cmd: F)
    where
        F: FnOnce(&mut Syzygy<Self::Model>) -> Effects<Self::Model> + Send + Sync + 'static,
    {
        let effect = Effect::Cmd(Box::new(cmd));
        self.effect_sender().dispatch(Effects::from(effect));
    }
    fn task<F>(&self, task: F)
    where
        F: FnOnce(ThreadContext<Self::Model>) -> Effects<Self::Model> + Send + Sync + 'static,
    {
        let effect = Effect::Task(Box::new(task));
        self.effect_sender().dispatch(Effects::from(effect));
    }
    fn async_task<F, Fut>(&self, task: F)
    where
        F: FnOnce(AsyncContext<Self::Model>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Effects<Self::Model>> + Send + 'static,
    {
        let task = Box::new(move |ctx| {
            Box::pin(task(ctx)) as Pin<Box<dyn Future<Output = Effects<Self::Model>> + Send>>
        });
        let effect = Effect::AsyncTask(task);
        self.effect_sender().dispatch(Effects::from(effect));
    }
}
