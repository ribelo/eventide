use thiserror::Error;

use crate::{model::Model, syzygy::Syzygy};
use std::marker::PhantomData;
#[cfg(feature = "async")]
use std::{future::Future, pin::Pin, task::Poll};

pub trait Effect<M: Model>: Sized + Send + Sync + 'static {
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
        self(effect)
    }
}

#[derive(Debug)]
pub struct EffectCompletion {
    #[cfg(not(feature = "async"))]
    rx: crossbeam_channel::Receiver<()>,
    #[cfg(feature = "async")]
    rx: tokio::sync::oneshot::Receiver<()>,
}

#[cfg(not(feature = "async"))]
impl From<crossbeam_channel::Receiver<()>> for EffectCompletion {
    fn from(rx: crossbeam_channel::Receiver<()>) -> Self {
        Self { rx }
    }
}

#[cfg(feature = "async")]
impl From<tokio::sync::oneshot::Receiver<()>> for EffectCompletion {
    fn from(rx: tokio::sync::oneshot::Receiver<()>) -> Self {
        Self { rx }
    }
}

#[derive(Debug, Error)]
pub enum EffectError {
    #[error("receiver disconnected")]
    Disconnected,
}

impl EffectCompletion {
    pub fn wait(self) -> Result<(), EffectError> {
        #[cfg(not(feature = "async"))]
        {
            self.rx.recv().map_err(|_| EffectError::Disconnected)
        }
        #[cfg(feature = "async")]
        {
            self.rx
                .blocking_recv()
                .map_err(|_| EffectError::Disconnected)
        }
    }
}

#[cfg(feature = "async")]
impl Future for EffectCompletion {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        match Pin::new(&mut self.rx).poll(cx) {
            Poll::Ready(_) => Poll::Ready(()),
            Poll::Pending => Poll::Pending,
        }
    }
}

pub struct EffectBus<M: Model, E: Effect<M>> {
    #[cfg(not(feature = "async"))]
    pub(crate) tx: crossbeam_channel::Sender<(Vec<E>, crossbeam_channel::Sender<()>)>,
    #[cfg(not(feature = "async"))]
    pub(crate) rx: crossbeam_channel::Receiver<(Vec<E>, crossbeam_channel::Sender<()>)>,
    #[cfg(feature = "async")]
    pub(crate) tx: crossbeam_channel::Sender<(Vec<E>, tokio::sync::oneshot::Sender<()>)>,
    #[cfg(feature = "async")]
    pub(crate) rx: crossbeam_channel::Receiver<(Vec<E>, tokio::sync::oneshot::Sender<()>)>,
    pub(crate) middlewares: Option<Vec<Box<dyn Middleware<M, E>>>>,
    pub(crate) after_effect: Option<Box<dyn Fn() + Send + Sync + 'static>>,
    phantom: PhantomData<M>,
}

impl<M: Model, E: Effect<M>> std::fmt::Debug for EffectBus<M, E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EffectBus")
            .field("tx", &self.tx)
            .field("rx", &self.rx)
            .field("middlewares", &"<middlewares>")
            .field("after_effect", &self.after_effect.as_ref().map(|_| "<fn>"))
            .field("phantom", &self.phantom)
            .finish()
    }
}

impl<M: Model, E: Effect<M>> Clone for EffectBus<M, E> {
    fn clone(&self) -> Self {
        Self {
            tx: self.tx.clone(),
            rx: self.rx.clone(),
            after_effect: None,
            middlewares: None,
            phantom: PhantomData,
        }
    }
}

impl<M: Model, E: Effect<M>> Default for EffectBus<M, E> {
    fn default() -> Self {
        let (tx, rx) = crossbeam_channel::unbounded();
        Self {
            tx,
            rx,
            after_effect: None,
            middlewares: None,
            phantom: PhantomData,
        }
    }
}

impl<M: Model, E: Effect<M>> EffectBus<M, E> {
    pub fn dispatch(&self, effect: impl Into<E>) -> EffectCompletion {
        let effects = vec![effect.into()];
        #[cfg(not(feature = "async"))]
        let (tx, rx) = crossbeam_channel::bounded(1);
        #[cfg(feature = "async")]
        let (tx, rx) = tokio::sync::oneshot::channel();

        self.tx
            .send((effects, tx))
            .expect("Effect bus channel unexpectedly closed");
        if let Some(after_effect) = self.after_effect.as_ref() {
            after_effect();
        }
        rx.into()
    }

    pub fn dispatch_multiple<T>(&self, effects: impl IntoIterator<Item = T>) -> EffectCompletion
    where
        T: Into<E>,
    {
        let effects = effects.into_iter().map(Into::into).collect();
        #[cfg(not(feature = "async"))]
        let (tx, rx) = crossbeam_channel::bounded(1);
        #[cfg(feature = "async")]
        let (tx, rx) = tokio::sync::oneshot::channel();

        self.tx
            .send((effects, tx))
            .expect("Effect bus channel unexpectedly closed");
        if let Some(after_effect) = self.after_effect.as_ref() {
            after_effect();
        }
        rx.into()
    }

    #[must_use]
    pub fn pop(&self) -> Option<Vec<E>> {
        self.rx.try_recv().ok().map(|(effects, tx)| {
            tx.send(()).ok();
            effects
        })
    }
}

pub trait DispatchEffect<M: Model, E: Effect<M>> {
    fn effect_bus(&self) -> &EffectBus<M, E>;
    fn dispatch<T>(&self, effect: T) -> EffectCompletion
    where
        T: Into<E>,
    {
        self.effect_bus().dispatch(effect.into())
    }
    fn dispatch_multiple<T>(&self, effects: impl IntoIterator<Item = T>) -> EffectCompletion
    where
        T: Into<E>,
    {
        self.effect_bus().dispatch_multiple(effects)
    }
}
