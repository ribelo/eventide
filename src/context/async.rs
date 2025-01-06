use crate::{
    dispatch::Effect,
    model::{Model, ModelAccess},
    resource::{ResourceAccess, Resources},
    spawn::{SpawnAsync, TokioHandle},
};

use bon::Builder;

use crate::dispatch::{EffectSender, DispatchEffect};

use super::{Context, FromContext};

#[derive(Debug, Builder)]
pub struct AsyncContext<M: Model> {
    pub model_snapshot: M,
    pub resources: Resources,
    pub effect_sender: EffectSender<M>,
    pub tokio_handle: TokioHandle,
}

impl<M: Model> Clone for AsyncContext<M> {
    fn clone(&self) -> Self {
        Self {
            model_snapshot: self.model_snapshot.clone(),
            resources: self.resources.clone(),
            effect_sender: self.effect_sender.clone(),
            tokio_handle: self.tokio_handle.clone(),
        }
    }
}

impl<M: Model> Context for AsyncContext<M> {
    type Model = M;
}

impl<T, M: Model> FromContext<T> for AsyncContext<M>
where
    T: Context<Model = M>,
    T: ModelAccess + ResourceAccess + DispatchEffect + SpawnAsync,
{
    fn from_context(context: &T) -> Self {
        Self {
            model_snapshot: context.model().clone(),
            resources: context.resources().clone(),
            effect_sender: context.effect_sender().clone(),
            tokio_handle: context.tokio_handle().clone(),
        }
    }
}

impl<M: Model> ModelAccess for AsyncContext<M> {
    fn model(&self) -> &M {
        &self.model_snapshot
    }
}

impl<M: Model> ResourceAccess for AsyncContext<M> {
    fn resources(&self) -> &Resources {
        &self.resources
    }
}

impl<M: Model> DispatchEffect for AsyncContext<M> {
    fn effect_sender(&self) -> &EffectSender<M> {
        &self.effect_sender
    }
}

impl<M: Model> SpawnAsync for AsyncContext<M> {
    fn tokio_handle(&self) -> &TokioHandle {
        &self.tokio_handle
    }
}
