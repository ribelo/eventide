use std::{
    any::{Any, TypeId},
    ops::Deref,
    sync::Arc,
};

use parking_lot::RwLock;
use rustc_hash::FxHashMap;

use crate::{context::Context, role::{ImpliedBy, RoleGuarded, RoleHolder}};

#[derive(Debug)]
pub struct Resource(RwLock<Box<dyn Any + Send + Sync + 'static>>);

impl Deref for Resource {
    type Target = RwLock<Box<dyn Any + Send + Sync>>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug, Default)]
pub struct ResourcesBuilder(FxHashMap<TypeId, Box<dyn Any + Send + Sync>>);

impl ResourcesBuilder {
    #[must_use]
    pub fn insert<T>(mut self, resource: T) -> Self
    where
        T: RoleGuarded + Clone + Send + Sync + 'static,
    {
        self.0.insert(TypeId::of::<T>(), Box::new(resource));
        self
    }
    #[must_use]
    pub fn build(self) -> Resources {
        let resources = self
            .0
            .into_iter()
            .map(|(id, resource)| (id, Resource(RwLock::new(resource))))
            .collect();

        Resources(Arc::new(resources))
    }
}

#[derive(Debug, Clone)]
pub struct Resources(Arc<FxHashMap<TypeId, Resource>>);

impl Deref for Resources {
    type Target = Arc<FxHashMap<TypeId, Resource>>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Resources {
    #[must_use]
    pub fn get<T>(&self) -> Option<T>
    where
        T: Clone + Send + Sync + 'static,
    {
        let ty = TypeId::of::<T>();
        self.0.get(&ty).map(|resource| {
            let boxed_value = resource.read();
            // SAFETY: We verify the type matches via TypeId before calling downcast_ref_unchecked
            unsafe { boxed_value.downcast_ref_unchecked::<T>().clone() }
        })
    }
}

pub trait ResourceAccess: Context {
    fn resources(&self) -> &Resources;
    fn resource<T>(&self) -> T
    where
        T: RoleGuarded + Clone + Send + Sync + 'static,
        T::Role: ImpliedBy<<Self as RoleHolder>::Role>,
    {
        self.resources().get::<T>().unwrap()
    }
    fn try_resource<T>(&self) -> Option<T>
    where
        T: RoleGuarded + Clone + Send + Sync + 'static,
        T::Role: ImpliedBy<<Self as RoleHolder>::Role>,
    {
        self.resources().get::<T>()
    }
}
