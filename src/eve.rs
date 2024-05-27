use std::{
    any::{Any, TypeId},
    sync::{Arc, Weak},
};

use atomic_take::AtomicTake;
use core::panic;
use parking_lot::RwLock;
use rustc_hash::FxHashMap;

pub type Effect<M> = Box<dyn FnOnce(&EffectContext<M>) + Send>;

pub trait Model: Send + Sync + 'static {}
impl<T> Model for T where T: Send + Sync + 'static {}

#[derive(Debug)]
pub struct AppContextBuilder<M: Model> {
    pub(crate) model: M,
    pub(crate) capabilities: FxHashMap<TypeId, Box<dyn Any + Send + Sync>>,
    pub(crate) resources: FxHashMap<TypeId, Box<dyn Any + Send + Sync>>,
    #[cfg(feature = "rayon")]
    pub(crate) pool: Option<rayon::ThreadPool>,
    #[cfg(feature = "tokio")]
    pub(crate) rt: Option<tokio::runtime::Runtime>,
}

impl<M: Model> AppContextBuilder<M> {
    #[must_use]
    fn new(model: M) -> Self {
        AppContextBuilder {
            model,
            capabilities: FxHashMap::default(),
            resources: FxHashMap::default(),
            #[cfg(feature = "rayon")]
            pool: None,
            #[cfg(feature = "tokio")]
            rt: None,
        }
    }

    #[must_use]
    pub fn capability<T>(mut self, capability: T) -> Self
    where
        T: Clone + Send + Sync + 'static,
    {
        let ty = TypeId::of::<T>();
        self.capabilities.insert(ty, Box::new(capability));

        self
    }

    #[must_use]
    pub fn resource<T>(mut self, resource: T) -> Self
    where
        T: Clone + Send + Sync + 'static,
    {
        let ty = TypeId::of::<T>();
        self.resources.insert(ty, Box::new(resource));

        self
    }

    #[must_use]
    #[cfg(feature = "tokio")]
    pub fn runtime(mut self, rt: tokio::runtime::Runtime) -> Self {
        self.rt = Some(rt);
        self
    }

    #[must_use]
    #[cfg(feature = "rayon")]
    pub fn pool(mut self, pool: rayon::ThreadPool) -> Self {
        self.pool = Some(pool);
        self
    }

    pub fn build(self) -> AppContext<M> {
        let (effect_tx, effect_rx) = flume::unbounded();
        let (cancelation_tx, cancelation_rx) = flume::bounded(0);

        #[cfg(feature = "rayon")]
        let pool = self
            .pool
            .unwrap_or_else(|| rayon::ThreadPoolBuilder::new().build().unwrap());

        #[cfg(feature = "tokio")]
        let runtime = self.rt.unwrap_or_else(|| {
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .unwrap()
        });

        let cx = AppContext {
            model: Arc::new(RwLock::new(self.model)),
            capabilities: Arc::new(RwLock::new(self.capabilities)),
            resources: Arc::new(RwLock::new(self.resources)),
            effect_tx,
            cancelation_tx: Arc::new(AtomicTake::new(cancelation_tx)),

            #[cfg(feature = "rayon")]
            pool: Arc::new(pool),

            #[cfg(feature = "tokio")]
            rt: Arc::new(runtime),
        };

        let effect_cx = EffectContext::from(cx.clone());
        run_effect_loop(effect_cx, effect_rx, cancelation_rx);

        cx
    }
}

#[derive(Debug)]
pub struct AppContext<M> {
    pub(crate) model: Arc<RwLock<M>>,
    pub(crate) capabilities: Arc<RwLock<FxHashMap<TypeId, Box<dyn Any + Send + Sync>>>>,
    pub(crate) resources: Arc<RwLock<FxHashMap<TypeId, Box<dyn Any + Send + Sync>>>>,
    pub(crate) effect_tx: flume::Sender<Effect<M>>,
    pub(crate) cancelation_tx: Arc<AtomicTake<flume::Sender<()>>>,
    #[cfg(feature = "rayon")]
    pub(crate) pool: Arc<rayon::ThreadPool>,
    #[cfg(feature = "tokio")]
    pub(crate) rt: Arc<tokio::runtime::Runtime>,
}

