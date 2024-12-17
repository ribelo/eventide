use crate::{
    context::{Context, ContextExecutor},
    syzygy::Syzygy,
};

use crossbeam_channel::{Receiver, Sender};

pub trait Effect: Send + Sync + 'static {
    fn handle(self: Box<Self>, syzygy: &Syzygy);
}

impl<F> Effect for F
where
    F: FnOnce(&Syzygy) + Send + Sync + 'static,
{
    fn handle(self: Box<Self>, syzygy: &Syzygy) {
        (*self)(syzygy);
    }
}

#[derive(Debug, thiserror::Error)]
pub enum EffectError {
    #[error("Channel closed")]
    ChannelClosed,
    #[error("Runtime stopped")]
    RuntimeStopped,
}

#[derive(Debug, Clone)]
pub struct EffectBus {
    pub(crate) tx: Sender<Box<dyn Effect>>,
    pub(crate) rx: Receiver<Box<dyn Effect>>,
}

impl Default for EffectBus {
    fn default() -> Self {
        let (tx, rx) = crossbeam_channel::unbounded();
        Self { tx, rx }
    }
}

impl EffectBus {
    pub fn dispatch<H, T>(&self, effect: H) -> Result<(), EffectError>
        where
            H: ContextExecutor<Syzygy, T, ()> + Send + Sync + 'static,
        {
            let effect = Box::new(move |syzygy: &Syzygy| effect.call(syzygy));
            self.tx
                .send(effect)
                .map_err(|_| EffectError::ChannelClosed)
        }

    #[must_use]
    pub fn pop(&self) -> Option<Box<dyn Effect>> {
        self.rx.try_recv().ok()
    }
}

pub trait DispatchEffect: Sized + Context {
    fn effect_bus(&self) -> &EffectBus;
    fn dispatch<H, T>(&self, effect: H) -> Result<(), EffectError>
        where
            H: ContextExecutor<Syzygy, T, ()> + Send + Sync + 'static,
        {
            self.effect_bus().dispatch(effect)
        }
}
