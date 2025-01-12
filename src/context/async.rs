use std::sync::Arc;

use crate::{
    dispatch::EffectsQueue, model::{Model, ModelSnapshotAccess, ModelSnapshotCreate}, resource::{ResourceAccess, Resources}, syzygy::Syzygy
};

use bon::Builder;


use super::{Context, FromContext};

#[derive(Debug, Builder)]
pub struct AsyncContext<M: Model> {
    pub model_snapshot: M::Snapshot,
    pub resources: Resources,
    pub effects_queue: EffectsQueue<M>,
}

impl<M: Model> Context for AsyncContext<M> {
    type Model = M;
}

impl<M: Model> Clone for AsyncContext<M> {
    fn clone(&self) -> Self {
        Self {
            model_snapshot: self.model_snapshot.clone(),
            resources: self.resources.clone(),
            effects_queue: self.effects_queue.clone(),
        }
    }
}

impl<M: Model> FromContext<Syzygy<M>> for AsyncContext<M> {
    fn from_context(context: &Syzygy<M>) -> Self {
        Self {
            model_snapshot: context.model.into_snapshot(),
            resources: context.resources().clone(),
            effects_queue: context.effects_queue.clone(),
        }
    }
}

impl<M: Model> ModelSnapshotAccess for AsyncContext<M> {
    fn snapshot(&self) -> &<<Self as Context>::Model as Model>::Snapshot {
        &self.model_snapshot
    }
}

impl<M: Model> ModelSnapshotCreate for AsyncContext<M> {
    fn create_snapshot(&self) -> <<Self as Context>::Model as Model>::Snapshot {
        self.model_snapshot.clone()
    }
}

impl<M: Model> ResourceAccess for AsyncContext<M> {
    fn resources(&self) -> &Resources {
        &self.resources
    }
}
