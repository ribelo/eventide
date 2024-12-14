use std::sync::Arc;

use crate::{
    dispatch::{DispatchEffect, Dispatcher},
    event_bus::{EmitEvent, EventBus},
    resource::{ResourceAccess, Resources},
    spawn::SpawnThread,
    syzygy::Syzygy,
};

#[cfg(feature = "parallel")]
use crate::spawn::SpawnParallel;

use super::{Context, FromContext};

#[derive(Clone)]
pub struct ThreadContext {
    resources: Resources,
    dispatcher: Dispatcher,
    event_bus: EventBus,
    #[cfg(feature = "parallel")]
    rayon_pool: Arc<rayon::ThreadPool>,
}

impl Context for ThreadContext {}

impl FromContext<ThreadContext> for ThreadContext {
    fn from_context(cx: ThreadContext) -> Self {
        cx
    }
}

impl FromContext<Syzygy> for ThreadContext {
    fn from_context(cx: Syzygy) -> Self {
        Self {
            dispatcher: cx.dispatcher.clone(),
            resources: cx.resources.clone(),
            event_bus: cx.event_bus.clone(),
            #[cfg(feature = "parallel")]
            rayon_pool: Arc::clone(&cx.rayon_pool),
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

impl SpawnThread for ThreadContext {}

#[cfg(feature = "parallel")]
impl SpawnParallel for ThreadContext {
    fn rayon_pool(&self) -> &rayon::ThreadPool {
        &self.rayon_pool
    }
}
