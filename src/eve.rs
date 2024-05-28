use std::{
    any::{Any, TypeId},
    sync::Arc,
};

use atomic_take::AtomicTake;
use parking_lot::RwLock;
use rustc_hash::FxHashMap;

pub type Effect<M> = Box<dyn FnOnce(&AppContext<M>) + Send>;

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
        let (effect_tx, effect_rx) = crossbeam_channel::unbounded();
        let (cancelation_tx, cancelation_rx) = crossbeam_channel::bounded(0);

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

        run_effect_loop(cx.clone(), effect_rx, cancelation_rx);

        cx
    }
}

#[derive(Debug)]
pub struct AppContext<M> {
    pub(crate) model: Arc<RwLock<M>>,
    pub(crate) capabilities: Arc<RwLock<FxHashMap<TypeId, Box<dyn Any + Send + Sync>>>>,
    pub(crate) resources: Arc<RwLock<FxHashMap<TypeId, Box<dyn Any + Send + Sync>>>>,
    pub(crate) effect_tx: crossbeam_channel::Sender<Effect<M>>,
    pub(crate) cancelation_tx: Arc<AtomicTake<crossbeam_channel::Sender<()>>>,
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

#[derive(Debug, thiserror::Error)]
pub enum EventideError {
    #[error("Event loop already stopped")]
    AlreadyStopped,
    #[error("Context dropped")]
    ContextDropped,
}

impl<M: Model> AppContext<M> {
    #[inline]
    pub fn builder(model: M) -> AppContextBuilder<M> {
        AppContextBuilder::new(model)
    }

    #[inline]
    pub fn stop(&mut self) -> Result<(), EventideError> {
        if let Some(cancelation_token) = self.cancelation_tx.take() {
            cancelation_token.send(()).unwrap();
            Ok(())
        } else {
            Err(EventideError::AlreadyStopped)
        }
    }

    #[inline]
    #[must_use]
    pub fn is_running(&self) -> bool {
        !self.cancelation_tx.is_taken()
    }

    #[inline]
    pub fn invoke<E, T, R>(&self, effect: E) -> R
    where
        E: FnOnce(T) -> R + Send + 'static,
        T: From<AppContext<M>>,
        R: Send + 'static,
    {
        let cx = T::from(self.clone());
        (effect)(cx)
    }

    #[inline]
    pub fn dispatch<E>(&self, effect: E)
    where
        E: FnOnce(&AppContext<M>) + Send + 'static,
    {
        self.effect_tx.send(Box::new(effect)).unwrap();
    }

    #[inline]
    pub fn provide_capability<T>(&self, capability: T)
    where
        T: Clone + Send + Sync + 'static,
    {
        let ty = TypeId::of::<T>();
        self.capabilities.write().insert(ty, Box::new(capability));
    }

    #[inline]
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

    #[inline]
    pub fn provide_resource<T>(&self, resource: T)
    where
        T: Clone + Send + Sync + 'static,
    {
        let ty = TypeId::of::<T>();
        self.resources.write().insert(ty, Box::new(resource));
    }

    #[inline]
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

    #[inline]
    pub fn model(&self) -> parking_lot::RwLockReadGuard<M> {
        self.model.read()
    }

    #[inline]
    pub fn model_mut(&self) -> parking_lot::RwLockWriteGuard<M> {
        self.model.write()
    }

    #[inline]
    pub fn with_model<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&M) -> R,
    {
        f(&*self.model.read())
    }

