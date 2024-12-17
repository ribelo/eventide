use std::{
    any::{Any, TypeId},
    cell::{Ref, RefMut},
    fmt,
    ops::Deref,
};

use generational_box::{
    AnyStorage, GenerationalBox, GenerationalRef, GenerationalRefMut, Owner, UnsyncStorage,
};
use rustc_hash::FxHashMap;

use crate::context::{Context, FromContext};

#[derive(Debug, Default)]
pub struct ModelsBuilder(FxHashMap<TypeId, Box<dyn Any>>);

impl ModelsBuilder {
    #[must_use]
    pub fn insert<M>(mut self, model: M) -> Self
    where
        M: 'static,
    {
        self.0.insert(TypeId::of::<M>(), Box::new(model));
        self
    }
    #[must_use]
    pub fn build(self) -> Models {
        let owner = UnsyncStorage::owner();
        let models: FxHashMap<_, _> = self
            .0
            .into_iter()
            .map(|(id, model)| (id, owner.insert(model)))
            .collect();

        Models {
            _owner: owner,
            models,
        }
    }
}

#[derive(Clone)]
pub struct Models {
    _owner: Owner,
    models: FxHashMap<TypeId, GenerationalBox<Box<dyn Any>>>,
}

impl fmt::Debug for Models {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Models")
            .field("models", &self.models)
            .finish_non_exhaustive()
    }
}

impl Models {
    #[must_use]
    pub fn builder() -> ModelsBuilder {
        ModelsBuilder::default()
    }

    #[must_use]
    pub fn try_get<M>(&self) -> Option<GenerationalRef<Ref<'static, M>>>
    where
        M: 'static,
    {
        let ty = TypeId::of::<M>();
        let gbox = self.models.get(&ty)?;
        let boxed_value = gbox.read();
        Some(UnsyncStorage::map(boxed_value, |any| unsafe {
            any.downcast_ref_unchecked::<M>()
        }))
    }

    #[must_use]
    pub fn try_get_mut<M>(&self) -> Option<GenerationalRefMut<RefMut<'static, M>>>
    where
        M: 'static,
    {
        let ty = TypeId::of::<M>();
        let gbox = self.models.get(&ty)?;
        let boxed_value = gbox.write();
        Some(UnsyncStorage::map_mut(boxed_value, |any| unsafe {
            any.downcast_mut_unchecked::<M>()
        }))
    }
}

pub trait ModelAccess: Context {
    fn models(&self) -> &Models;
    fn model<M>(&self) -> GenerationalRef<Ref<'static, M>>
    where
        M: 'static,
    {
        self.models().try_get::<M>().unwrap()
    }
    fn try_model<M>(&self) -> Option<GenerationalRef<Ref<'static, M>>>
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

pub trait ModelModify: ModelAccess {
    fn model_mut<M>(&self) -> GenerationalRefMut<RefMut<'static, M>>
    where
        M: 'static,
    {
        self.models().try_get_mut::<M>().unwrap()
    }
    fn try_model_mut<M>(&self) -> Option<GenerationalRefMut<RefMut<'static, M>>>
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

pub struct Model<T: 'static>(pub GenerationalRef<Ref<'static, T>>);

impl<T> Deref for Model<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<C, T> FromContext<C> for Model<T>
where
    C: Context + ModelAccess,
    T: 'static,
{
    fn from_context(context: &C) -> Self {
        Self(context.model::<T>())
    }
}

pub struct ModelMut<T: 'static>(pub GenerationalRefMut<RefMut<'static, T>>);

impl<T> Deref for ModelMut<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<C, T> FromContext<C> for ModelMut<T>
where
    C: Context + ModelModify,
    T: 'static,
{
    fn from_context(context: &C) -> Self {
        Self(context.model_mut::<T>())
    }
}
