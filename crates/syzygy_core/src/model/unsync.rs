use std::{
    any::{Any, TypeId},
    cell::{Ref, RefCell, RefMut},
    fmt,
    ops::Deref,
    rc::Rc,
};

use rustc_hash::FxHashMap;

use crate::permission::{ImpliedBy, HasPermission, Permission, RequiredPermission};

#[derive(Debug)]
pub struct Model(RefCell<Box<dyn Any + 'static>>);

impl Deref for Model {
    type Target = RefCell<Box<dyn Any + 'static>>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug, Default)]
pub struct ModelsBuilder(FxHashMap<TypeId, Box<dyn Any>>);

impl ModelsBuilder {
    #[must_use]
    pub fn insert<M>(mut self, model: M) -> Self
    where
        M: RequiredPermission + 'static,
    {
        self.0.insert(TypeId::of::<M>(), Box::new(model));
        self
    }
    #[must_use]
    pub fn build(self) -> Models {
        let models = self
            .0
            .into_iter()
            .map(|(id, model)| (id, Model(RefCell::new(model))))
            .collect();

        Models(Rc::new(models))
    }
}

#[derive(Debug, Clone)]
pub struct Models(Rc<FxHashMap<TypeId, Model>>);

impl Deref for Models {
    type Target = Rc<FxHashMap<TypeId, Model>>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Models {
    #[must_use]
    pub fn builder() -> ModelsBuilder {
        ModelsBuilder::default()
    }

    #[must_use]
    pub fn get<T>(&self) -> Option<Ref<T>>
    where
        T: 'static,
    {
        let ty = TypeId::of::<T>();
        self.0.get(&ty).map(|model| {
            Ref::map(model.borrow(), |boxed_value| {
                boxed_value.downcast_ref::<T>().expect("Type mismatch")
            })
        })
    }

    #[must_use]
    pub fn get_mut<T>(&self) -> Option<RefMut<T>>
    where
        T: 'static,
    {
        let ty = TypeId::of::<T>();
        self.0.get(&ty).map(|model| {
            RefMut::map(model.borrow_mut(), |boxed_value| {
                boxed_value.downcast_mut::<T>().expect("Type mismatch")
            })
        })
    }
}

pub trait ModelAccess {
    fn models(&self) -> &Models;
    fn model<M>(&self) -> Ref<M>
    where
        Self: HasPermission,
        M: RequiredPermission + 'static,
        M::Required: ImpliedBy<<Self as HasPermission>::Permission>,
    {
        self.models().get::<M>().unwrap()
    }
    fn try_model<M>(&self) -> Option<Ref<M>>
    where
        Self: HasPermission,
        M: RequiredPermission + 'static,
        M::Required: ImpliedBy<<Self as HasPermission>::Permission>,
    {
        self.models().get::<M>()
    }
    fn query<M, F, R>(&self, f: F) -> R
    where
        Self: HasPermission,
        M: RequiredPermission + 'static,
        M::Required: ImpliedBy<<Self as HasPermission>::Permission>,
        F: FnOnce(&M) -> R,
        R: 'static,
    {
        f(&self.model())
    }
}

pub trait ModelMut: ModelAccess {
    fn model_mut<M>(&self) -> RefMut<M>
    where
        Self: HasPermission,
        M: RequiredPermission + 'static,
        M::Required: ImpliedBy<<Self as HasPermission>::Permission>,
    {
        self.models().get_mut::<M>().unwrap()
    }
    fn try_model_mut<M>(&self) -> Option<RefMut<M>>
    where
        Self: HasPermission,
        M: RequiredPermission + 'static,
        M::Required: ImpliedBy<<Self as HasPermission>::Permission>,
    {
        self.models().get_mut::<M>()
    }
    fn update<M, F>(&self, mut f: F)
    where
        Self: HasPermission,
        M: RequiredPermission + 'static,
        M::Required: ImpliedBy<<Self as HasPermission>::Permission>,
        F: FnMut(&mut M),
    {
        f(&mut self.model_mut());
    }
}