    #[inline]
    pub fn with_model_mut<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut M) -> R,
    {
        f(&mut *self.model.write())
    }

    #[inline]
    #[cfg(feature = "tokio")]
    pub fn task<F, Fut, R>(&self, f: F) -> tokio::task::JoinHandle<R>
    where
        F: FnOnce(&AsyncTaskContext<M>) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = R> + Send,
        R: Send + 'static,
    {
        let cx = AsyncTaskContext::from(self.clone());
        self.rt.spawn(async move { f(&cx).await })
    }

    #[inline]
    #[cfg(feature = "tokio")]
    pub fn task_blocking<F, R>(&self, f: F) -> tokio::task::JoinHandle<R>
    where
        F: FnOnce(&TaskContext<M>) -> R + Send + Sync + 'static,
        R: Send + 'static,
    {
        let cx = TaskContext::from(self.clone());
        self.rt.spawn_blocking(move || f(&cx))
    }

    #[inline]
    pub fn spawn<F, R>(&self, f: F) -> std::thread::JoinHandle<R>
    where
        F: FnOnce(&TaskContext<M>) -> R + Send + 'static,
        R: Send + 'static,
    {
        let cx = TaskContext::from(self.clone());
        std::thread::spawn(move || f(&cx))
    }

    #[inline]
    pub fn scope<F, T, R>(&self, f: F) -> R
    where
        F: FnOnce(&std::thread::Scope<'_, '_>, &TaskContext<M>) -> R + Send,
        T: Send,
        R: Send + 'static,
    {
        let cx = TaskContext::from(self.clone());
        std::thread::scope(|s| f(s, &cx))
    }

    #[inline]
    #[cfg(feature = "rayon")]
    pub fn rayon_spawn<F>(&self, f: F)
    where
        F: FnOnce(&TaskContext<M>) + Send + 'static,
    {
        let cx = TaskContext::from(self.clone());
        self.pool.spawn(move || f(&cx));
    }

    #[inline]
    #[cfg(feature = "rayon")]
    pub fn rayon_scope<F, T>(&self, f: F)
    where
        F: FnOnce(&rayon::Scope<'_>, &TaskContext<M>) + Send,
        T: Send,
    {
        let cx = TaskContext::from(self.clone());
        self.pool.scope(|s| f(s, &cx));
    }
}

pub struct TaskContext<M>(AppContext<M>);

impl<M> Clone for TaskContext<M> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<M: Model> TaskContext<M> {
    #[inline]
    pub fn invoke<E, T, R>(&self, effect: E) -> R
    where
        E: FnOnce(T) -> R + Send + 'static,
        T: From<TaskContext<M>>,
        R: Send + 'static,
    {
        let cx = T::from(self.clone());
        (effect)(cx)
    }

    #[inline]
    pub fn dispatch<F>(&self, effect: F)
    where
        F: FnOnce(&AppContext<M>) + Send + 'static,
    {
        self.0.dispatch(effect);
    }

    #[inline]
    #[must_use]
    pub fn expect_capability<T>(&self) -> T
    where
        T: Clone + Send + Sync + 'static,
    {
        self.0.expect_capability::<T>()
    }

    #[inline]
    #[must_use]
    pub fn expect_resource<T>(&self) -> T
    where
        T: Clone + Send + Sync + 'static,
    {
        self.0.expect_resource::<T>()
    }

    #[inline]
    pub fn spawn<F, R>(&self, f: F) -> std::thread::JoinHandle<R>
    where
        F: FnOnce(&TaskContext<M>) -> R + Send + 'static,
        R: Send + 'static,
    {
        self.0.spawn(f)
    }

    #[inline]
    pub fn scope<F, T, R>(&self, f: F) -> R
    where
        F: FnOnce(&std::thread::Scope<'_, '_>, &TaskContext<M>) -> R + Send,
        T: Send,
        R: Send + 'static,
    {
        self.0.scope::<F, T, R>(f)
    }

    #[inline]
    #[cfg(feature = "rayon")]
    pub fn rayon_spawn<F>(&self, f: F)
    where
        F: FnOnce(&TaskContext<M>) + Send + 'static,
    {
        self.0.rayon_spawn(f);
    }

