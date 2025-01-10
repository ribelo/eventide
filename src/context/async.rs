use crate::{
    model::{Model, ModelSnapshotAccess, ModelSnapshotCreate},
    resource::{ResourceAccess, Resources},
    spawn::{SpawnAsync, TokioHandle},
};

use bon::Builder;

use crate::dispatch::{DispatchEffect, EffectSender};

use super::{Context, FromContext};

#[derive(Debug, Builder)]
pub struct AsyncContext<M: Model> {
    pub model_snapshot: M::Snapshot,
    pub resources: Resources,
    pub effect_sender: EffectSender<M>,
    pub tokio_handle: TokioHandle,
}

impl<M: Model> Clone for AsyncContext<M> {
    fn clone(&self) -> Self {
        Self {
            model_snapshot: self.model_snapshot.clone(),
            resources: self.resources.clone(),
            effect_sender: self.effect_sender.clone(),
            tokio_handle: self.tokio_handle.clone(),
        }
    }
}

impl<M: Model> Context for AsyncContext<M> {
    type Model = M;
}

impl<T> FromContext<T> for AsyncContext<T::Model>
where
    T: Context,
    T: ModelSnapshotCreate + ResourceAccess + DispatchEffect + SpawnAsync,
{
    fn from_context(context: &T) -> Self {
        Self {
            model_snapshot: context.create_snapshot(),
            resources: context.resources().clone(),
            effect_sender: context.effect_sender().clone(),
            tokio_handle: context.tokio_handle().clone(),
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

impl<M: Model> DispatchEffect for AsyncContext<M> {
    fn effect_sender(&self) -> &EffectSender<M> {
        &self.effect_sender
    }
}

impl<M: Model> SpawnAsync for AsyncContext<M> {
    fn tokio_handle(&self) -> &TokioHandle {
        &self.tokio_handle
    }
}
