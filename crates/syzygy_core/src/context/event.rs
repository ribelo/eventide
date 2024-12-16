use crate::{
    dispatch::{DispatchEffect, Dispatcher},
    event_bus::{EmitEvent, EventBus},
    model::{ModelAccess, Models},
    resource::{ResourceAccess, Resources},
    syzygy::Syzygy,
};

use super::{Context, BorrowFromContext};

#[derive(Debug, Clone)]
pub struct EventContext<'a> {
    models: Models,
    resources: Resources,
    dispatcher: Dispatcher<'a>,
    emiter: EventBus,
}

impl<'a> Context for EventContext<'a> {}

impl<'a> BorrowFromContext<'a, Syzygy> for EventContext<'a> {
    fn from_context(cx: &'a Syzygy) -> Self {
        Self {
            models: cx.models.clone(),
            resources: cx.resources.clone(),
            dispatcher: cx.dispatcher.clone(),
            emiter: cx.event_bus.clone(),
        }
    }
}

impl<'a> ModelAccess for EventContext<'a> {
    fn models(&self) -> &Models {
        &self.models
    }
}

impl<'a> ResourceAccess for EventContext<'a> {
    fn resources(&self) -> &Resources {
        &self.resources
    }
}

impl<'a> DispatchEffect<'a> for EventContext<'a> {
    fn dispatcher(&self) -> &Dispatcher<'a> {
        &self.dispatcher
    }
}

impl<'a> EmitEvent for EventContext<'a> {
    fn event_bus(&self) -> &EventBus {
        &self.emiter
    }
}
