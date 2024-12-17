use crate::{
    dispatch::{DispatchEffect, Dispatcher},
    event_bus::{EmitEvent, EventBus},
    model::{ModelAccess, Models},
    resource::{ResourceAccess, Resources},
};

use super::{Context, FromContext};

#[derive(Debug, Clone)]
pub struct EventContext {
    models: Models,
    resources: Resources,
    dispatcher: Dispatcher,
    emiter: EventBus,
}

impl Context for EventContext {}

impl<C> FromContext<C> for EventContext
where
    C: ModelAccess + ResourceAccess + DispatchEffect + EmitEvent,
{
    fn from_context(cx: &C) -> Self {
        Self {
            models: <C as ModelAccess>::models(cx).clone(),
            resources: <C as ResourceAccess>::resources(cx).clone(),
            dispatcher: <C as DispatchEffect>::dispatcher(cx).clone(),
            emiter: <C as EmitEvent>::event_bus(cx).clone(),
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
    fn dispatcher(&self) -> &Dispatcher {
        &self.dispatcher
    }
}

impl EmitEvent for EventContext {
    fn event_bus(&self) -> &EventBus {
        &self.emiter
    }
}
