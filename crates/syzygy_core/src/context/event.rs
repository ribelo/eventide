use crate::{
    effect_bus::{DispatchEffect, EffectBus},
    event_bus::{EmitEvent, EventBus},
    model::{ModelAccess, Models},
    resource::{ResourceAccess, Resources},
};

use super::{Context, FromContext};

#[derive(Debug, Clone)]
pub struct EventContext {
    models: Models,
    resources: Resources,
    effect_bus: EffectBus,
    event_bus: EventBus,
}

impl Context for EventContext {}

impl<C> FromContext<C> for EventContext
where
    C: ModelAccess + ResourceAccess + DispatchEffect + EmitEvent,
{
    fn from_context(cx: &C) -> Self {
        Self {
            models: cx.models().clone(),
            resources: cx.resources().clone(),
            effect_bus: cx.effect_bus().clone(),
            event_bus: cx.event_bus().clone(),
        }
    }
}

impl ModelAccess for EventContext {
    fn models(&self) -> &Models {
        &self.models
    }
}

impl ResourceAccess for EventContext {
    fn resources(&self) -> &Resources {
        &self.resources
    }
}

impl DispatchEffect for EventContext {
    fn effect_bus(&self) -> &EffectBus {
        &self.effect_bus
    }
}

impl EmitEvent for EventContext {
    fn event_bus(&self) -> &EventBus {
        &self.event_bus
    }
}
