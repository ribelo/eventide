use std::sync::Arc;

use crate::{
    dispatch::{DispatchEffect, Dispatcher},
    event_bus::{EmitEvent, EventBus},
    resource::{ResourceAccess, Resources},
    spawn::SpawnAsync,
};

use super::{BorrowFromContext, Context, FromContext};

#[derive(Debug, Clone)]
pub struct AsyncContext<'a> {
    resources: Resources,
    dispatcher: Dispatcher<'a>,
    event_bus: EventBus,
    tokio_rt: Arc<tokio::runtime::Runtime>,
}

impl<'a> Context for AsyncContext<'a> {}

impl<'a, C> FromContext<C> for AsyncContext<'a>
where
    C: ResourceAccess + DispatchEffect<'a> + EmitEvent + SpawnAsync + 'static,
{
    fn from_context(cx: &C) -> Self {
        Self {
            resources: <C as ResourceAccess>::resources(cx).clone(),
            dispatcher: <C as DispatchEffect>::dispatcher(cx).clone(),
            event_bus: <C as EmitEvent>::event_bus(cx).clone(),
            tokio_rt: <C as SpawnAsync>::tokio_rt(cx).clone(),
        }
    }
}

impl<'a> ResourceAccess for AsyncContext<'a> {
    fn resources(&self) -> &Resources {
        &self.resources
    }
}

impl<'a> DispatchEffect<'a> for AsyncContext<'a> {
    fn dispatcher(&'a self) -> &'a Dispatcher<'a> {
        &self.dispatcher
    }
}

impl<'a> EmitEvent for AsyncContext<'a> {
    fn event_bus(&self) -> &EventBus {
        &self.event_bus
    }
}

impl<'a> SpawnAsync for AsyncContext<'a> {
    fn tokio_rt(&self) -> Arc<tokio::runtime::Runtime> {
        Arc::clone(&self.tokio_rt)
    }
}
