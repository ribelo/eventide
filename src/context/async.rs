use std::sync::Arc;

use crate::{
    dispatch::EffectsTx,
    model::{Model, ModelSnapshotAccess, ModelSnapshotCreate},
    prelude::DispatchEffect,
    resource::{ResourceAccess, Resources},
    syzygy::Syzygy,
};

use bon::Builder;

use super::{Context, FromContext};

#[derive(Debug, Builder)]
pub struct AsyncContext<M: Model> {
    model_snapshot: Arc<M::Snapshot>,
    resources: Resources,
    effects_tx: EffectsTx<M>,
}

impl<M: Model> Context for AsyncContext<M> {
    type Model = M;
}

impl<M: Model> Clone for AsyncContext<M> {
    fn clone(&self) -> Self {
        Self {
            model_snapshot: self.model_snapshot.clone(),
            resources: self.resources.clone(),
            effects_tx: self.effects_tx.clone(),
        }
    }
}

impl<M: Model> FromContext<Syzygy<M>> for AsyncContext<M> {
    fn from_context(context: &Syzygy<M>) -> Self {
        Self {
            model_snapshot: Arc::new(context.model.to_snapshot()),
            resources: context.resources().clone(),
            effects_tx: context.effects_bus.tx.clone(),
        }
    }
}

// impl<'a, M: Model> FromContext<'a, EffectContext<'a, M>> for AsyncContext<M> {
//     fn from_context(context: &'a mut EffectContext<'a, M>) -> Self {
//         Self {
//             model_snapshot: context.ctx.model.into_snapshot(),
//             resources: context.resources().clone(),
//         }
//     }
// }

impl<M: Model> ModelSnapshotAccess for AsyncContext<M> {
    fn snapshot(&self) -> &<<Self as Context>::Model as Model>::Snapshot {
        &self.model_snapshot
    }
}

impl<M: Model> ModelSnapshotCreate for AsyncContext<M> {
    fn create_snapshot(&self) -> <M as Model>::Snapshot {
        (*self.model_snapshot).clone()
    }
}

impl<M: Model> ResourceAccess for AsyncContext<M> {
    fn resources(&self) -> &Resources {
        &self.resources
    }
}

impl<M: Model> DispatchEffect for AsyncContext<M> {
    fn effects_tx(&self) -> &EffectsTx<M> {
        &self.effects_tx
    }
}
