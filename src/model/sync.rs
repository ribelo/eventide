use std::{
    any::{Any, TypeId},
    fmt,
    ops::Deref,
};

use generational_box::{
    AnyStorage, GenerationalBox, GenerationalRef, GenerationalRefMut, Owner, SyncStorage,
};
use parking_lot::{MappedRwLockReadGuard, MappedRwLockWriteGuard};
use rustc_hash::FxHashMap;

use crate::context::{Context, FromContext};

#[derive(Debug, Default)]
pub struct SyncModelsBuilder(FxHashMap<TypeId, Box<dyn Any + Send + Sync>>);

impl SyncModelsBuilder {
    #[must_use]
    pub fn insert<M>(mut self, model: M) -> Self
    where
        M: Send + Sync + 'static,
    {
        self.0.insert(TypeId::of::<M>(), Box::new(model));
        self
    }
    #[must_use]
    pub fn build(self) -> SyncModels {
        let owner = SyncStorage::owner();
        let models: FxHashMap<_, _> = self
            .0
            .into_iter()
            .map(|(id, model)| (id, owner.insert(model)))
            .collect();

        SyncModels {
            _owner: owner,
            models,
        }
    }
}

#[derive(Clone)]
pub struct SyncModels {
    _owner: Owner<SyncStorage>,
    models: FxHashMap<TypeId, GenerationalBox<Box<dyn Any + Send + Sync>, SyncStorage>>,
}

impl fmt::Debug for SyncModels {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Models")
            .field("models", &self.models)
            .finish_non_exhaustive()
    }
}

impl SyncModels {
    #[must_use]
    pub fn builder() -> SyncModelsBuilder {
        SyncModelsBuilder::default()
    }

    #[must_use]
    pub fn try_get<M>(&self) -> Option<GenerationalRef<MappedRwLockReadGuard<'static, M>>>
    where
        M: 'static,
    {
        let ty = TypeId::of::<M>();
        let gbox = self.models.get(&ty)?;
        let boxed_value = gbox.read();
        Some(SyncStorage::map(boxed_value, |any|
            // SAFETY: We verify the type matches via TypeId before calling downcast_ref_unchecked
            unsafe {
            any.downcast_ref_unchecked::<M>()
        }))
    }

    #[must_use]
    pub fn try_get_mut<M>(&self) -> Option<GenerationalRefMut<MappedRwLockWriteGuard<'static, M>>>
    where
        M: 'static,
    {
        let ty = TypeId::of::<M>();
        let gbox = self.models.get(&ty)?;
        let boxed_value = gbox.write();
        Some(SyncStorage::map_mut(boxed_value, |any|
            // SAFETY: We verify the type matches via TypeId before calling downcast_ref_unchecked
            unsafe {
            any.downcast_mut_unchecked::<M>()
        }))
    }
}

pub trait SyncModelAccess: Context {
    fn models(&self) -> &SyncModels;
    fn model<M>(&self) -> GenerationalRef<MappedRwLockReadGuard<'static, M>>
    where
        M: 'static,
    {
        self.models().try_get::<M>().unwrap()
    }
    fn try_model<M>(&self) -> Option<GenerationalRef<MappedRwLockReadGuard<'static, M>>>
    where
        M: 'static,
    {
        self.models().try_get::<M>()
    }
    fn query<M, F, R>(&self, f: F) -> R
    where
        M: 'static,
        F: FnOnce(&M) -> R,
        R: 'static,
    {
        f(&self.model())
    }
}

pub trait SyncModelModify: SyncModelAccess {
    fn model_mut<M>(&self) -> GenerationalRefMut<MappedRwLockWriteGuard<'static, M>>
    where
        M: 'static,
    {
        self.models().try_get_mut::<M>().unwrap()
    }
    fn try_model_mut<M>(&self) -> Option<GenerationalRefMut<MappedRwLockWriteGuard<'static, M>>>
    where
        M: 'static,
    {
        self.models().try_get_mut::<M>()
    }
    fn update<M, F>(&self, mut f: F)
    where
        M: 'static,
        F: FnMut(&mut M),
    {
        f(&mut self.model_mut());
    }
}

pub struct SyncModel<M: 'static>(pub GenerationalRef<MappedRwLockReadGuard<'static, M>>);

impl<T> Deref for SyncModel<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<C, T> FromContext<C> for SyncModel<T>
where
    C: Context + SyncModelAccess,
    T: 'static,
{
    fn from_context(context: &C) -> Self {
        Self(context.model::<T>())
    }
}

pub struct SyncModelMut<M: 'static>(pub GenerationalRefMut<MappedRwLockWriteGuard<'static, M>>);

impl<T> Deref for SyncModelMut<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<C, T> FromContext<C> for SyncModelMut<T>
where
    C: Context + SyncModelModify,
    T: 'static,
{
    fn from_context(context: &C) -> Self {
        Self(context.model_mut::<T>())
    }
}
