use tokio::sync::{mpsc, oneshot};

use crate::{model::Model, syzygy::Syzygy};
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
    fn process(&mut self, effect: &mut E) {
        self(effect);
    }
}

#[derive(Debug)]
pub struct EffectSender<M: Model, E: Effect<M>> {
    pub(crate) tx: mpsc::UnboundedSender<(Vec<E>, Option<oneshot::Sender<()>>)>,
    phantom: PhantomData<M>,
}

impl<M: Model, E: Effect<M>> Clone for EffectSender<M, E> {
    fn clone(&self) -> Self {
        Self {
            tx: self.tx.clone(),
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

#[derive(Debug)]
pub struct EffectBus<M: Model, E: Effect<M>> {
    pub(crate) effect_sender: EffectSender<M, E>,
    pub(crate) effect_receiver: EffectReceiver<M, E>,
}

impl<M: Model, E: Effect<M>> Default for EffectBus<M, E> {
    fn default() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        let effect_sender = EffectSender {
            tx,
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
    pub(crate) fn send(
        &self,
        effects: Vec<E>,
        tx: Option<oneshot::Sender<()>>,
        rx: Option<oneshot::Receiver<()>>,
    ) -> Option<oneshot::Receiver<()>> {
        let effects = effects.into_iter().map(Into::into).collect();

        self.tx
            .send((effects, tx))
            .expect("Effect bus channel unexpectedly closed");
        rx.map(Into::into)
    }
}

impl<M: Model, E: Effect<M>> EffectReceiver<M, E> {
    #[must_use]
    pub(crate) fn next_effect(&mut self) -> Option<(Vec<E>, Option<oneshot::Sender<()>>)> {
        match self.rx.try_recv() {
            Ok(effect) => Some(effect),
            Err(_) => None
        }
    }
}

pub trait SendEffect<M: Model, E: Effect<M>> {
    fn effect_sender(&self) -> &EffectSender<M, E>;
    fn send<T>(&self, effect: T)
    where
        T: Into<E>,
    {
        self.effect_sender().send(vec![effect.into()], None, None);
    }

    fn send_blocking<T>(&self, effect: T) -> oneshot::Receiver<()>
    where
        T: Into<E>,
    {
        let (tx, rx) = oneshot::channel();

        self.effect_sender()
            .send(vec![effect.into()], Some(tx), Some(rx))
            .expect("Effect completion failed")
    }

    fn send_multiple<T>(&self, effects: impl IntoIterator<Item = T>)
    where
        T: Into<E>,
    {
        let effects = effects.into_iter().map(Into::into).collect();
        self.effect_sender().send(effects, None, None);
    }

    fn send_blocking_multiple<T>(
        &self,
        effects: impl IntoIterator<Item = T>,
    ) -> oneshot::Receiver<()>
    where
        T: Into<E>,
    {
        let effects = effects.into_iter().map(Into::into).collect();
        let (tx, rx) = oneshot::channel();

        self.effect_sender()
            .send(effects, Some(tx), Some(rx))
            .expect("Effect completion failed")
    }
}
