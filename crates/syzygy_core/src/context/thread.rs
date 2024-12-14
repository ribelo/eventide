use crate::{
    dispatch::{DispatchEffect, Dispatcher},
    event_bus::{EmitEvent, EventBus},
    resource::{ResourceAccess, Resources},
    spawn::{SpawnThread, TaskQueue},
    syzygy::Syzygy,
};

#[cfg(feature = "parallel")]
use crate::spawn::{ParallelTaskQueue, SpawnParallel};

use super::{Context, FromContext};

#[derive(Clone)]
pub struct ThreadContext {
    resources: Resources,
    dispatcher: Dispatcher,
    event_bus: EventBus,
    task_queue: TaskQueue,
    #[cfg(feature = "parallel")]
    parallel_task_queue: ParallelTaskQueue,
}

impl Context for ThreadContext {}

impl FromContext<Syzygy> for ThreadContext {
    fn from_context(cx: Syzygy) -> Self {
        Self {
            dispatcher: cx.dispatcher.clone(),
            resources: cx.resources.clone(),
            event_bus: cx.event_bus.clone(),
            task_queue: cx.task_queue.clone(),
            #[cfg(feature = "parallel")]
            parallel_task_queue: cx.parallel_task_queue.clone(),
        }
    }
}

impl ResourceAccess for ThreadContext {
    fn resources(&self) -> &Resources {
        &self.resources
    }
}

impl DispatchEffect for ThreadContext {
    fn dispatcher(&self) -> &Dispatcher {
        &self.dispatcher
    }
}

impl EmitEvent for ThreadContext {
    fn event_bus(&self) -> &EventBus {
        &self.event_bus
    }
}

impl SpawnThread for ThreadContext {
    fn task_queue(&self) -> &TaskQueue {
        &self.task_queue
    }
}

#[cfg(feature = "parallel")]
impl SpawnParallel for ThreadContext {
    fn parallel_task_queue(&self) -> &ParallelTaskQueue {
        &self.parallel_task_queue
    }
}
