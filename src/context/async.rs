use crate::{
    effect_bus::{DispatchEffect, EffectBus},
    event_bus::{EmitEvent, EventBus},
    resource::{ResourceAccess, Resources},
    spawn::{TokioHandle, SpawnAsync},
};

use super::{Context, FromContext};

#[derive(Debug, Clone)]
pub struct AsyncContext<M> {
    resources: Resources,
    effect_bus: EffectBus<M>,
    event_bus: EventBus<M>,
    tokio_handle: TokioHandle,
}

impl<M> Context for AsyncContext<M> {}

impl<'a, C, M> FromContext<'a, C> for AsyncContext<M>
where
    C: ResourceAccess + DispatchEffect<M> + EmitEvent<M> + SpawnAsync<'a, M> + 'static,
    M: 'static,
{
    fn from_context(cx: &'a C) -> Self {
        Self {
            resources: cx.resources().clone(),
            effect_bus: cx.effect_bus().clone(),
            event_bus: cx.event_bus().clone(),
            tokio_handle: cx.tokio_handle().clone(),
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

impl<'a, M: 'static> SpawnAsync<'a, M> for AsyncContext<M> {
    fn tokio_handle(&'a self) -> &'a TokioHandle {
        &self.tokio_handle
    }
}
