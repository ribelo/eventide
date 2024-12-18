use std::{
    any::{Any, TypeId},
    fmt,
    ops::Deref,
};

use generational_box::{GenerationalBox, Owner, SyncStorage};
use rustc_hash::FxHashMap;

use crate::context::{Context, FromContext};

#[derive(Clone, Default)]
pub struct Resources {
    owner: Owner<SyncStorage>,
    models: FxHashMap<TypeId, GenerationalBox<Box<dyn Any + Send + Sync>, SyncStorage>>,
}

impl fmt::Debug for Resources {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Resources")
            .field("models", &self.models)
            .finish_non_exhaustive()
    }
}

impl Resources {
    pub fn insert<T>(&mut self, value: T)
    where
        T: Send + Sync + Clone + 'static,
    {
        let ty = TypeId::of::<T>();
        let boxed_value = Box::new(value);
        self.models
            .insert(ty, self.owner.insert(boxed_value));
    }

    #[must_use]
    pub fn get<T>(&self) -> Option<T>
    where
        T: Clone + Send + Sync + 'static,
    {
        let ty = TypeId::of::<T>();
        self.models.get(&ty).map(|resource| {
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
}

pub struct Resource<T>(pub T)
where
    T: Clone + Send + Sync + 'static;

impl<C, T> FromContext<C> for Resource<T>
where
    C: Context + ResourceAccess,
    T: Clone + Send + Sync + 'static,
{
    fn from_context(context: &C) -> Self {
        context.resource::<T>().into()
    }
}

impl<T> From<T> for Resource<T>
where
    T: Clone + Send + Sync + 'static,
{
    fn from(value: T) -> Self {
        Self(value)
    }
}

impl<T> Deref for Resource<T>
where
    T: Clone + Send + Sync + 'static,
{
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
