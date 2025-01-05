use bon::Builder;
use tokio::sync::{mpsc, oneshot};

use crate::{context::Context, model::Model, syzygy::Syzygy};
use std::{fmt, marker::PhantomData};

pub trait Effect<M: Model>: Sized + Send + Sync + fmt::Debug + 'static {
    fn execute(self, syzygy: &mut Syzygy<M, Self>);
}

pub trait Middleware<M: Model, E: Effect<M>>: Send + Sync {
    fn process(&mut self, effect: &mut E);
}

impl<M, E, F> Middleware<M, E> for F
where
    M: Model,
    E: Effect<M>,
    F: FnMut(&mut E) + Send + Sync,
{
    #[inline]
    fn process(&mut self, effect: &mut E) {
        self(effect);
    }
}

pub struct EffectSender<M: Model, E: Effect<M>> {
    pub(crate) tx: mpsc::UnboundedSender<(Vec<E>, Option<oneshot::Sender<()>>)>,
    pub(crate) effect_hook: Option<Box<dyn Fn() + Send + Sync>>,
    phantom: PhantomData<M>,
}

impl<M: Model, E: Effect<M>> std::fmt::Debug for EffectSender<M, E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let hook_str = self.effect_hook.as_ref().map_or("None", |_| "<fn>");

        f.debug_struct("EffectSender")
            .field("tx", &self.tx)
            .field("effect_hook", &hook_str)
            .field("phantom", &self.phantom)
            .finish()
    }
}

impl<M: Model, E: Effect<M>> Clone for EffectSender<M, E> {
    fn clone(&self) -> Self {
        Self {
            tx: self.tx.clone(),
            effect_hook: None,
            phantom: PhantomData,
        }
    }
}

pub struct EffectReceiver<M: Model, E: Effect<M>> {
    pub(crate) rx: mpsc::UnboundedReceiver<(Vec<E>, Option<oneshot::Sender<()>>)>,
    pub(crate) middlewares: Option<Vec<Box<dyn Middleware<M, E>>>>,
    phantom: PhantomData<M>,
}

impl<M: Model, E: Effect<M>> std::fmt::Debug for EffectReceiver<M, E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EffectBus")
            .field("rx", &self.rx)
            .field("middlewares", &"<middlewares>")
            .field("phantom", &self.phantom)
            .finish()
    }
}

#[derive(Debug, Builder)]
pub struct EffectQueue<M: Model, E: Effect<M>> {
    pub(crate) effect_sender: EffectSender<M, E>,
    pub(crate) effect_receiver: EffectReceiver<M, E>,
}

impl<M: Model, E: Effect<M>> Default for EffectQueue<M, E> {
    fn default() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        let effect_sender = EffectSender {
            tx,
            effect_hook: None,
            phantom: PhantomData,
        };
        let effect_receiver = EffectReceiver {
            rx,
            middlewares: None,
            phantom: PhantomData,
        };
        Self {
            effect_sender,
            effect_receiver,
        }
    }
}

impl<M: Model, E: Effect<M>> EffectSender<M, E> {
    #[inline]
    pub fn dispatch(
        &self,
        effects: Vec<E>,
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

impl<M: Model, E: Effect<M>> EffectReceiver<M, E> {
    #[must_use]
    #[inline]
    pub(crate) fn next_effect(&mut self) -> Option<(Vec<E>, Option<oneshot::Sender<()>>)> {
        match self.rx.try_recv() {
            Ok(effect) => Some(effect),
            Err(_) => None,
        }
    }
}

pub trait SendEffect: Context {
    fn effect_sender(&self) -> &EffectSender<Self::Model, Self::Effect>;
    #[inline]
    fn dispatch<T>(&self, effect: T)
    where
        T: Into<Self::Effect>,
    {
        self.effect_sender()
            .dispatch(vec![effect.into()], None, None);
    }

    #[inline]
    fn dispatch_awaitable<T>(&self, effect: T) -> oneshot::Receiver<()>
    where
        T: Into<Self::Effect>,
    {
        let (tx, rx) = oneshot::channel();

        self.effect_sender()
            .dispatch(vec![effect.into()], Some(tx), Some(rx))
            .expect("Effect completion failed")
    }

    #[inline]
    fn dispatch_many<T>(&self, effects: impl IntoIterator<Item = T>)
    where
        T: Into<Self::Effect>,
    {
        let effects = effects.into_iter().map(Into::into).collect();
        self.effect_sender().dispatch(effects, None, None);
    }

    #[inline]
    fn dispatch_many_awaitable<T>(
        &self,
        effects: impl IntoIterator<Item = T>,
    ) -> oneshot::Receiver<()>
    where
        T: Into<Self::Effect>,
    {
        let effects = effects.into_iter().map(Into::into).collect();
        let (tx, rx) = oneshot::channel();

        self.effect_sender()
            .dispatch(effects, Some(tx), Some(rx))
            .expect("Effect completion failed")
    }
}