impl<M> Clone for AppContext<M> {
    fn clone(&self) -> Self {
        Self {
            model: Arc::clone(&self.model),
            capabilities: Arc::clone(&self.capabilities),
            resources: Arc::clone(&self.resources),
            effect_tx: self.effect_tx.clone(),
            cancelation_tx: Arc::clone(&self.cancelation_tx),
            #[cfg(feature = "rayon")]
            pool: Arc::clone(&self.pool),
            #[cfg(feature = "tokio")]
            rt: Arc::clone(&self.rt),
        }
    }
}

impl<A> TryFrom<EffectContext<A>> for AppContext<A> {
    type Error = EventideError;
    fn try_from(value: EffectContext<A>) -> Result<Self, Self::Error> {
        let model = value.model.upgrade().ok_or(EventideError::ContextDropped)?;
        let capabilities = value
            .capabilities
            .upgrade()
            .ok_or(EventideError::ContextDropped)?;
        let resources = value
            .resources
            .upgrade()
            .ok_or(EventideError::ContextDropped)?;
        let cancelation_tx = value
            .cancelation_tx
            .upgrade()
            .ok_or(EventideError::ContextDropped)?;

        #[cfg(feature = "rayon")]
        let pool = value.pool.upgrade().ok_or(EventideError::ContextDropped)?;

        #[cfg(feature = "tokio")]
        let rt = value.rt.upgrade().ok_or(EventideError::ContextDropped)?;

        Ok(Self {
            model,
            capabilities,
            resources,
            effect_tx: value.effect_tx,
            cancelation_tx,
            #[cfg(feature = "rayon")]
            pool,
            #[cfg(feature = "tokio")]
            rt,
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum EventideError {
    #[error("Event loop already stopped")]
    AlreadyStopped,
    #[error("Context dropped")]
    ContextDropped,
}

impl<M: Model> AppContext<M> {
    pub fn builder(model: M) -> AppContextBuilder<M> {
        AppContextBuilder::new(model)
    }

    pub fn stop(&mut self) -> Result<(), EventideError> {
        if let Some(cancelation_token) = self.cancelation_tx.take() {
            cancelation_token.send(()).unwrap();
            Ok(())
        } else {
            Err(EventideError::AlreadyStopped)
        }
    }

    #[must_use]
    pub fn is_running(&self) -> bool {
        !self.cancelation_tx.is_taken()
    }

    pub fn dispatch<F>(&self, effect: F)
    where
        F: FnOnce(&EffectContext<M>) + Send + 'static,
    {
        self.effect_tx.send(Box::new(effect)).unwrap();
    }

    pub fn provide_capability<T>(&self, capability: T)
    where
        T: Clone + Send + Sync + 'static,
    {
        let ty = TypeId::of::<T>();
        self.capabilities.write().insert(ty, Box::new(capability));
    }

    #[must_use]
    pub fn expect_capability<T>(&self) -> T
    where
        T: Clone + Send + Sync + 'static,
    {
        let ty = TypeId::of::<T>();
        self.capabilities
            .read()
            .get(&ty)
            .map(|boxed_value| boxed_value.downcast_ref::<T>().unwrap())
            .cloned()
            .unwrap()
    }

    pub fn provide_resource<T>(&self, resource: T)
    where
        T: Clone + Send + Sync + 'static,
    {
        let ty = TypeId::of::<T>();
        self.resources.write().insert(ty, Box::new(resource));
    }

    #[must_use]
    pub fn expect_resource<T>(&self) -> T
    where
        T: Clone + Send + Sync + 'static,
    {
        let ty = TypeId::of::<T>();
        self.resources
            .read()
            .get(&ty)
            .map(|boxed_value| boxed_value.downcast_ref::<T>().unwrap())
            .cloned()
            .unwrap()
    }

    pub fn model(&self) -> parking_lot::RwLockReadGuard<M> {
        self.model.read()
    }

    pub fn model_mut(&self) -> parking_lot::RwLockWriteGuard<M> {
        self.model.write()
    }

    pub fn with_model<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&M) -> R,
    {
        f(&*self.model.read())
    }

    pub fn with_model_mut<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut M) -> R,
    {
        f(&mut *self.model.write())
    }

    #[cfg(feature = "tokio")]
    pub fn spawn<F, Fut, R>(&self, f: F) -> tokio::task::JoinHandle<R>
    where
        F: FnOnce(TaskContext<M>) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = R> + Send,
        R: Send + 'static,
    {
        let cx = TaskContext::from(self.clone());
        self.rt.spawn(async move { f(cx).await })
    }

    #[cfg(feature = "tokio")]
    pub fn spawn_blocking<F, R>(&self, f: F) -> tokio::task::JoinHandle<R>
    where
        F: FnOnce(AppContext<M>) -> R + Send + Sync + 'static,
        R: Send + 'static,
    {
        let cx = self.clone();
        self.rt.spawn_blocking(move || f(cx))
    }

    #[cfg(feature = "rayon")]
    pub fn spawn_rayon<F>(&self, f: F)
    where
        F: FnOnce(AppContext<M>) + Send + 'static,
    {
        let cx = self.clone();
        self.pool.spawn(move || f(cx));
    }

    #[cfg(feature = "rayon")]
    pub fn scope<F, T>(&self, f: F)
    where
        F: FnOnce(&rayon::Scope<'_>, AppContext<M>) + Send,
        T: Send,
    {
        let cx = self.clone();
        self.pool.scope(|s| f(s, cx));
    }
}

pub struct EffectContext<M> {
    pub(crate) model: Weak<RwLock<M>>,
    pub(crate) capabilities: Weak<RwLock<FxHashMap<TypeId, Box<dyn Any + Send + Sync>>>>,
    pub(crate) resources: Weak<RwLock<FxHashMap<TypeId, Box<dyn Any + Send + Sync>>>>,
    pub(crate) effect_tx: flume::Sender<Effect<M>>,
    pub(crate) cancelation_tx: Weak<AtomicTake<flume::Sender<()>>>,
    #[cfg(feature = "rayon")]
    pub(crate) pool: Weak<rayon::ThreadPool>,
    #[cfg(feature = "tokio")]
    pub(crate) rt: Weak<tokio::runtime::Runtime>,
}

impl<A> Clone for EffectContext<A> {
    fn clone(&self) -> Self {
        Self {
            model: Weak::clone(&self.model),
            capabilities: Weak::clone(&self.capabilities),
            resources: Weak::clone(&self.resources),
            effect_tx: self.effect_tx.clone(),
            cancelation_tx: Weak::clone(&self.cancelation_tx),
            #[cfg(feature = "rayon")]
            pool: Weak::clone(&self.pool),
            #[cfg(feature = "tokio")]
            rt: Weak::clone(&self.rt),
        }
    }
}

impl<A> From<AppContext<A>> for EffectContext<A> {
    fn from(app: AppContext<A>) -> Self {
        Self {
            model: Arc::downgrade(&app.model),
            capabilities: Arc::downgrade(&app.capabilities),
            resources: Arc::downgrade(&app.resources),
            effect_tx: app.effect_tx,
            cancelation_tx: Arc::downgrade(&app.cancelation_tx),
            #[cfg(feature = "rayon")]
            pool: Arc::downgrade(&app.pool),
            #[cfg(feature = "tokio")]
            rt: Arc::downgrade(&app.rt),
        }
    }
}

impl<M: Model> EffectContext<M> {
    pub fn stop(&mut self) -> Result<(), EventideError> {
        if let Some(cancelation_tx) = self.cancelation_tx.upgrade() {
            if let Some(cancelation_token) = cancelation_tx.take() {
                // Ignore the result as the receiver might be already dropped
                let _ = cancelation_token.send(());
            }
            Ok(())
        } else {
            Err(EventideError::AlreadyStopped)
        }
    }

    #[inline]
    pub fn dispatch<F>(&self, effect: F)
    where
        F: FnOnce(&EffectContext<M>) + Send + 'static,
    {
        self.effect_tx.send(Box::new(effect)).unwrap();
    }

    pub fn provide_capability<T>(&self, capability: T)
    where
        T: Clone + Send + Sync + 'static,
    {
        print!("capability provided {}", std::any::type_name::<T>());
        let ty = TypeId::of::<T>();
        self.capabilities
            .upgrade()
            .expect("Capabilities should not be dropped")
            .write()
            .insert(ty, Box::new(capability));
    }

    #[must_use]
    pub fn expect_capability<T>(&self) -> T
    where
        T: Clone + Send + Sync + 'static,
    {
        let ty = TypeId::of::<T>();
        println!("Look for Capability: {}", std::any::type_name::<T>());
        dbg!(&self.capabilities.upgrade().unwrap().read().get(&ty));
        self.capabilities
            .upgrade()
            .expect("Capabilities should not be dropped")
            .read()
            .get(&ty)
            .map(|boxed_value| boxed_value.downcast_ref::<T>().unwrap())
            .cloned()
            .unwrap_or_else(|| panic!("Capability not found: {}", std::any::type_name::<T>()))
    }

    pub fn provide_resource<T>(&self, resource: T)
    where
        T: Clone + Send + Sync + 'static,
    {
        let ty = TypeId::of::<T>();
        self.resources
            .upgrade()
            .expect("Resources should not be dropped")
            .write()
            .insert(ty, Box::new(resource));
    }

    #[must_use]
    pub fn expect_resource<T>(&self) -> T
    where
        T: Clone + Send + Sync + 'static,
    {
        let ty = TypeId::of::<T>();
        self.resources
            .upgrade()
            .expect("Resources should not be dropped")
            .read()
            .get(&ty)
            .map(|boxed_value| boxed_value.downcast_ref::<T>().unwrap())
            .cloned()
            .unwrap_or_else(|| panic!("Resource not found: {}", std::any::type_name::<T>()))
    }

    pub fn with_model<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&M) -> R,
    {
        f(&*self
            .model
            .upgrade()
            .expect("Model shoudl not be dropped")
            .read())
    }

    pub fn with_model_mut<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut M) -> R,
    {
        f(&mut *self
            .model
            .upgrade()
            .expect("Model shoudl not be dropped")
            .write())
    }
    #[inline]
    #[cfg(feature = "tokio")]
    pub fn spawn<F, Fut, R>(&self, f: F) -> tokio::task::JoinHandle<R>
    where
        F: FnOnce(TaskContext<M>) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = R> + Send,
        R: Send + 'static,
    {
        let Some(rt) = self.rt.upgrade() else {
            panic!("Tokio runtime is not available.");
        };
        let cx = TaskContext::from(self.clone());
        rt.spawn(async move { f(cx).await })
    }

    #[cfg(feature = "tokio")]
    #[inline]
    pub fn spawn_blocking<F, R>(&self, f: F) -> tokio::task::JoinHandle<R>
    where
        F: FnOnce(TaskContext<M>) -> R + Send + Sync + 'static,
        R: Send + 'static,
    {
        let Some(rt) = self.rt.upgrade() else {
            panic!("Tokio runtime is not available.");
        };
        let cx = TaskContext::from(self.clone());
        rt.spawn_blocking(move || f(cx))
    }
    #[inline]
    #[cfg(feature = "rayon")]
    pub fn spawn_rayon<F>(&self, f: F)
    where
        F: FnOnce(TaskContext<M>) + Send + 'static,
    {
        let Some(pool) = self.pool.upgrade() else {
            panic!("Rayon pool is not available.");
        };
        let cx = TaskContext::from(self.clone());
        pool.spawn(move || f(cx));
    }
    #[inline]
    #[cfg(feature = "rayon")]
    pub fn scope<F, T>(&self, f: F)
    where
        F: FnOnce(&rayon::Scope<'_>, AppContext<M>) + Send,
        T: Send,
    {
        let Some(pool) = self.pool.upgrade() else {
            panic!("Rayon pool is not available.");
        };
        let cx = AppContext::try_from(self.clone()).unwrap();
        pool.scope(move |s| f(s, cx));
    }
}

pub struct TaskContext<M> {
    pub(crate) capabilities: Weak<RwLock<FxHashMap<TypeId, Box<dyn Any + Send + Sync>>>>,
    pub(crate) resources: Weak<RwLock<FxHashMap<TypeId, Box<dyn Any + Send + Sync>>>>,
    pub(crate) effect_tx: flume::Sender<Effect<M>>,
    #[cfg(feature = "rayon")]
    pub(crate) pool: Weak<rayon::ThreadPool>,
    #[cfg(feature = "tokio")]
    pub(crate) rt: Weak<tokio::runtime::Runtime>,
}

impl<M> Clone for TaskContext<M> {
    fn clone(&self) -> Self {
        Self {
            capabilities: Weak::clone(&self.capabilities),
            resources: Weak::clone(&self.resources),
            effect_tx: self.effect_tx.clone(),
            #[cfg(feature = "rayon")]
            pool: Weak::clone(&self.pool),
            #[cfg(feature = "tokio")]
            rt: Weak::clone(&self.rt),
        }
    }
}

impl<A> From<AppContext<A>> for TaskContext<A> {
    fn from(app: AppContext<A>) -> Self {
        Self {
            capabilities: Arc::downgrade(&app.capabilities),
            resources: Arc::downgrade(&app.resources),
            effect_tx: app.effect_tx,
            #[cfg(feature = "rayon")]
            pool: Arc::downgrade(&app.pool),
            #[cfg(feature = "tokio")]
            rt: Arc::downgrade(&app.rt),
        }
    }
}

impl<A> From<EffectContext<A>> for TaskContext<A> {
    fn from(app: EffectContext<A>) -> Self {
        Self {
            capabilities: app.capabilities,
            resources: app.resources,
            effect_tx: app.effect_tx,
            #[cfg(feature = "rayon")]
            pool: app.pool,
            #[cfg(feature = "tokio")]
            rt: app.rt,
        }
    }
}

impl<A> From<&EffectContext<A>> for TaskContext<A> {
    fn from(app: &EffectContext<A>) -> Self {
        Self {
            capabilities: Weak::clone(&app.capabilities),
            resources: Weak::clone(&app.resources),
            effect_tx: app.effect_tx.clone(),
            #[cfg(feature = "rayon")]
            pool: Weak::clone(&app.pool),
            #[cfg(feature = "tokio")]
            rt: Weak::clone(&app.rt),
        }
    }
}

impl<M: Model> TaskContext<M> {
    #[inline]
    pub fn dispatch<F>(&self, effect: F)
    where
        F: FnOnce(&EffectContext<M>) + Send + 'static,
    {
        self.effect_tx.send(Box::new(effect)).unwrap();
    }

    pub fn provide_capability<T>(&self, capability: T)
    where
        T: Clone + Send + Sync + 'static,
    {
        let ty = TypeId::of::<T>();
        self.capabilities
            .upgrade()
            .expect("Capabilities should not be dropped")
            .write()
            .insert(ty, Box::new(capability));
    }

    #[must_use]
    pub fn expect_capability<T>(&self) -> T
    where
        T: Clone + Send + Sync + 'static,
    {
        let ty = TypeId::of::<T>();
        self.capabilities
            .upgrade()
            .expect("Capabilities should not be dropped")
            .read()
            .get(&ty)
            .map(|boxed_value| boxed_value.downcast_ref::<T>().unwrap())
            .cloned()
            .expect("Capability not found")
    }

    pub fn provide_resource<T>(&self, resource: T)
    where
        T: Clone + Send + Sync + 'static,
    {
        let ty = TypeId::of::<T>();
        self.resources
            .upgrade()
            .expect("Resources should not be dropped")
            .write()
            .insert(ty, Box::new(resource));
    }

    #[must_use]
    pub fn expect_resource<T>(&self) -> T
    where
        T: Clone + Send + Sync + 'static,
    {
        let ty = TypeId::of::<T>();
        self.resources
            .upgrade()
            .expect("Resources should not be dropped")
            .read()
            .get(&ty)
            .map(|boxed_value| boxed_value.downcast_ref::<T>().unwrap())
            .cloned()
            .expect("Resource not found")
    }

    #[inline]
    #[cfg(feature = "tokio")]
    pub fn spawn<F, Fut, R>(&self, f: F) -> tokio::task::JoinHandle<R>
    where
        F: FnOnce(TaskContext<M>) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = R> + Send,
        R: Send + 'static,
    {
        let Some(rt) = self.rt.upgrade() else {
            panic!("Tokio runtime is not available.");
        };
        let cx = self.clone();
        rt.spawn(async move { f(cx).await })
    }

    #[cfg(feature = "tokio")]
    #[inline]
    pub fn spawn_blocking<F, R>(&self, f: F) -> tokio::task::JoinHandle<R>
    where
        F: FnOnce(TaskContext<M>) -> R + Send + Sync + 'static,
        R: Send + 'static,
    {
        let Some(rt) = self.rt.upgrade() else {
            panic!("Tokio runtime is not available.");
        };
        let cx = self.clone();
        rt.spawn_blocking(move || f(cx))
    }
    #[inline]
    #[cfg(feature = "rayon")]
    pub fn spawn_rayon<F>(&self, f: F)
    where
        F: FnOnce(TaskContext<M>) + Send + 'static,
    {
        let Some(pool) = self.pool.upgrade() else {
            panic!("Rayon pool is not available.");
        };
        let cx = self.clone();
        pool.spawn(move || f(cx));
    }
    #[inline]
    #[cfg(feature = "rayon")]
    pub fn scope<F, T>(&self, f: F)
    where
        F: FnOnce(&rayon::Scope<'_>, TaskContext<M>) + Send,
        T: Send,
    {
        let Some(pool) = self.pool.upgrade() else {
            panic!("Rayon pool is not available.");
        };
        let cx = self.clone();
        pool.scope(move |s| f(s, cx));
    }
}

fn run_effect_loop<M: Model>(
    cx: EffectContext<M>,
    effect_rx: flume::Receiver<Effect<M>>,
    cancelation_rx: flume::Receiver<()>,
) {
    std::thread::spawn(move || {
        let mut is_canceled = false;
        while !is_canceled {
            flume::Selector::new()
                .recv(&cancelation_rx, |_| is_canceled = true)
                .recv(&effect_rx, |effect| {
                    if let Ok(effect) = effect {
                        (effect)(&cx);
                    }
                })
                .wait();
        }
    });
}

#[cfg(test)]
mod test {
    use crate::prelude::*;

    #[test]
    fn effect_test() {
        struct MyModel;
        fn hello(_cx: &EffectContext<MyModel>) {
            println!("hello from function")
        }

        let cx = AppContext::builder(MyModel).build();
        cx.dispatch(|_cx| println!("hello from closure"));
        let x = 1;
        cx.dispatch(move |_cx| println!("hello from closure with x={x}"));
        cx.dispatch(hello);
        cx.dispatch(|cx| hello(cx));
    }
}
