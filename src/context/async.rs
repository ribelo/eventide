use crate::{
    effect_bus::Effect,
    model::{Model, ModelAccess},
    resource::{ResourceAccess, Resources},
    spawn::{SpawnAsync, TokioHandle},
};

use bon::Builder;

use crate::effect_bus::{EffectSender, SendEffect};

use super::{Context, IntoContext};

#[derive(Debug, Builder)]
pub struct AsyncContext<M: Model, E: Effect<M>> {
    pub model_snapshot: M,
    pub resources: Resources,
    pub effect_sender: EffectSender<M, E>,
    pub tokio_handle: TokioHandle,
}

impl<M: Model, E: Effect<M>> Clone for AsyncContext<M, E> {
    fn clone(&self) -> Self {
        Self {
            model_snapshot: self.model_snapshot.clone(),
            resources: self.resources.clone(),
            effect_sender: self.effect_sender.clone(),
            tokio_handle: self.tokio_handle.clone(),
        }
    }
}

impl<M: Model, E: Effect<M>> Context for AsyncContext<M, E> {}

impl<T, M: Model, E: Effect<M>> IntoContext<AsyncContext<M, E>> for T
where
    T: ModelAccess<M> + ResourceAccess + SendEffect<M, E> + SpawnAsync<M, E>,
{
    fn into_context(&self) -> AsyncContext<M, E> {
        AsyncContext {
            model_snapshot: self.model().clone(),
            resources: self.resources().clone(),
            effect_sender: self.effect_sender().clone(),
            tokio_handle: self.tokio_handle().clone(),
        }
    }
}

impl<M: Model, E: Effect<M>> ModelAccess<M> for AsyncContext<M, E> {
    fn model(&self) -> &M {
        &self.model_snapshot
    }
}

impl<M: Model, E: Effect<M>> ResourceAccess for AsyncContext<M, E> {
    fn resources(&self) -> &Resources {
        &self.resources
    }
}

impl<M: Model, E: Effect<M>> SendEffect<M, E> for AsyncContext<M, E> {
    fn effect_sender(&self) -> &EffectSender<M, E> {
        &self.effect_sender
    }
}

impl<M: Model, E: Effect<M>> SpawnAsync<M, E> for AsyncContext<M, E> {
    fn tokio_handle(&self) -> &TokioHandle {
        &self.tokio_handle
    }
}
