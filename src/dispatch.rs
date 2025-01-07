use bon::Builder;
use tokio::sync::{mpsc, oneshot};

use crate::{
    context::{effect::EffectContext, Context},
    model::Model,
    syzygy::Syzygy,
};
use std::marker::PhantomData;

pub trait Effect<M: Model>: Send + Sync + 'static {
    fn execute(self: Box<Self>, cx: &mut EffectContext<M>);
}

impl<F, M: Model> Effect<M> for F
where
    F: FnOnce(&mut EffectContext<M>) + Send + Sync + 'static,
{
    fn execute(self: Box<Self>, cx: &mut EffectContext<M>) {
        (*self)(cx);
    }
}

#[derive(Debug)]
pub enum EffectStatus {
    Completed,
    Failed(Option<Box<dyn std::error::Error + Send + Sync>>),
}

#[derive(Default)]
pub struct CompletionNotifier(pub(crate) Option<oneshot::Sender<EffectStatus>>);

impl CompletionNotifier {
    pub fn send_status(self, status: EffectStatus) {
        if let Some(sender) = self.0 {
            let _ = sender.send(status);
        }
    }

    pub fn completed(self) {
        self.send_status(EffectStatus::Completed);
    }

    pub fn failed(self) {
        self.send_status(EffectStatus::Failed(None));
    }

    pub fn error(self, err: impl std::error::Error + Send + Sync + 'static) {
        self.send_status(EffectStatus::Failed(Some(Box::new(err))));
    }
}

type DispatchMsg<M> = (Box<dyn Effect<M>>, Option<oneshot::Sender<EffectStatus>>);

pub struct EffectSender<M: Model> {
    pub(crate) tx: mpsc::UnboundedSender<DispatchMsg<M>>,
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
    pub(crate) rx: mpsc::UnboundedReceiver<DispatchMsg<M>>,
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
    pub fn dispatch(&self, msg: DispatchMsg<M>) {
        self.tx
            .send(msg)
            .expect("Effect bus channel unexpectedly closed");
        if let Some(hook) = &self.effect_hook {
            (hook)();
        }
    }
}

impl<M: Model> EffectReceiver<M> {
    #[must_use]
    #[inline]
    pub(crate) fn next_effect(
        &mut self,
    ) -> Option<DispatchMsg<M>> {
        match self.rx.try_recv() {
            Ok(effect) => Some(effect),
            Err(_) => None,
        }
    }
}

pub trait DispatchEffect: Context {
    fn effect_sender(&self) -> &EffectSender<Self::Model>;
    #[inline]
    fn dispatch<E>(&self, effect: E)
    where
        E: FnOnce(&mut EffectContext<Self::Model>) + Send + Sync + 'static,
    {
        self.effect_sender().dispatch((Box::new(effect), None));
    }

    fn dispatch_awaitable<E>(&self, effect: E) -> oneshot::Receiver<EffectStatus>
    where
        E: FnOnce(&mut EffectContext<Self::Model>) + Send + Sync + 'static,
    {
        let (tx, rx) = oneshot::channel();
        self.effect_sender().dispatch((Box::new(effect), Some(tx)));
        rx
    }
}
