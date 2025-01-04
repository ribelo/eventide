use crate::{
    effect_bus::{DispatchEffect, Effect, EffectBus}, model::{Model, ModelAccess}, resource::{ResourceAccess, Resources}, syzygy::Syzygy
};

use crate::spawn::SpawnThread;
#[cfg(feature = "parallel")]
use crate::spawn::{RayonPool, SpawnParallel};

use super::Context;

#[derive(Debug)]
pub struct ThreadContext<M: Model, E: Effect<M>> {
    model_snapshot: M,
    resources: Resources,
    effect_bus: EffectBus<M, E>,
    #[cfg(feature = "parallel")]
    rayon_pool: RayonPool,
}

impl<M: Model, E: Effect<M>> Clone for ThreadContext<M, E> {
fn clone(&self) -> Self {
    Self {
        model_snapshot: self.model_snapshot.clone(),
        resources: self.resources.clone(),
        effect_bus: self.effect_bus.clone(),
        #[cfg(feature = "parallel")]
        rayon_pool: self.rayon_pool.clone(),
    }
}
}

impl<M: Model, E: Effect<M>> Context for ThreadContext<M, E> {}

impl<M: Model, E: Effect<M>> From<Syzygy<M, E>> for ThreadContext<M, E> {
    fn from(syzygy: Syzygy<M, E>) -> Self {
        Self {
            resources: syzygy.resources,
            effect_bus: syzygy.effect_bus,
            model_snapshot: syzygy.model.clone(),
            #[cfg(feature = "parallel")]
            rayon_pool: syzygy.rayon_pool,
        }
    }
}

impl<M: Model, E: Effect<M>> ModelAccess<M> for ThreadContext<M, E> {
    fn model(&self) -> &M {
        &self.model_snapshot
    }
}

impl<M: Model, E: Effect<M>> ResourceAccess for ThreadContext<M, E> {
    fn resources(&self) -> &Resources {
        &self.resources
    }
}

impl<M: Model, E: Effect<M>> DispatchEffect<M, E> for ThreadContext<M, E> {
    fn effect_bus(&self) -> &EffectBus<M, E> {
        &self.effect_bus
    }
}

impl<M: Model, E: Effect<M>> SpawnThread<M, E> for ThreadContext<M, E> {}

#[cfg(feature = "parallel")]
impl<M: Model, E: Effect<M>> SpawnParallel<M, E> for ThreadContext<M, E> {
    fn rayon_pool(&self) -> &RayonPool {
        &self.rayon_pool
    }
}
