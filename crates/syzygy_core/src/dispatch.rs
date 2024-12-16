use crate::{
    context::{Context, ContextExecutor, BorrowFromContext},
    syzygy::Syzygy,
};

use crossbeam_channel::{Receiver, Sender};

pub trait Effect<'a>: Send + Sync + 'static {
    fn handle(self: Box<Self>, syzygy: &'a Syzygy);
}

impl<'a, F> Effect<'a> for F
where
    F: FnOnce(&'a Syzygy) + Send + Sync + 'static,
{
    fn handle(self: Box<Self>, syzygy: &'a Syzygy) {
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
pub struct Dispatcher<'a> {
    pub(crate) tx: Sender<Box<dyn Effect<'a>>>,
    pub(crate) rx: Receiver<Box<dyn Effect<'a>>>,
}

impl<'a> Default for Dispatcher<'a> {
    fn default() -> Self {
        let (tx, rx) = crossbeam_channel::unbounded();
        Self { tx, rx }
    }
}

impl<'a> Dispatcher<'a> {
    pub fn dispatch<H, T>(&self, effect: H) -> Result<(), DispatchError>
        where
            H: ContextExecutor<'a, Syzygy, T, ()> + Send + Sync + 'static,
        {
            let effect = Box::new(move |syzygy: &'a Syzygy| effect.call(syzygy));
            self.tx
                .send(effect)
                .map_err(|_| DispatchError::ChannelClosed)
        }

    #[must_use]
    pub fn pop(&self) -> Option<Box<dyn Effect<'a>>> {
        self.rx.try_recv().ok()
    }
}

pub trait DispatchEffect<'a>: Sized + Context {
    fn dispatcher(&self) -> &'a Dispatcher;
    fn dispatch<H, T>(&self, effect: H) -> Result<(), DispatchError>
        where
            H: ContextExecutor<'a, Syzygy, T, ()> + Send + Sync + 'static,
        {
            self.dispatcher().dispatch(effect)
        }
}
