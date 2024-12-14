use crate::{
    dispatch::{DispatchEffect, Dispatcher},
    event_bus::{EmitEvent, EventBus},
    resource::{ResourceAccess, Resources},
    spawn::{AsyncTaskQueue, SpawnAsync},
    syzygy::Syzygy,
};

use super::{Context, FromContext};

#[derive(Debug, Clone)]
pub struct AsyncContext {
    resources: Resources,
    dispatcher: Dispatcher,
    event_bus: EventBus,
    async_task_queue: AsyncTaskQueue,
}

impl Context for AsyncContext {}

impl FromContext<Syzygy> for AsyncContext {
    fn from_context(cx: Syzygy) -> Self {
        Self {
            dispatcher: cx.dispatcher.clone(),
            resources: cx.resources.clone(),
            event_bus: cx.event_bus.clone(),
            async_task_queue: cx.async_task_queue.clone(),
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
    fn async_task_queue(&self) -> &AsyncTaskQueue {
        &self.async_task_queue
    }
}
