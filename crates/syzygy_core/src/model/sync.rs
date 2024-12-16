// use std::{
//     any::{Any, TypeId},
//     ops::Deref,
//     sync::Arc,
// };

// use parking_lot::{
//     MappedRwLockReadGuard, MappedRwLockWriteGuard, RwLock, RwLockReadGuard, RwLockWriteGuard,
// };
// use rustc_hash::FxHashMap;

// use crate::context::{BorrowFromContext, Context, FromContext};

// #[derive(Debug)]
// pub struct ModelBox(RwLock<Box<dyn Any + 'static>>);

// impl ModelBox {
//     pub fn downcast_ref<T: 'static>(&self) -> MappedRwLockReadGuard<'_, T> {
//         RwLockReadGuard::map(self.0.read(), |boxed_value| {
//             boxed_value.downcast_ref::<T>().expect("Type mismatch")
//         })
//     }
//     pub fn downcast_mut<T: 'static>(&self) -> MappedRwLockWriteGuard<'_, T> {
//         RwLockWriteGuard::map(self.0.write(), |boxed_value| unsafe {
//             boxed_value.downcast_mut_unchecked::<T>()
//         })
//     }
// }

// impl Deref for ModelBox {
//     type Target = RwLock<Box<dyn Any + 'static>>;
//     fn deref(&self) -> &Self::Target {
//         &self.0
//     }
// }

// #[derive(Debug, Default)]
// pub struct ModelsBuilder(FxHashMap<TypeId, Box<dyn Any>>);

// impl ModelsBuilder {
//     #[must_use]
//     pub fn insert<M>(mut self, model: M) -> Self
//     where
//         M: 'static,
//     {
//         self.0.insert(TypeId::of::<M>(), Box::new(model));
//         self
//     }
//     #[must_use]
//     pub fn build(self) -> Models {
//         let models = self
//             .0
//             .into_iter()
//             .map(|(id, model)| (id, ModelBox(RwLock::new(model))))
//             .collect();

//         Models(Arc::new(models))
//     }
// }

// #[derive(Debug, Clone)]
// pub struct Models(Arc<FxHashMap<TypeId, ModelBox>>);

// impl Deref for Models {
//     type Target = Arc<FxHashMap<TypeId, ModelBox>>;
//     fn deref(&self) -> &Self::Target {
//         &self.0
//     }
// }

// impl Models {
//     #[must_use]
//     pub fn builder() -> ModelsBuilder {
//         ModelsBuilder::default()
//     }

//     #[must_use]
//     pub fn get<T>(&self) -> Option<MappedRwLockReadGuard<'_, T>>
//     where
//         T: 'static,
//     {
//         let ty = TypeId::of::<T>();
//         self.0.get(&ty).map(|model| model.downcast_ref::<T>())
//     }

//     #[must_use]
//     pub fn get_mut<T>(&self) -> Option<MappedRwLockWriteGuard<'_, T>>
//     where
//         T: 'static,
//     {
//         let ty = TypeId::of::<T>();
//         self.0.get(&ty).map(|model| model.downcast_mut::<T>())
//     }
// }

// pub trait ModelAccess: Context {
//     fn models(&self) -> &Models;
//     fn model<M>(&self) -> MappedRwLockReadGuard<'_, M>
//     where
//         M: 'static,
//     {
//         self.models().get::<M>().unwrap()
//     }
//     fn try_model<M>(&self) -> Option<MappedRwLockReadGuard<'_, M>>
//     where
//         M: 'static,
//     {
//         self.models().get::<M>()
//     }
//     fn query<M, F, R>(&self, f: F) -> R
//     where
//         M: 'static,
//         F: FnOnce(&M) -> R,
//         R: 'static,
//     {
//         f(&self.model())
//     }
// }

// pub trait ModelModify: ModelAccess {
//     fn model_mut<M>(&self) -> MappedRwLockWriteGuard<'_, M>
//     where
//         M: 'static,
//     {
//         self.models().get_mut::<M>().unwrap()
//     }
//     fn try_model_mut<M>(&self) -> Option<MappedRwLockWriteGuard<'_, M>>
//     where
//         M: 'static,
//     {
//         self.models().get_mut::<M>()
//     }
//     fn update<M, F>(&self, mut f: F)
//     where
//         M: 'static,
//         F: FnMut(&mut M),
//     {
//         f(&mut self.model_mut());
//     }
// }

// pub struct Model<T: 'static>(pub MappedRwLockReadGuard<'static, T>);

// impl<T> Deref for Model<T> {
//     type Target = T;
//     fn deref(&self) -> &Self::Target {
//         &self.0
//     }
// }

// impl<C, T> FromContext<C> for Model<T>
// where
//     C: Context + ModelAccess,
//     T: 'static,
// {
//     fn from_context(context: &C) -> Self {
//         Self(context.model::<T>())
//     }
// }

// pub struct ModelMut<'a, T>(pub RefMut<'a, T>);

// impl<'a, T> Deref for ModelMut<'a, T> {
//     type Target = T;
//     fn deref(&self) -> &Self::Target {
//         &self.0
//     }
// }

// impl<'a, C, T> BorrowFromContext<'a, C> for ModelMut<'a, T>
// where
//     C: Context + ModelModify,
//     T: 'static,
// {
//     fn from_context(context: &'a C) -> Self {
//         Self(context.model_mut::<T>())
//     }
// }
