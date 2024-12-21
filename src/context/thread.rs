use crate::{
    effect_bus::{DispatchEffect, EffectBus},
    resource::{ResourceAccess, Resources}, syzygy::Syzygy,
};

use crate::spawn::SpawnThread;
#[cfg(feature = "parallel")]
use crate::spawn::{RayonPool, SpawnParallel};

use super::{Context};

pub struct ThreadContext<M: 'static> {
    resources: Resources,
    effect_bus: EffectBus<M>,
    #[cfg(feature = "parallel")]
    rayon_pool: RayonPool,
}

impl<M> Clone for ThreadContext<M> {
    fn clone(&self) -> Self {
        Self {
            resources: self.resources.clone(),
            effect_bus: self.effect_bus.clone(),
            #[cfg(feature = "parallel")]
            rayon_pool: self.rayon_pool.clone(),
        }
    }
}

impl<M> Context for ThreadContext<M> {}

impl<M> From<Syzygy<M>> for ThreadContext<M> {
    fn from(syzygy: Syzygy<M>) -> Self {
        Self {
            resources: syzygy.resources,
            effect_bus: syzygy.effect_bus,
            #[cfg(feature = "parallel")]
            rayon_pool: syzygy.rayon_pool,
        }
    }
}

// #[cfg(not(feature = "parallel"))]
// impl<'a, C, M> FromContext<'a, C> for ThreadContext<M>
// where
//     C: ResourceAccess + DispatchEffect<M> + EmitEvent<M> + SpawnThread<'a, M> + 'static,
//     M: 'static,
// {
//     fn from_context(cx: &C) -> Self {
//         Self {
//             resources: cx.resources().clone(),
//             effect_bus: cx.effect_bus().clone(),
//             event_bus: cx.event_bus().clone(),
//         }
//     }
// }

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

impl<M: 'static> SpawnThread<M> for ThreadContext<M> {}

#[cfg(feature = "parallel")]
impl<M: 'static> SpawnParallel<M> for ThreadContext<M> {
    fn rayon_pool(&self) -> &RayonPool {
        &self.rayon_pool
    }
}
