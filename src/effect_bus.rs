use crate::{context::Context, model::Model, syzygy::Syzygy};

use crossbeam_channel::{Receiver, Sender};

pub trait Effect<M: Model>: Send + Sync + 'static {
    fn handle(self: Box<Self>, syzygy: &mut Syzygy<M>);
}

impl<M, F> Effect<M> for F
where
    M: Model,
    F: FnOnce(&mut Syzygy<M>) + Send + Sync + 'static,
{
    fn handle(self: Box<Self>, mut syzygy: &mut Syzygy<M>) {
        (*self)(&mut syzygy);
    }
}

#[derive(Debug, thiserror::Error)]
pub enum EffectError {
    #[error("Channel closed")]
    ChannelClosed,
    #[error("Runtime stopped")]
    RuntimeStopped,
}

#[derive(Debug)]
pub struct EffectBus<M> {
    pub(crate) tx: Sender<Box<dyn Effect<M>>>,
    pub(crate) rx: Receiver<Box<dyn Effect<M>>>,
}

impl<M> Clone for EffectBus<M> {
    fn clone(&self) -> Self {
        Self {
            tx: self.tx.clone(),
            rx: self.rx.clone(),
        }
    }
}

impl<M> Default for EffectBus<M> {
    fn default() -> Self {
        let (tx, rx) = crossbeam_channel::unbounded();
        Self { tx, rx }
    }
}

impl<M: Model> EffectBus<M> {
    pub fn dispatch<E>(&self, effect: E) -> Result<(), EffectError>
    where
        E: FnOnce(&mut Syzygy<M>) + Send + Sync + 'static,
    {
        let effect = Box::new(effect);
        self.tx.send(effect).map_err(|_| EffectError::ChannelClosed)
    }

    #[must_use]
    pub fn pop(&self) -> Option<Box<dyn Effect<M>>> {
        self.rx.try_recv().ok()
    }
}

pub trait DispatchEffect<M: Model>: Sized + Context {
    fn effect_bus(&self) -> &EffectBus<M>;
    fn dispatch<E>(&self, effect: E) -> Result<(), EffectError>
    where
        E: FnOnce(&mut Syzygy<M>) + Send + Sync + 'static,
    {
        self.effect_bus().dispatch(effect)
    }
}
