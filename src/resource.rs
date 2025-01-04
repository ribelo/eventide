use std::{
    any::{Any, TypeId},
    ops::Deref,
    sync::{Arc, RwLock},
};

use rustc_hash::FxHashMap;

use crate::context::Context;

#[derive(Default, Debug, Clone)]
pub struct Resources(Arc<RwLock<FxHashMap<TypeId, Box<dyn Any + Send + Sync>>>>);

impl Deref for Resources {
    type Target = RwLock<FxHashMap<TypeId, Box<dyn Any + Send + Sync>>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Resources {
    pub fn insert<T>(&mut self, value: T)
    where
        T: Send + Sync + Clone + 'static,
    {
        let ty = TypeId::of::<T>();
        let boxed_value = Box::new(value);
        let mut lock = self.write().expect("Failed to acquire write lock");
        lock.insert(ty, boxed_value);
    }

    #[must_use]
    pub fn get<T>(&self) -> Option<T>
    where
        T: Clone + Send + Sync + 'static,
    {
        let ty = TypeId::of::<T>();
        let lock = self.read().expect("Failed to acquire read lock");
        lock.get(&ty).map(|boxed_value|
            // SAFETY: We verify the type matches via TypeId before insertion,
            // so this downcast is guaranteed to succeed
            unsafe { boxed_value.downcast_ref_unchecked::<T>().clone() })
    }
}

pub trait ResourceAccess: Context {
    fn resources(&self) -> &Resources;
    fn resource<T>(&self) -> T
    where
        T: Clone + Send + Sync + 'static,
    {
        self.resources().get::<T>().unwrap()
    }
    fn try_resource<T>(&self) -> Option<T>
    where
        T: Clone + Send + Sync + 'static,
    {
        self.resources().get::<T>()
    }
    fn with_resource<T, F, R>(&self, f: F) -> R
    where
        T: Clone + Send + Sync + 'static,
        F: FnOnce(&T) -> R,
    {
        f(&self.resource::<T>())
    }
}

pub trait ResourceModify: ResourceAccess {
    fn add_resource<T>(&self, value: T)
    where
        T: Clone + Send + Sync + 'static,
    {
        self.resources().write().expect("Failed to acquire write lock")
            .insert(TypeId::of::<T>(), Box::new(value));
    }

    fn remove_resource<T>(&self) -> Option<Box<dyn Any + Send + Sync>>
    where
        T: Clone + Send + Sync + 'static,
    {
        self.resources().write().expect("Failed to acquire write lock")
            .remove(&TypeId::of::<T>())
    }
}
