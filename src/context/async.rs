use crate::{
    effect_bus::{DispatchEffect, EffectBus},
    event_bus::{EmitEvent, EventBus},
    resource::{ResourceAccess, Resources},
    spawn::{SpawnAsync, TokioHandle},
    syzygy::Syzygy,
};

use super::Context;

#[derive(Debug)]
pub struct AsyncContext<M: 'static> {
    resources: Resources,
    effect_bus: EffectBus<M>,
    event_bus: EventBus<M>,
    tokio_handle: TokioHandle,
}

impl<M> Clone for AsyncContext<M> {
    fn clone(&self) -> Self {
        Self {
            resources: self.resources.clone(),
            effect_bus: self.effect_bus.clone(),
            event_bus: self.event_bus.clone(),
            tokio_handle: self.tokio_handle.clone(),
        }
    }
}

impl<M> Context for AsyncContext<M> {}

impl<M> From<Syzygy<M>> for AsyncContext<M> {
    fn from(value: Syzygy<M>) -> Self {
        Self {
            resources: value.resources().clone(),
            effect_bus: value.effect_bus().clone(),
            event_bus: value.event_bus().clone(),
            tokio_handle: value.tokio_handle().clone(),
        }
    }
}

impl<M> ResourceAccess for AsyncContext<M> {
    fn resources(&self) -> &Resources {
        &self.resources
    }
}

impl<M> DispatchEffect<M> for AsyncContext<M> {
    fn effect_bus(&self) -> &EffectBus<M> {
        &self.effect_bus
    }
}

impl<M> EmitEvent<M> for AsyncContext<M> {
    fn event_bus(&self) -> &EventBus<M> {
        &self.event_bus
    }
}

impl<M: 'static> SpawnAsync<M> for AsyncContext<M> {
    fn tokio_handle(&self) -> &TokioHandle {
        &self.tokio_handle
    }
}
