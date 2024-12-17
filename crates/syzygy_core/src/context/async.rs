use std::sync::Arc;

use crate::{
    effect_bus::{DispatchEffect, EffectBus},
    event_bus::{EmitEvent, EventBus},
    resource::{ResourceAccess, Resources},
    spawn::SpawnAsync,
};

use super::{Context, FromContext};

#[derive(Debug, Clone)]
pub struct AsyncContext {
    resources: Resources,
    effect_bus: EffectBus,
    event_bus: EventBus,
    tokio_rt: Arc<tokio::runtime::Runtime>,
}

impl Context for AsyncContext {}

impl<C> FromContext<C> for AsyncContext
where
    C: ResourceAccess + DispatchEffect + EmitEvent + SpawnAsync + 'static,
{
    fn from_context(cx: &C) -> Self {
        Self {
            resources: cx.resources().clone(),
            effect_bus: cx.effect_bus().clone(),
            event_bus: cx.event_bus().clone(),
            tokio_rt: cx.tokio_rt(),
        }
    }
}

impl ResourceAccess for AsyncContext {
    fn resources(&self) -> &Resources {
        &self.resources
    }
}

impl DispatchEffect for AsyncContext {
    fn effect_bus(&self) -> &EffectBus {
        &self.effect_bus
    }
}

impl EmitEvent for AsyncContext {
    fn event_bus(&self) -> &EventBus {
        &self.event_bus
    }
}

impl SpawnAsync for AsyncContext {
    fn tokio_rt(&self) -> Arc<tokio::runtime::Runtime> {
        Arc::clone(&self.tokio_rt)
    }
}
