use crate::{
    effect_bus::Effect, model::{Model, ModelAccess}, resource::{ResourceAccess, Resources}, spawn::{SpawnAsync, TokioHandle}, syzygy::Syzygy
};

use bon::Builder;

use crate::effect_bus::{DispatchEffect, EffectBus};

use super::Context;

#[derive(Debug, Builder)]
pub struct AsyncContext<M: Model, E: Effect<M>> {
    pub model_snapshot: M,
    pub resources: Resources,
    pub effect_bus: EffectBus<M, E>,
    pub tokio_handle: TokioHandle,
}

impl<M: Model, E: Effect<M>> Clone for AsyncContext<M, E> {
    fn clone(&self) -> Self {
        Self {
            model_snapshot: self.model_snapshot.clone(),
            resources: self.resources.clone(),
            effect_bus: self.effect_bus.clone(),
            tokio_handle: self.tokio_handle.clone(),
        }
    }
}

impl<M: Model, E: Effect<M>> Context for AsyncContext<M, E> {}

impl<M: Model, E: Effect<M>> From<Syzygy<M, E>> for AsyncContext<M, E> {
    fn from(syzygy: Syzygy<M, E>) -> Self {
        Self {
            model_snapshot: syzygy.model.clone(),
            resources: syzygy.resources().clone(),
            effect_bus: syzygy.effect_bus().clone(),
            tokio_handle: syzygy.tokio_handle().clone(),
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

impl<M: Model, E: Effect<M>> DispatchEffect<M, E> for AsyncContext<M, E> {
    fn effect_bus(&self) -> &EffectBus<M, E> {
        &self.effect_bus
    }
}

impl<M: Model, E: Effect<M>> SpawnAsync<M, E> for AsyncContext<M, E> {
    fn tokio_handle(&self) -> &TokioHandle {
        &self.tokio_handle
    }
}
