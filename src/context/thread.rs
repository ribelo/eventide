use crate::{
    effect_bus::{DispatchEffect, EffectBus},
    event_bus::{EmitEvent, EventBus},
    resource::{ResourceAccess, Resources},
};

use crate::spawn::SpawnThread;
#[cfg(feature = "parallel")]
use crate::spawn::{RayonPool, SpawnParallel};

use super::{Context, FromContext};

#[derive(Clone)]
pub struct ThreadContext<M: 'static> {
    resources: Resources,
    effect_bus: EffectBus<M>,
    event_bus: EventBus<M>,
    #[cfg(feature = "parallel")]
    rayon_pool: RayonPool,
}

impl<M> Context for ThreadContext<M> {}

#[cfg(not(feature = "parallel"))]
impl<'a, C, M> FromContext<'a, C> for ThreadContext<M>
where
    C: ResourceAccess + DispatchEffect<M> + EmitEvent<M> + SpawnThread<'a, M> + 'static,
    M: 'static,
{
    fn from_context(cx: &C) -> Self {
        Self {
            resources: cx.resources().clone(),
            effect_bus: cx.effect_bus().clone(),
            event_bus: cx.event_bus().clone(),
        }
    }
}

#[cfg(feature = "parallel")]
impl<'a, C, M> FromContext<'a, C> for ThreadContext<M>
where
    C: ResourceAccess + DispatchEffect<M> + EmitEvent<M> + SpawnThread<'a, M> + SpawnParallel<'a, M> +'static,
    M: 'static,
{
    fn from_context(cx: &'a C) -> Self {
        Self {
            resources: cx.resources().clone(),
            effect_bus: cx.effect_bus().clone(),
            event_bus: cx.event_bus().clone(),
            rayon_pool: cx.rayon_pool().clone(),
        }
    }
}

impl<M> ResourceAccess for ThreadContext<M> {
    fn resources(&self) -> &Resources {
        &self.resources
    }
}

impl<M> DispatchEffect<M> for ThreadContext<M> {
    fn effect_bus(&self) -> &EffectBus<M> {
        &self.effect_bus
    }
}

impl<M> EmitEvent<M> for ThreadContext<M> {
    fn event_bus(&self) -> &EventBus<M> {
        &self.event_bus
    }
}

impl<M: 'static> SpawnThread<'_, M> for ThreadContext<M> {}

#[cfg(feature = "parallel")]
impl<'a, M: 'static> SpawnParallel<'a, M> for ThreadContext<M> {
    fn rayon_pool(&'a self) -> &'a RayonPool {
        &self.rayon_pool
    }
}
