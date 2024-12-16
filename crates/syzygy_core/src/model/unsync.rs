use std::{
    any::{Any, TypeId},
    cell::{Ref, RefCell, RefMut},
    ops::Deref,
    rc::Rc,
};

use rustc_hash::FxHashMap;

use crate::context::{Context, BorrowFromContext};

#[derive(Debug)]
pub struct ModelBox(Rc<RefCell<Box<dyn Any + 'static>>>);

impl Deref for ModelBox {
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
        M: 'static,
    {
        self.0.insert(TypeId::of::<M>(), Box::new(model));
        self
    }
    #[must_use]
    pub fn build(self) -> Models {
        let models = self
            .0
            .into_iter()
            .map(|(id, model)| (id, ModelBox(Rc::new(RefCell::new(model)))))
            .collect();

        Models(Rc::new(models))
    }
}

#[derive(Debug, Clone)]
pub struct Models(Rc<FxHashMap<TypeId, ModelBox>>);

impl Deref for Models {
    type Target = Rc<FxHashMap<TypeId, ModelBox>>;
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

pub trait ModelAccess: Context {
    fn models(&self) -> &Models;
    fn model<M>(&self) -> Ref<M>
    where
        M: 'static,
    {
        self.models().get::<M>().unwrap()
    }
    fn try_model<M>(&self) -> Option<Ref<M>>
    where
        M: 'static,
    {
        self.models().get::<M>()
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
    fn model_mut<M>(&self) -> RefMut<M>
    where
        M: 'static,
    {
        self.models().get_mut::<M>().unwrap()
    }
    fn try_model_mut<M>(&self) -> Option<RefMut<M>>
    where
        M: 'static,
    {
        self.models().get_mut::<M>()
    }
    fn update<M, F>(&self, mut f: F)
    where
        M: 'static,
        F: FnMut(&mut M),
    {
        f(&mut self.model_mut());
    }
}

pub struct Model<'a, T>(pub Ref<'a, T>);

impl<'a, T> Deref for Model<'a, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'a, C, T> BorrowFromContext<'a, C> for Model<'a, T>
where
    C: Context + ModelAccess,
    T: 'static,
{
    fn from_context(context: &'a C) -> Self {
        Self(context.model::<T>())
    }
}

pub struct ModelMut<'a, T>(pub RefMut<'a, T>);

impl<'a, T> Deref for ModelMut<'a, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'a, C, T> BorrowFromContext<'a, C> for ModelMut<'a, T>
where
    C: Context + ModelModify,
    T: 'static,
{
    fn from_context(context: &'a C) -> Self {
        Self(context.model_mut::<T>())
    }
}
