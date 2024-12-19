use crate::{
    effect_bus::{DispatchEffect, EffectBus},
    event_bus::{EmitEvent, EventBus},
    model::ModelAccess,
    resource::{ResourceAccess, Resources},
};

use super::{Context, FromContext};

#[derive(Debug)]
pub struct EventContext<'a, M> {
    model: &'a M,
    resources: Resources,
    effect_bus: EffectBus<M>,
    event_bus: EventBus<M>,
}

impl<M> Context for EventContext<'_, M> {}

impl<'a, C, M> FromContext<'a, C> for EventContext<'a, M>
where
    C: ModelAccess<M> + ResourceAccess + DispatchEffect<M> + EmitEvent<M>,
{
    fn from_context(cx: &'a C) -> Self {
        Self {
            model: cx.model(),
            resources: cx.resources().clone(),
            effect_bus: cx.effect_bus().clone(),
            event_bus: cx.event_bus().clone(),
        }
    }
}

impl<'a, M> ModelAccess<M> for EventContext<'a, M> {
    fn model(&self) -> &'a M {
        self.model
    }
}

impl<M> ResourceAccess for EventContext<'_, M> {
    fn resources(&self) -> &Resources {
        &self.resources
    }
}

impl<M> DispatchEffect<M> for EventContext<'_, M> {
    fn effect_bus(&self) -> &EffectBus<M> {
        &self.effect_bus
    }
}

impl<M> EmitEvent<M> for EventContext<'_, M> {
    fn event_bus(&self) -> &EventBus<M> {
        &self.event_bus
    }
}
