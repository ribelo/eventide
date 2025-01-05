use bon::Builder;
use tokio::sync::{mpsc, oneshot};

use crate::{context::Context, model::Model, syzygy::Syzygy};
use std::marker::PhantomData;

pub trait Effect<M: Model>: Send + Sync + 'static {
    fn execute(self: Box<Self>, syzygy: &mut Syzygy<M>);
}

impl<F, M: Model> Effect<M> for F
where
    F: FnOnce(&mut Syzygy<M>) + Send + Sync + 'static,
{
    fn execute(self: Box<Self>, syzygy: &mut Syzygy<M>) {
        (*self)(syzygy);
    }
}

// pub trait Middleware<M: Model>: Send + Sync {
//     fn process(&mut self, effect: &mut E);
// }

// impl<M, E, F> Middleware<M, E> for F
// where
//     M: Model,
//     E: Effect<M>,
//     F: FnMut(&mut E) + Send + Sync,
// {
//     #[inline]
//     fn process(&mut self, effect: &mut E) {
//         self(effect);
//     }
// }

pub struct EffectSender<M: Model> {
    pub(crate) tx: mpsc::UnboundedSender<(Vec<Box<dyn Effect<M>>>, Option<oneshot::Sender<()>>)>,
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
    pub(crate) rx: mpsc::UnboundedReceiver<(Vec<Box<dyn Effect<M>>>, Option<oneshot::Sender<()>>)>,
    // pub(crate) middlewares: Option<Vec<Box<dyn Middleware<M, E>>>>,
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
    pub fn dispatch(
        &self,
        effects: Vec<Box<dyn Effect<M>>>,
        tx: Option<oneshot::Sender<()>>,
        rx: Option<oneshot::Receiver<()>>,
    ) -> Option<oneshot::Receiver<()>> {
        let effects = effects.into_iter().map(Into::into).collect();

        self.tx
            .send((effects, tx))
            .expect("Effect bus channel unexpectedly closed");
        if let Some(hook) = &self.effect_hook {
            (hook)();
        }
        rx.map(Into::into)
    }
}

impl<M: Model> EffectReceiver<M> {
    #[must_use]
    #[inline]
    pub(crate) fn next_effect(
        &mut self,
    ) -> Option<(Vec<Box<dyn Effect<M>>>, Option<oneshot::Sender<()>>)> {
        match self.rx.try_recv() {
            Ok(effect) => Some(effect),
            Err(_) => None,
        }
    }
}

pub trait SendEffect: Context {
    fn effect_sender(&self) -> &EffectSender<Self::Model>;
    #[inline]
    fn dispatch<E>(&self, effect: E)
    where
        E: FnOnce(&mut Syzygy<Self::Model>) + Send + Sync + 'static,
    {
        self.effect_sender()
            .dispatch(vec![Box::new(effect)], None, None);
    }

    #[inline]
    fn dispatch_awaitable<E>(&self, effect: E) -> oneshot::Receiver<()>
    where
        E: FnOnce(&mut Syzygy<Self::Model>) + Send + Sync + 'static,
    {
        let (tx, rx) = oneshot::channel();

        self.effect_sender()
            .dispatch(vec![Box::new(effect)], Some(tx), Some(rx))
            .expect("Effect completion failed")
    }

    #[inline]
    fn dispatch_many<E>(&self, effects: impl IntoIterator<Item = E>)
    where
        E: FnOnce(&mut Syzygy<Self::Model>) + Send + Sync + 'static,
    {
        let effects = effects
            .into_iter()
            .map(|e| Box::new(e) as Box<dyn Effect<Self::Model>>)
            .collect();
        self.effect_sender().dispatch(effects, None, None);
    }

    #[inline]
    fn dispatch_many_awaitable<E>(
        &self,
        effects: impl IntoIterator<Item = E>,
    ) -> oneshot::Receiver<()>
    where
        E: FnOnce(&mut Syzygy<Self::Model>) + Send + Sync + 'static,
    {
        let effects = effects
            .into_iter()
            .map(|e| Box::new(e) as Box<dyn Effect<Self::Model>>)
            .collect();
        let (tx, rx) = oneshot::channel();

        self.effect_sender()
            .dispatch(effects, Some(tx), Some(rx))
            .expect("Effect completion failed")
    }
}
