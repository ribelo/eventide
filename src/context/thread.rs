use crate::{
    effects::{Effect, EffectSender, SendEffect},
    model::{Model, ModelAccess},
    resource::{ResourceAccess, Resources},
};

use crate::spawn::SpawnThread;
#[cfg(feature = "parallel")]
use crate::spawn::{RayonPool, SpawnParallel};

use super::{Context, FromContext};

#[derive(Debug)]
pub struct ThreadContext<M: Model, E: Effect<M>> {
    model_snapshot: M,
    resources: Resources,
    effect_sender: EffectSender<M, E>,
    #[cfg(feature = "parallel")]
    rayon_pool: RayonPool,
}

impl<M: Model, E: Effect<M>> Clone for ThreadContext<M, E> {
    fn clone(&self) -> Self {
        Self {
            model_snapshot: self.model_snapshot.clone(),
            resources: self.resources.clone(),
            effect_sender: self.effect_sender.clone(),
            #[cfg(feature = "parallel")]
            rayon_pool: self.rayon_pool.clone(),
        }
    }
}

impl<M: Model, E: Effect<M>> Context for ThreadContext<M, E> {
    type Model = M;
    type Effect = E;
}

impl<T, M: Model, E: Effect<M>> FromContext<T> for ThreadContext<M, E>
where
    T: Context<Model = M, Effect = E>,
    T: ModelAccess + ResourceAccess + SendEffect + SpawnThread,
{
    fn from_context(context: &T) -> Self {
        Self {
            model_snapshot: context.model().clone(),
            resources: context.resources().clone(),
            effect_sender: context.effect_sender().clone(),
        }
    }
}

impl<M: Model, E: Effect<M>> ModelAccess for ThreadContext<M, E> {
    fn model(&self) -> &M {
        &self.model_snapshot
    }
}

impl<M: Model, E: Effect<M>> ResourceAccess for ThreadContext<M, E> {
    fn resources(&self) -> &Resources {
        &self.resources
    }
}

impl<M: Model, E: Effect<M>> SendEffect for ThreadContext<M, E> {
    fn effect_sender(&self) -> &EffectSender<M, E> {
        &self.effect_sender
    }
}

impl<M: Model, E: Effect<M>> SpawnThread for ThreadContext<M, E> {}

#[cfg(feature = "parallel")]
impl<M: Model, E: Effect<M>> SpawnParallel<M, E> for ThreadContext<M, E> {
    fn rayon_pool(&self) -> &RayonPool {
        &self.rayon_pool
    }
}
