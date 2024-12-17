use std::{
    any::{Any, TypeId},
    cell::{Ref, RefMut},
    fmt,
    ops::Deref,
};

use bon::Builder;

use generational_box::{
    AnyStorage, GenerationalBox, GenerationalRef, GenerationalRefMut, Owner, UnsyncStorage,
};
use rustc_hash::FxHashMap;

use crate::context::{Context, FromContext};

// impl UnsyncModelsBuilder {
//     #[must_use]
//     pub fn insert<M>(mut self, model: M) -> Self
//     where
//         M: 'static,
//     {
//         self.0.insert(TypeId::of::<M>(), Box::new(model));
//         self
//     }
//     #[must_use]
//     pub fn build(self) -> UnsyncModels {
//         let owner = UnsyncStorage::owner();
//         let models: FxHashMap<_, _> = self
//             .0
//             .into_iter()
//             .map(|(id, model)| (id, owner.insert(model)))
//             .collect();

//         UnsyncModels {
//             _owner: owner,
//             models,
//         }
//     }
// }

#[derive(Clone, Default, Builder)]
pub struct UnsyncModels {
    owner: Owner,
    models: FxHashMap<TypeId, GenerationalBox<Box<dyn Any>>>,
}

impl fmt::Debug for UnsyncModels {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Models")
            .field("models", &self.models)
            .finish_non_exhaustive()
    }
}

impl UnsyncModels {
    pub fn insert<M>(&mut self, model: M)
    where
        M: 'static,
    {
        self.models
            .insert(TypeId::of::<M>(), self.owner.insert(Box::new(model)));
    }

    #[must_use]
    pub fn try_get<M>(&self) -> Option<GenerationalRef<Ref<'static, M>>>
    where
        M: 'static,
    {
        let ty = TypeId::of::<M>();
        let gbox = self.models.get(&ty)?;
        let boxed_value = gbox.read();
        Some(UnsyncStorage::map(boxed_value, |any|
            // SAFETY: We verify the type matches via TypeId before calling downcast_ref_unchecked
            unsafe {
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
        Some(UnsyncStorage::map_mut(boxed_value, |any|
            // SAFETY: We verify the type matches via TypeId before calling downcast_ref_unchecked
            unsafe {
            any.downcast_mut_unchecked::<M>()
        }))
    }
}

pub trait UnsyncModelAccess: Context {
    fn models(&self) -> &UnsyncModels;
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

pub trait UnsyncModelModify: UnsyncModelAccess {
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

pub struct UnsyncModel<T: 'static>(pub GenerationalRef<Ref<'static, T>>);

impl<T> Deref for UnsyncModel<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<C, T> FromContext<C> for UnsyncModel<T>
where
    C: Context + UnsyncModelAccess,
    T: 'static,
{
    fn from_context(context: &C) -> Self {
        Self(context.model::<T>())
    }
}

pub struct UnsyncModelMut<T: 'static>(pub GenerationalRefMut<RefMut<'static, T>>);

impl<T> Deref for UnsyncModelMut<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<C, T> FromContext<C> for UnsyncModelMut<T>
where
    C: Context + UnsyncModelModify,
    T: 'static,
{
    fn from_context(context: &C) -> Self {
        Self(context.model_mut::<T>())
    }
}
