use crate::{
    context::{Context, FromContext},
    syzygy::Syzygy,
};

use crossbeam_channel::{Receiver, Sender};

pub trait Effect: Send + Sync + 'static {
    fn handle(self: Box<Self>, syzygy: Syzygy);
}

impl<F> Effect for F
where
    F: FnOnce(Syzygy) + Send + Sync + 'static,
{
    fn handle(self: Box<Self>, syzygy: Syzygy) {
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
    pub fn dispatch<C, E, T>(&self, effect: E) -> Result<(), DispatchError>
    where
        C: FromContext<Syzygy>,
        E: FnOnce(T) + Send + Sync + 'static,
        T: FromContext<C> + FromContext<Syzygy>,
    {
        let effect = Box::new(|syzygy: Syzygy| effect(T::from_context(syzygy)));
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
    fn dispatch<E, T>(&self, effect: E) -> Result<(), DispatchError>
    where
        E: FnOnce(T) + Send + Sync + 'static,
        T: FromContext<Syzygy>,
    {
        self.dispatcher().dispatch(effect)
    }
}
