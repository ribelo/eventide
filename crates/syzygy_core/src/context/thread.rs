#[cfg(feature = "parallel")]
use std::sync::Arc;

use crate::{
    effect_bus::{DispatchEffect, EffectBus},
    event_bus::{EmitEvent, EventBus},
    resource::{ResourceAccess, Resources},
};

#[cfg(feature = "parallel")]
use crate::spawn::SpawnParallel;
#[cfg(not(feature = "parallel"))]
use crate::spawn::SpawnThread;

use super::{Context, FromContext};

#[derive(Clone)]
pub struct ThreadContext {
    resources: Resources,
    effect_bus: EffectBus,
    event_bus: EventBus,
    #[cfg(feature = "parallel")]
    rayon_pool: Arc<rayon::ThreadPool>,
}

impl Context for ThreadContext {}

#[cfg(not(feature = "parallel"))]
impl<C> FromContext<C> for ThreadContext
where
    C: ResourceAccess + DispatchEffect + EmitEvent + SpawnThread + 'static,
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
impl<C> FromContext<C> for ThreadContext
where
    C: ResourceAccess + DispatchEffect + EmitEvent + SpawnParallel + 'static,
{
    fn from_context(cx: &C) -> Self {
        Self {
            resources: cx.resources().clone(),
            effect_bus: cx.effect_bus().clone(),
            event_bus: cx.event_bus().clone(),
            rayon_pool: cx.rayon_pool(),
        }
    }
}

impl ResourceAccess for ThreadContext {
    fn resources(&self) -> &Resources {
        &self.resources
    }
}

impl DispatchEffect for ThreadContext {
    fn effect_bus(&self) -> &EffectBus {
        &self.effect_bus
    }
}

impl EmitEvent for ThreadContext {
    fn event_bus(&self) -> &EventBus {
        &self.event_bus
    }
}

#[cfg(feature = "parallel")]
impl SpawnParallel for ThreadContext {
    fn rayon_pool(&self) -> Arc<rayon::ThreadPool> {
        Arc::clone(&self.rayon_pool)
    }
}
