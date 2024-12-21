use crate::{
    effect_bus::{DispatchEffect, EffectBus}, model::Model, resource::{ResourceAccess, Resources}, spawn::{SpawnAsync, TokioHandle}, syzygy::Syzygy
};

use super::Context;

#[derive(Debug)]
pub struct AsyncContext<M: 'static> {
    resources: Resources,
    effect_bus: EffectBus<M>,
    tokio_handle: TokioHandle,
    model_snapshot: M,
}

impl<M: Clone> Clone for AsyncContext<M> {
    fn clone(&self) -> Self {
        Self {
            resources: self.resources.clone(),
            effect_bus: self.effect_bus.clone(),
            tokio_handle: self.tokio_handle.clone(),
            model_snapshot: self.model_snapshot.clone(),
        }
    }
}

impl<M: Model> Context for AsyncContext<M> {}

impl<M: Model> From<Syzygy<M>> for AsyncContext<M> {
    fn from(syzygy: Syzygy<M>) -> Self {
        Self {
            resources: syzygy.resources().clone(),
            effect_bus: syzygy.effect_bus().clone(),
            tokio_handle: syzygy.tokio_handle().clone(),
            model_snapshot: syzygy.model.clone(),
        }
    }
}

impl<M: Model> ResourceAccess for AsyncContext<M> {
    fn resources(&self) -> &Resources {
        &self.resources
    }
}

impl<M: Model> DispatchEffect<M> for AsyncContext<M> {
    fn effect_bus(&self) -> &EffectBus<M> {
        &self.effect_bus
    }
}

impl<M: Model> SpawnAsync<M> for AsyncContext<M> {
    fn tokio_handle(&self) -> &TokioHandle {
        &self.tokio_handle
    }
}
