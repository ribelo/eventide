use std::marker::PhantomData;

use bon::Builder;
use derive_more::{
    derive::{From, Into},
    Deref,
};
use dyn_clone::{self, DynClone};
use tokio::sync::mpsc;

use crate::{context::Context, model::Model, syzygy::Syzygy};

pub trait Effect: Clone + Send + Sync + 'static {
    type Model: Model;
    fn execute(
        &self,
        cx: &mut Syzygy<Self::Model, Self>,
    ) -> impl IntoEffects<Effect = Self>;
}

// #[derive(Clone, Cop)]
// pub struct Nop<M>(PhantomData<M>);

// impl<M: Model> Effect for Nop<M> {
//     type Model = M;

//     fn execute(&self, cx: &mut Syzygy<Self::Model, Self>) -> Option<impl IntoEffects<Effect = Self>> {
//         None::<Vec<Self>>
//     }
// }

#[derive(Clone, Deref, Builder)]
pub struct Effects<E: Effect> {
    #[builder(field)]
    pub(crate) items: Vec<E>,
}

impl<E: Effect> Default for Effects<E> {
    fn default() -> Self {
        Self {items: Vec::default()}
    }
}

impl<E: Effect> Effects<E> {
    #[must_use]
    pub fn none() -> Self {
        Self::default()
    }
}

impl<E: Effect, S: effects_builder::State> EffectsBuilder<E, S> {
    pub fn with(mut self, effect: impl Into<E>) -> EffectsBuilder<E, S> {
        self.items.push(effect.into());
        self
    }
}

pub trait IntoEffects {
    type Effect: Effect;
    fn into_effects(self) -> Effects<Self::Effect>;
}

impl<E: Effect> IntoEffects for Effects<E> {
    type Effect = E;
    fn into_effects(self) -> Effects<E> {
        self
    }
}

impl<E: Effect> IntoEffects for E {
    type Effect = E;
    fn into_effects(self) -> Effects<E> {
        Effects { items: vec![self] }
    }
}

impl<E: Effect> IntoEffects for Vec<E> {
    type Effect = E;
    fn into_effects(self) -> Effects<E> {
        Effects { items: self }
    }
}

pub struct EffectSender<M: Model, E: Effect> {
    pub(crate) tx: mpsc::UnboundedSender<Effects<E>>,
    pub(crate) effect_hook: Option<Box<dyn Fn() + Send + Sync>>,
    phantom: PhantomData<M>,
}

impl<M: Model, E: Effect> std::fmt::Debug for EffectSender<M, E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let hook_str = self.effect_hook.as_ref().map_or("None", |_| "<fn>");

        f.debug_struct("EffectSender")
            .field("tx", &self.tx)
            .field("effect_hook", &hook_str)
            .field("phantom", &self.phantom)
            .finish()
    }
}

impl<M: Model, E: Effect> Clone for EffectSender<M, E> {
    fn clone(&self) -> Self {
        Self {
            tx: self.tx.clone(),
            effect_hook: None,
            phantom: PhantomData,
        }
    }
}

pub struct EffectReceiver<M: Model, E: Effect> {
    pub(crate) rx: mpsc::UnboundedReceiver<Effects<E>>,
    phantom: PhantomData<M>,
}

impl<M: Model, E: Effect> std::fmt::Debug for EffectReceiver<M, E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EffectBus")
            .field("rx", &self.rx)
            .field("middlewares", &"<middlewares>")
            .field("phantom", &self.phantom)
            .finish()
    }
}

#[derive(Debug, Builder)]
pub struct EffectQueue<M: Model, E: Effect> {
    pub(crate) effect_sender: EffectSender<M, E>,
    pub(crate) effect_receiver: EffectReceiver<M, E>,
}

impl<M: Model, E: Effect> Default for EffectQueue<M, E> {
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

impl<M: Model, E: Effect> EffectSender<M, E> {
    #[inline]
    pub fn dispatch(&self, msgs: Effects<E>) {
        self.tx
            .send(msgs)
            .expect("Effect bus channel unexpectedly closed");
        if let Some(hook) = &self.effect_hook {
            (hook)();
        }
    }
}

impl<M: Model, E: Effect> EffectReceiver<M, E> {
    #[must_use]
    #[inline]
    pub(crate) fn next_effect(&mut self) -> Option<Effects<E>> {
        match self.rx.try_recv() {
            Ok(effect) => Some(effect),
            Err(_) => None,
        }
    }
}

pub trait DispatchEffect: Context {
    fn effect_sender(&self) -> &EffectSender<Self::Model, Self::Effect>;
    #[inline]
    fn dispatch(&self, effect: impl IntoEffects<Effect = Self::Effect>) {
        self.effect_sender().dispatch(effect.into_effects());
    }
}
