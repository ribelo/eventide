use crate::{
    effects::Effect,
    model::{Model, ModelAccess},
    resource::{ResourceAccess, Resources},
    spawn::{SpawnAsync, TokioHandle},
};

use bon::Builder;

use crate::effects::{EffectSender, SendEffect};

use super::{Context, FromContext};

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

impl<M: Model, E: Effect<M>> Context for AsyncContext<M, E> {
    type Model = M;
    type Effect = E;
}

impl<T, M: Model, E: Effect<M>> FromContext<T> for AsyncContext<M, E>
where
    T: Context<Model = M, Effect = E>,
    T: ModelAccess + ResourceAccess + SendEffect + SpawnAsync,
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

impl<M: Model, E: Effect<M>> ModelAccess for AsyncContext<M, E> {
    fn model(&self) -> &M {
        &self.model_snapshot
    }
}

impl<M: Model, E: Effect<M>> ResourceAccess for AsyncContext<M, E> {
    fn resources(&self) -> &Resources {
        &self.resources
    }
}

impl<M: Model, E: Effect<M>> SendEffect for AsyncContext<M, E> {
    fn effect_sender(&self) -> &EffectSender<M, E> {
        &self.effect_sender
    }
}

impl<M: Model, E: Effect<M>> SpawnAsync for AsyncContext<M, E> {
    fn tokio_handle(&self) -> &TokioHandle {
        &self.tokio_handle
    }
}
