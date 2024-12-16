use std::sync::Arc;

use crate::{
    dispatch::{DispatchEffect, Dispatcher},
    event_bus::{EmitEvent, EventBus},
    resource::{ResourceAccess, Resources},
    spawn::SpawnAsync,
    syzygy::Syzygy,
};

#[cfg(feature = "role")]
use crate::role::{RoleHolder, Root};

use super::{Context, FromContext};

#[derive(Debug, Clone)]
pub struct AsyncContext {
    resources: Resources,
    dispatcher: Dispatcher,
    event_bus: EventBus,
    tokio_rt: Arc<tokio::runtime::Runtime>,
}

#[cfg(feature = "role")]
impl RoleHolder for AsyncContext {
    type Role = Root;
}

impl Context for AsyncContext {}

impl<C> FromContext<C> for AsyncContext
where
    C: ResourceAccess + DispatchEffect + EmitEvent + SpawnAsync + 'static,
{
    fn from_context(cx: C) -> Self {
        Self {
            resources: <C as ResourceAccess>::resources(&cx).clone(),
            dispatcher: <C as DispatchEffect>::dispatcher(&cx).clone(),
            event_bus: <C as EmitEvent>::event_bus(&cx).clone(),
            tokio_rt: <C as SpawnAsync>::tokio_rt(&cx).clone(),
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
    fn tokio_rt(&self) -> Arc<tokio::runtime::Runtime> {
        Arc::clone(&self.tokio_rt)
    }
}