    #[inline]
    #[cfg(feature = "rayon")]
    pub fn rayon_scope<F, T>(&self, f: F)
    where
        F: FnOnce(&rayon::Scope<'_>, &TaskContext<M>) + Send,
        T: Send,
    {
        self.0.rayon_scope::<F, T>(f);
    }
}

#[cfg(feature = "tokio")]
pub struct AsyncTaskContext<M>(AppContext<M>);

#[cfg(feature = "tokio")]
impl<M> Clone for AsyncTaskContext<M> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

#[cfg(feature = "tokio")]
impl<M: Model> AsyncTaskContext<M> {
    #[inline]
    pub fn invoke<E, T, R>(&self, effect: E) -> R
    where
        E: FnOnce(T) -> R + Send + 'static,
        T: From<AsyncTaskContext<M>>,
        R: Send + 'static,
    {
        let cx = T::from(self.clone());
        (effect)(cx)
    }

    #[inline]
    pub fn dispatch<F>(&self, effect: F)
    where
        F: FnOnce(&AppContext<M>) + Send + 'static,
    {
        self.0.dispatch(effect);
    }

    #[inline]
    #[must_use]
    pub fn expect_capability<T>(&self) -> T
    where
        T: Clone + Send + Sync + 'static,
    {
        self.0.expect_capability::<T>()
    }

    #[inline]
    #[must_use]
    pub fn expect_resource<T>(&self) -> T
    where
        T: Clone + Send + Sync + 'static,
    {
        self.0.expect_resource::<T>()
    }

    #[inline]
    #[cfg(feature = "tokio")]
    pub fn task<F, Fut, R>(&self, f: F) -> tokio::task::JoinHandle<R>
    where
        F: FnOnce(&AsyncTaskContext<M>) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = R> + Send,
        R: Send + 'static,
    {
        self.0.task(f)
    }

    #[inline]
    pub fn spawn<F, R>(&self, f: F) -> std::thread::JoinHandle<R>
    where
        F: FnOnce(&TaskContext<M>) -> R + Send + 'static,
        R: Send + 'static,
    {
        self.0.spawn(f)
    }

    #[inline]
    pub fn scope<F, T, R>(&self, f: F) -> R
    where
        F: FnOnce(&std::thread::Scope<'_, '_>, &TaskContext<M>) -> R + Send,
        T: Send,
        R: Send + 'static,
    {
        self.0.scope::<F, T, R>(f)
    }

    #[inline]
    #[cfg(feature = "rayon")]
    pub fn rayon_spawn<F>(&self, f: F)
    where
        F: FnOnce(&TaskContext<M>) + Send + 'static,
    {
        self.0.rayon_spawn(f);
    }
    #[inline]
    #[cfg(feature = "rayon")]
    pub fn rayon_scope<F, T>(&self, f: F)
    where
        F: FnOnce(&rayon::Scope<'_>, &TaskContext<M>) + Send,
        T: Send,
    {
        self.0.rayon_scope::<F, T>(f);
    }
}

impl<M: Model> From<AppContext<M>> for TaskContext<M> {
    fn from(value: AppContext<M>) -> Self {
        Self(value)
    }
}

impl<M: Model> From<AppContext<M>> for AsyncTaskContext<M> {
    fn from(value: AppContext<M>) -> Self {
        Self(value)
    }
}

impl<M: Model> From<AsyncTaskContext<M>> for TaskContext<M> {
    fn from(value: AsyncTaskContext<M>) -> Self {
        Self(value.0)
    }
}

fn run_effect_loop<M: Model>(
    cx: AppContext<M>,
    effect_rx: crossbeam_channel::Receiver<Effect<M>>,
    cancelation_rx: crossbeam_channel::Receiver<()>,
) {
    std::thread::spawn(move || {
        let mut is_canceled = false;
        while !is_canceled {
            crossbeam_channel::select! {
                recv(cancelation_rx) -> _ => is_canceled = true,
                recv(effect_rx) -> maybe_effect => {
                    if let Ok(effect) = maybe_effect {
                        (effect)(&cx);
                    }
                }
            }
        }
    });
}

#[cfg(test)]
mod test {
    use crate::prelude::*;

    #[test]
    fn effect_test() {
        struct MyModel;
        fn hello(_cx: &AppContext<MyModel>) {
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
