use bon::Builder;
use tokio::sync::oneshot;

use crate::{
    dispatch::{CompletionNotifier, DispatchEffect, EffectSender, EffectStatus},
    model::{Model, ModelAccess, ModelModify},
    prelude::{ResourceAccess, ResourceModify, Resources},
    spawn::{SpawnAsync, SpawnThread},
    syzygy::Syzygy,
};

use super::Context;

#[derive(Builder)]
pub struct EffectContext<'a, M: Model> {
    syzygy: &'a mut Syzygy<M>,
    #[builder(with = |tx: Option<oneshot::Sender<EffectStatus>>| CompletionNotifier(tx))]
    notifer: CompletionNotifier,
}

impl<'a, M: Model> EffectContext<'a, M> {
    pub(crate) fn new(syzygy: &'a mut Syzygy<M>, tx: oneshot::Sender<EffectStatus>) -> EffectContext<'a, M> {
        Self {
            syzygy,
            notifer: CompletionNotifier(Some(tx)),
        }
    }
    pub fn notify(&mut self) -> CompletionNotifier {
        std::mem::take(&mut self.notifer)
    }
}

impl<M: Model> Context for EffectContext<'_, M> {
    type Model = M;
}

impl<M: Model> DispatchEffect for EffectContext<'_, M> {
    fn effect_sender(&self) -> &EffectSender<Self::Model> {
        &self.syzygy.effect_bus.effect_sender
    }
}

impl<M: Model> ModelAccess for EffectContext<'_, M> {
    fn model(&self) -> &M {
        self.syzygy.model()
    }
}

impl<M: Model> ModelModify for EffectContext<'_, M> {
    fn model_mut(&mut self) -> &mut M {
        self.syzygy.model_mut()
    }
}

impl<M: Model> ResourceAccess for EffectContext<'_, M> {
    fn resources(&self) -> &Resources {
        self.syzygy.resources()
    }
}

impl<M: Model> ResourceModify for EffectContext<'_, M> {}

impl<M: Model> SpawnThread for EffectContext<'_, M> {}

impl<M: Model> SpawnAsync for EffectContext<'_, M> {
    fn tokio_handle(&self) -> &crate::prelude::TokioHandle {
        self.syzygy.tokio_handle()
    }
}
