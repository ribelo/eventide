use crate::{
    dispatch::{DispatchEffect, EffectSender},
    model::{Model, ModelSnapshotAccess, ModelSnapshotCreate},
    resource::{ResourceAccess, Resources},
};

use crate::spawn::SpawnThread;
#[cfg(feature = "parallel")]
use crate::spawn::{RayonPool, SpawnParallel};

use super::{Context, FromContext};

#[derive(Debug)]
pub struct ThreadContext<M: Model> {
    model_snapshot: M::Snapshot,
    resources: Resources,
    effect_sender: EffectSender<M>,
    #[cfg(feature = "parallel")]
    rayon_pool: RayonPool,
}

impl<M: Model> Clone for ThreadContext<M> {
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

impl<M: Model> Context for ThreadContext<M> {
    type Model = M;
}

impl<T, M: Model> FromContext<T> for ThreadContext<M>
where
    T: Context<Model = M>,
    T: ModelSnapshotCreate + ResourceAccess + DispatchEffect + SpawnThread,
{
    fn from_context(context: &T) -> Self {
        Self {
            model_snapshot: context.create_snapshot(),
            resources: context.resources().clone(),
            effect_sender: context.effect_sender().clone(),
        }
    }
}

impl<M: Model> ModelSnapshotAccess for ThreadContext<M> {
    fn snapshot(&self) -> &<<Self as Context>::Model as Model>::Snapshot {
        todo!()
    }
}

impl<M: Model> ModelSnapshotCreate for ThreadContext<M> {
    fn create_snapshot(&self) -> <<Self as Context>::Model as Model>::Snapshot {
        self.model_snapshot.clone()
    }
}

impl<M: Model> ResourceAccess for ThreadContext<M> {
    fn resources(&self) -> &Resources {
        &self.resources
    }
}

impl<M: Model> DispatchEffect for ThreadContext<M> {
    fn effect_sender(&self) -> &EffectSender<M> {
        &self.effect_sender
    }
}

impl<M: Model> SpawnThread for ThreadContext<M> {}

#[cfg(feature = "parallel")]
impl<M: Model> SpawnParallel<M> for ThreadContext<M> {
    fn rayon_pool(&self) -> &RayonPool {
        &self.rayon_pool
    }
}
