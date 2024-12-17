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
pub enum DispatchError {
    #[error("Channel closed")]
    ChannelClosed,
    #[error("Runtime stopped")]
    RuntimeStopped,
}

#[derive(Debug, Clone)]
pub struct Dispatcher {
    pub(crate) tx: Sender<Box<dyn Effect>>,
    pub(crate) rx: Receiver<Box<dyn Effect>>,
}

impl Default for Dispatcher {
    fn default() -> Self {
        let (tx, rx) = crossbeam_channel::unbounded();
        Self { tx, rx }
    }
}

impl Dispatcher {
    pub fn dispatch<H, T>(&self, effect: H) -> Result<(), DispatchError>
        where
            H: ContextExecutor<Syzygy, T, ()> + Send + Sync + 'static,
        {
            let effect = Box::new(move |syzygy: &Syzygy| effect.call(syzygy));
            self.tx
                .send(effect)
                .map_err(|_| DispatchError::ChannelClosed)
        }

    #[must_use]
    pub fn pop(&self) -> Option<Box<dyn Effect>> {
        self.rx.try_recv().ok()
    }
}

pub trait DispatchEffect: Sized + Context {
    fn dispatcher(&self) -> &Dispatcher;
    fn dispatch<H, T>(&self, effect: H) -> Result<(), DispatchError>
        where
            H: ContextExecutor<Syzygy, T, ()> + Send + Sync + 'static,
        {
            self.dispatcher().dispatch(effect)
        }
}
