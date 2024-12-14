use std::{
    any::{Any, TypeId},
    cell::{Ref, RefCell, RefMut},
    fmt,
    ops::Deref,
    rc::Rc,
};

use rustc_hash::FxHashMap;

#[derive(Debug)]
pub struct Model(RefCell<Box<dyn Any>>);

impl Deref for Model {
    type Target = RefCell<Box<dyn Any>>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug, Default)]
pub struct ModelsBuilder(FxHashMap<TypeId, Box<dyn Any>>);

impl ModelsBuilder {
    #[must_use]
    pub fn insert<T>(mut self, model: T) -> Self
    where
        T: 'static,
    {
        self.0.insert(TypeId::of::<T>(), Box::new(model));
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
        M: fmt::Debug + 'static,
    {
        self.models().get::<M>().unwrap()
    }
    fn try_model<M>(&self) -> Option<Ref<M>>
    where
        M: fmt::Debug + 'static,
    {
        self.models().get::<M>()
    }
    fn query<M, F, R>(&self, f: F) -> R
    where
        M: fmt::Debug + 'static,
        F: FnOnce(&M) -> R,
        R: 'static,
    {
        f(&self.model())
    }
}

pub trait ModelMut: ModelAccess {
    fn model_mut<M>(&self) -> RefMut<M>
    where
        M: fmt::Debug + 'static,
    {
        self.models().get_mut::<M>().unwrap()
    }
    fn try_model_mut<M>(&self) -> Option<RefMut<M>>
    where
        M: fmt::Debug + 'static,
    {
        self.models().get_mut::<M>()
    }
    fn update<M, F>(&self, mut f: F)
    where
        M: fmt::Debug + 'static,
        F: FnMut(&mut M),
    {
        f(&mut self.model_mut());
    }
}
