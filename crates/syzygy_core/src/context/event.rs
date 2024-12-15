use crate::{
    dispatch::{DispatchEffect, Dispatcher}, event_bus::{EmitEvent, EventBus}, model::{ModelAccess, Models}, permission::{self, PermissionHolder}, resource::{ResourceAccess, Resources}, syzygy::Syzygy
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

impl FromContext<Syzygy> for EventContext {
    fn from_context(cx: Syzygy) -> Self {
        Self {
            models: cx.models.clone(),
            resources: cx.resources.clone(),
            dispatcher: cx.dispatcher.clone(),
            emiter: cx.event_bus.clone(),
        }
    }
}

impl PermissionHolder for EventContext {
    type Granted = permission::None;
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
