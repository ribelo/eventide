use crate::{
    effect_bus::{DispatchEffect, EffectBus}, model::{Model, ModelAccess}, resource::{ResourceAccess, Resources}, syzygy::Syzygy
};

use crate::spawn::SpawnThread;
#[cfg(feature = "parallel")]
use crate::spawn::{RayonPool, SpawnParallel};

use super::Context;

#[derive(Debug, Clone)]
pub struct ThreadContext<M: Model> {
    resources: Resources,
    effect_bus: EffectBus<M>,
    model_snapshot: M,
    #[cfg(feature = "parallel")]
    rayon_pool: RayonPool,
}


impl<M: Model> Context for ThreadContext<M> {}

impl<M: Model> From<Syzygy<M>> for ThreadContext<M> {
    fn from(syzygy: Syzygy<M>) -> Self {
        Self {
            resources: syzygy.resources,
            effect_bus: syzygy.effect_bus,
            model_snapshot: syzygy.model.clone(),
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

impl<M: Model> ModelAccess<M> for ThreadContext<M> {
    fn model(&self) -> &M {
        &self.model_snapshot
    }
}

impl<M: Model> ResourceAccess for ThreadContext<M> {
    fn resources(&self) -> &Resources {
        &self.resources
    }
}

impl<M: Model> DispatchEffect<M> for ThreadContext<M> {
    fn effect_bus(&self) -> &EffectBus<M> {
        &self.effect_bus
    }
}

impl<M: Model> SpawnThread<M> for ThreadContext<M> {}

#[cfg(feature = "parallel")]
impl<M: Model> SpawnParallel<M> for ThreadContext<M> {
    fn rayon_pool(&self) -> &RayonPool {
        &self.rayon_pool
    }
}
