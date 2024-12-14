use std::sync::Arc;

use crate::{
    dispatch::{DispatchEffect, Dispatcher},
    event_bus::{EmitEvent, EventBus},
    resource::{ResourceAccess, Resources},
    spawn::SpawnAsync,
    syzygy::Syzygy,
};

use super::{Context, FromContext};

#[derive(Debug, Clone)]
pub struct AsyncContext {
    resources: Resources,
    dispatcher: Dispatcher,
    event_bus: EventBus,
    tokio_rt: Arc<tokio::runtime::Runtime>,
}

impl Context for AsyncContext {}

impl FromContext<AsyncContext> for AsyncContext {
    fn from_context(cx: AsyncContext) -> Self {
        cx
    }
}

impl FromContext<Syzygy> for AsyncContext {
    fn from_context(cx: Syzygy) -> Self {
        Self {
            dispatcher: cx.dispatcher.clone(),
            resources: cx.resources.clone(),
            event_bus: cx.event_bus.clone(),
            tokio_rt: Arc::clone(&cx.tokio_rt),
        }
    }
}

impl ResourceAccess for AsyncContext {
    fn resources(&self) -> &Resources {
        &self.resources
    }
}

impl DispatchEffect for AsyncContext {
    fn dispatcher(&self) -> &Dispatcher {
        &self.dispatcher
    }
}

impl EmitEvent for AsyncContext {
    fn event_bus(&self) -> &EventBus {
        &self.event_bus
    }
}

impl SpawnAsync for AsyncContext {
    fn tokio_rt(&self) -> &tokio::runtime::Runtime {
        &self.tokio_rt
    }
}
