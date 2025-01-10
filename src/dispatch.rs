use std::collections::VecDeque;
use std::fmt;
use std::marker::PhantomData;
use std::pin::Pin;

use async_trait::async_trait;
use bon::Builder;
use derive_more::derive::{Deref, DerefMut, From, IntoIterator};
use tokio::sync::{mpsc, oneshot};

use crate::prelude::ThreadContext;
use crate::{context::Context, model::Model, prelude::AsyncContext, syzygy::Syzygy};

pub trait Command<M: Model>: fmt::Debug + Send + Sync + 'static {
    fn run(self: Box<Self>, syzygy: &mut Syzygy<M>) -> Messages<M>;
}

pub trait Task<M: Model>: fmt::Debug + Send + Sync + 'static {
    fn run(self: Box<Self>, ctx: ThreadContext<M>) -> Messages<M>;
}

#[async_trait]
pub trait AsyncTask<M: Model>: fmt::Debug + Send + Sync + 'static {
    async fn run(self: Box<Self>, ctx: AsyncContext<M>) -> Messages<M>;
}

#[derive(Debug)]
struct SyncCommand<M: Model + std::fmt::Debug> {
    tx: oneshot::Sender<()>,
    phantom: PhantomData<M>,
}

impl<M: Model> SyncCommand<M> {
    fn new(tx: oneshot::Sender<()>) -> Self {
        Self {
            tx,
            phantom: PhantomData,
        }
    }
}

impl<M: Model + std::fmt::Debug> Command<M> for SyncCommand<M> {
    fn run(self: Box<Self>, _syzygy: &mut Syzygy<M>) -> Messages<M> {
        let _ = self.tx.send(());
        Messages::none()
    }
}

#[derive(Debug, From)]
pub enum Message<M: Model> {
    Cmd(Box<dyn Command<M>>),
    Task(Box<dyn Task<M>>),
    AsyncTask(Box<dyn AsyncTask<M>>),
}

#[derive(Debug, Builder, Deref, DerefMut, IntoIterator)]
pub struct Messages<M: Model> {
    pub(crate) items: VecDeque<Message<M>>,
}

impl<M: Model> Default for Messages<M> {
    fn default() -> Self {
        Self {
            items: VecDeque::default(),
        }
    }
}

impl<M: Model> Messages<M> {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    pub fn none() -> Self {
        Self::default()
    }
    #[must_use]
    pub fn and_then_cmd(mut self, cmd: impl Command<M>) -> Self {
        let effect = Message::Cmd(Box::new(cmd));
        self.items.push_back(effect);
        self
    }
    #[must_use]
    pub fn and_then_task(mut self, task: impl Task<M>) -> Self {
        let effect = Message::Task(Box::new(task));
        self.items.push_back(effect);
        self
    }
    #[must_use]
    pub fn and_then_async_task(mut self, async_task: impl AsyncTask<M>) -> Self {
        let effect = Message::AsyncTask(Box::new(async_task));
        self.items.push_back(effect);
        self
    }
}

impl<M: Model> From<Message<M>> for Messages<M> {
    fn from(effect: Message<M>) -> Self {
        Messages {
            items: VecDeque::from([effect]),
        }
    }
}

pub struct EffectSender<M: Model> {
    pub(crate) tx: mpsc::UnboundedSender<Messages<M>>,
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
    pub(crate) rx: mpsc::UnboundedReceiver<Messages<M>>,
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
    pub fn dispatch(&self, msgs: Messages<M>) {
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
    pub(crate) fn next_batch(&mut self) -> Option<Messages<M>> {
        match self.rx.try_recv() {
            Ok(effect) => Some(effect),
            Err(_) => None,
        }
    }
}

pub trait DispatchEffect: Context {
    fn effect_sender(&self) -> &EffectSender<Self::Model>;
    #[inline]
    fn send(&self, message: impl Into<Messages<Self::Model>>) {
        self.effect_sender().dispatch(message.into());
    }
    fn dispatch(&self, cmd: impl Command<Self::Model>) {
        let message = Message::Cmd(Box::new(cmd));
        self.send(message);
    }
    fn dispatch_sync(&self, cmd: impl Command<Self::Model>) -> oneshot::Receiver<()> {
        let (tx, rx) = oneshot::channel();
        let messages = Messages::new()
            .and_then_cmd(cmd)
            .and_then_cmd(SyncCommand::new(tx));
        self.send(messages);
        rx
    }
    fn task(&self, task: impl Task<Self::Model>) {
        let message = Message::Task(Box::new(task));
        self.send(message);
    }
    fn async_task(&self, async_task: impl AsyncTask<Self::Model>) {
        let message = Message::AsyncTask(Box::new(async_task));
        self.send(message);
    }
}
