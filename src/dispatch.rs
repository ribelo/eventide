use std::marker::PhantomData;
use std::{fmt, pin::Pin};

use async_trait::async_trait;
use bon::Builder;
use derive_more::Deref;
use tokio::sync::mpsc;

use crate::{context::Context, model::Model, prelude::AsyncContext, syzygy::Syzygy};

pub type Task<C> = Box<dyn FnOnce() -> Effects<C> + Send + Sync + 'static>;
pub type AsyncTask<C> = Box<
    dyn FnOnce() -> Pin<Box<dyn Future<Output = Effects<C>> + Send + 'static>>
        + Send
        + Sync
        + 'static,
>;

pub trait Command: fmt::Debug + Clone + Send + Sync + 'static {
    type Model: Model;
    fn execute(&self, cx: &mut Syzygy<Self::Model, Self>) -> impl IntoEffects<Command = Self>;
}

pub enum Effect<C: Command> {
    Cmd(C),
    Task(Task<C>),
    AsyncTask(AsyncTask<C>),
}

impl<C: Command> std::fmt::Debug for Effect<C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Effect::Cmd(cmd) => f.debug_tuple("Cmd").field(cmd).finish(),
            Effect::Task(_) => f.debug_tuple("Task").field(&"<task>").finish(),
            Effect::AsyncTask(_) => f.debug_tuple("AsyncTask").field(&"<async_task>").finish(),
        }
    }
}

#[derive(Debug, Builder)]
pub struct Effects<C: Command> {
    pub(crate) items: Vec<Effect<C>>,
}

impl<C: Command> Default for Effects<C> {
    fn default() -> Self {
        Self {
            items: Vec::default(),
        }
    }
}

impl<C: Command> Effects<C> {
    #[must_use]
    pub fn none() -> Self {
        Self::default()
    }
    #[must_use]
    pub fn and_then<T>(self, effects: T) -> Self
    where
        T: IntoEffects<Command = C>,
    {
        let mut items = self.items;
        items.extend(effects.into_effects().items);
        Self { items }
    }
    #[must_use]
    pub fn and_then_cmd(self, cmd: C) -> Self {
        let mut items = self.items;
        items.push(Effect::Cmd(cmd));
        Self { items }
    }
    #[must_use]
    pub fn and_then_task<F>(self, task: F) -> Self
    where
        F: FnOnce() -> Effects<C> + Send + Sync + 'static,
    {
        let mut items = self.items;
        items.push(Effect::Task(Box::new(task)));
        Self { items }
    }
    #[must_use]
    pub fn and_then_async_task<F, Fut>(self, task: F) -> Self
    where
        F: FnOnce() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Effects<C>> + Send + 'static,
    {
        let task = Box::new(move || {
            Box::pin(task()) as Pin<Box<dyn Future<Output = Effects<C>> + Send>>
        });
        let effect = Effect::AsyncTask(task);
        let effects = Effects {
            items: vec![effect],
        };
        self.and_then(effects)
    }
}

pub trait IntoEffects {
    type Command: Command;
    fn into_effects(self) -> Effects<Self::Command>;
}

impl<E: Command> IntoEffects for Effects<E> {
    type Command = E;
    fn into_effects(self) -> Effects<E> {
        self
    }
}

impl<E: Command> IntoEffects for E {
    type Command = E;
    fn into_effects(self) -> Effects<E> {
        Effects {
            items: vec![Effect::Cmd(self)],
        }
    }
}

impl<E: Command> IntoEffects for Vec<E> {
    type Command = E;
    fn into_effects(self) -> Effects<E> {
        Effects {
            items: self.into_iter().map(Effect::Cmd).collect(),
        }
    }
}

pub struct EffectSender<M: Model, E: Command> {
    pub(crate) tx: mpsc::UnboundedSender<Effects<E>>,
    pub(crate) effect_hook: Option<Box<dyn Fn() + Send + Sync>>,
    phantom: PhantomData<M>,
}

impl<M: Model, E: Command> std::fmt::Debug for EffectSender<M, E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let hook_str = self.effect_hook.as_ref().map_or("None", |_| "<fn>");

        f.debug_struct("EffectSender")
            .field("tx", &self.tx)
            .field("effect_hook", &hook_str)
            .field("phantom", &self.phantom)
            .finish()
    }
}

impl<M: Model, E: Command> Clone for EffectSender<M, E> {
    fn clone(&self) -> Self {
        Self {
            tx: self.tx.clone(),
            effect_hook: None,
            phantom: PhantomData,
        }
    }
}

pub struct EffectReceiver<M: Model, E: Command> {
    pub(crate) rx: mpsc::UnboundedReceiver<Effects<E>>,
    phantom: PhantomData<M>,
}

impl<M: Model, E: Command> std::fmt::Debug for EffectReceiver<M, E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EffectBus")
            .field("rx", &self.rx)
            .field("middlewares", &"<middlewares>")
            .field("phantom", &self.phantom)
            .finish()
    }
}

#[derive(Debug, Builder)]
pub struct EffectQueue<M: Model, E: Command> {
    pub(crate) effect_sender: EffectSender<M, E>,
    pub(crate) effect_receiver: EffectReceiver<M, E>,
}

impl<M: Model, E: Command> Default for EffectQueue<M, E> {
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

impl<M: Model, E: Command> EffectSender<M, E> {
    #[inline]
    pub fn dispatch(&self, msgs: Effects<E>) {
        self.tx
            .send(msgs)
            .expect("Effect bus channel unexpectedly closed");
        if let Some(hook) = &self.effect_hook {
            (hook)();
        }
    }
}

impl<M: Model, E: Command> EffectReceiver<M, E> {
    #[must_use]
    #[inline]
    pub(crate) fn next_effect(&mut self) -> Option<Effects<E>> {
        match self.rx.try_recv() {
            Ok(effect) => Some(effect),
            Err(_) => None,
        }
    }
}

pub trait DispatchEffect: Context {
    fn effect_sender(&self) -> &EffectSender<Self::Model, Self::Command>;
    #[inline]
    fn dispatch(&self, effect: impl IntoEffects<Command = Self::Command>) {
        self.effect_sender().dispatch(effect.into_effects());
    }
    fn task<F>(&self, task: F)
    where
        F: FnOnce() -> Effects<Self::Command> + Send + Sync + 'static,
    {
        let effect = Effect::Task(Box::new(task));
        let effects = Effects {
            items: vec![effect],
        };
        self.effect_sender().dispatch(effects);
    }
    fn async_task<F, Fut>(&self, task: F)
    where
        F: FnOnce() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Effects<Self::Command>> + Send + 'static,
    {
        let task = Box::new(move || {
            Box::pin(task()) as Pin<Box<dyn Future<Output = Effects<Self::Command>> + Send>>
        });
        let effect = Effect::AsyncTask(task);
        let effects = Effects {
            items: vec![effect],
        };
        self.effect_sender().dispatch(effects);
    }
}
