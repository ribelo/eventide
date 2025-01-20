use std::future::Future;

use derive_more::derive::{Deref, DerefMut};
use tokio::sync::oneshot;

use crate::context::{Context, FromContext};
use crate::model::ModelModify;
use crate::{model::Model, prelude::AsyncContext, syzygy::Syzygy};

pub trait EffectFn<M: Model>: FnOnce(&mut Syzygy<M>) + Send + Sync + 'static {}

pub struct DispatchContext<'a, M: Model> {
    syzygy: &'a mut Syzygy<M>,
}

// impl<M: Model> Context for DispatchContext<'_, M> {
//     type Model = M;
// }

// impl<M: Model> FromContext<Syzygy<M>> for DispatchContext<'_, M> {
//     fn from_context(syzygy: &mut Syzygy<M>) -> Self {
//         Self { syzygy }
//     }
// }

impl<M, F> EffectFn<M> for F
where
    M: Model,
    F: FnOnce(&mut Syzygy<M>) + Send + Sync + 'static,
{
}

#[derive(Debug, Deref)]
pub struct EffectsTx<M: Model> {
    inner: crossbeam_channel::Sender<Box<dyn EffectFn<M>>>,
}

impl<M: Model> Clone for EffectsTx<M> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

#[derive(Debug, Deref, DerefMut)]
pub struct EffectsRx<M: Model> {
    inner: crossbeam_channel::Receiver<Box<dyn EffectFn<M>>>,
}

#[derive(Debug)]
pub struct EffectsBus<M: Model> {
    pub(crate) tx: EffectsTx<M>,
    pub(crate) rx: EffectsRx<M>,
}

impl<M: Model> Default for EffectsBus<M> {
    fn default() -> Self {
        let (tx, rx) = crossbeam_channel::unbounded();
        Self {
            tx: EffectsTx { inner: tx },
            rx: EffectsRx { inner: rx },
        }
    }
}

impl<M: Model> EffectsBus<M> {
    #[must_use]
    pub fn split(self) -> (EffectsTx<M>, EffectsRx<M>) {
        (self.tx, self.rx)
    }
}


pub trait DispatchEffect: Context {
    fn effects_tx(&self) -> &EffectsTx<Self::Model>;

    #[inline]
    fn send_effect<F>(&self, effect: F)
    where
        F: EffectFn<Self::Model> + Send + Sync + 'static,
    {
        self.effects_tx()
            .send(Box::new(effect))
            .expect("Effect receiver should be active");
    }

    #[inline]
    fn dispatch<F>(&self, effect: F)
    where
        F: EffectFn<Self::Model> + Send + Sync + 'static,
    {
        self.send_effect(effect);
    }

    #[must_use]
    #[inline]
    fn dispatch_sync(&self, effect: impl EffectFn<Self::Model>) -> oneshot::Receiver<()> {
        let (tx, rx) = oneshot::channel();
        let wrapped_effect = move |ctx: &mut Syzygy<Self::Model>| {
            (effect)(ctx);
            let _ = tx.send(());
        };
        self.send_effect(wrapped_effect);
        rx
    }

    #[inline]
    fn dispatch_update<F>(&self, update: F)
    where
        F: FnOnce(&mut Self::Model) + Send + Sync + 'static,
    {
        let wrapped = move |ctx: &mut Syzygy<Self::Model>| {
            ctx.update(update);
        };
        self.send_effect(wrapped);
    }

    #[inline]
    fn spawn<F>(&self, f: F)
    where
        F: FnOnce(AsyncContext<Self::Model>) + Send + Sync + 'static,
    {
        let wrapped = move |syzygy: &mut Syzygy<Self::Model>| {
            let ctx = AsyncContext::from_context(syzygy);
            tokio::task::spawn_blocking(move || f(ctx));
        };
        self.dispatch(wrapped);
    }

    #[inline]
    fn task<F, Fut>(&self, f: F)
    where
        F: FnOnce(AsyncContext<Self::Model>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let wrapped = move |syzygy: &mut Syzygy<Self::Model>| {
            let ctx = AsyncContext::from_context(syzygy);
            tokio::spawn(async move {
                (f)(ctx).await;
            });
        };
        self.send_effect(wrapped);
    }
}
