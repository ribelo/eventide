use std::{
    any::{Any, TypeId},
    future::Future,
    pin::Pin,
    sync::OnceLock,
};

use generational_box::{GenerationalBox, Owner, SyncStorage};
use rustc_hash::FxHashMap;

#[cfg(not(feature = "async"))]
pub type Effect<M> = Box<dyn FnOnce(AppContext<M>) + Send + Sync>;

#[cfg(feature = "async")]
pub type Effect<M> = Box<
    dyn FnOnce(AppContext<M>) -> Pin<Box<dyn Future<Output = ()> + Send + 'static>>
        + Send
        + 'static,
>;

pub trait Model: Send + Sync + 'static {}
impl<T> Model for T where T: Send + Sync + 'static {}

static STORAGE: OnceLock<Owner<SyncStorage>> = OnceLock::new();

pub struct AppContextBuilder<M: Model> {
    pub(crate) storage: Owner<SyncStorage>,
    pub(crate) model: M,
    pub(crate) capabilities: FxHashMap<TypeId, Box<dyn Any + Send + Sync>>,
    pub(crate) resources: FxHashMap<TypeId, Box<dyn Any + Send + Sync>>,
    #[cfg(feature = "rayon")]
    pub(crate) pool: Option<rayon::ThreadPool>,
    #[cfg(feature = "tokio")]
    pub(crate) rt: Option<tokio::runtime::Runtime>,
}

#[derive(Debug, thiserror::Error)]
#[error("Capability of type '{0}' already exists")]
pub struct CapabilityExistsError(&'static str);

#[derive(Debug, thiserror::Error)]
#[error("Resource of type '{0}' already exists")]
pub struct ResourceExistsError(&'static str);

impl<M: Model> AppContextBuilder<M> {
    #[must_use]
    fn new(model: M) -> Self {
        AppContextBuilder {
            storage: Owner::default(),
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
        T: Send + Sync + 'static,
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
        #[cfg(not(feature = "async"))]
        let (effect_tx, effect_rx) = crossbeam_channel::unbounded();
        #[cfg(feature = "async")]
        let (effect_tx, effect_rx) = tokio::sync::mpsc::unbounded_channel();

        #[cfg(feature = "rayon")]
        let pool = self
            .pool
            .unwrap_or_else(|| rayon::ThreadPoolBuilder::new().build().unwrap());

        #[cfg(feature = "tokio")]
        let rt = self.rt.unwrap_or_else(|| {
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .unwrap()
        });

        let storage = self.storage;

        let model = storage.insert(self.model);
        let capabilities = storage.insert(self.capabilities);
        let resources = storage.insert(self.resources);
        let effect_tx = storage.insert(effect_tx);
        let effect_rx = storage.insert(effect_rx);
        let is_running = storage.insert(false);
        #[cfg(feature = "rayon")]
        let pool = storage.insert(pool);
        #[cfg(feature = "tokio")]
        let rt = storage.insert(rt);

        assert!(STORAGE.set(storage).is_ok(), "Cannot set storage");

        AppContext {
            model,
            capabilities,
            resources,
            effect_tx,
            effect_rx,
            is_running,
            #[cfg(feature = "rayon")]
            pool,
            #[cfg(feature = "tokio")]
            rt,
        }
    }
}

#[cfg(not(feature = "async"))]
#[derive(Debug)]
pub struct AppContext<M: Model> {
    pub model: GenerationalBox<M, SyncStorage>,
    pub(crate) capabilities:
        GenerationalBox<FxHashMap<TypeId, Box<dyn Any + Send + Sync>>, SyncStorage>,
    pub(crate) resources:
        GenerationalBox<FxHashMap<TypeId, Box<dyn Any + Send + Sync>>, SyncStorage>,
    effect_tx: GenerationalBox<crossbeam_channel::Sender<Effect<M>>, SyncStorage>,
    effect_rx: GenerationalBox<crossbeam_channel::Receiver<Effect<M>>, SyncStorage>,
    is_running: GenerationalBox<bool, SyncStorage>,
    #[cfg(feature = "rayon")]
    pool: GenerationalBox<rayon::ThreadPool, SyncStorage>,
    #[cfg(feature = "tokio")]
    rt: GenerationalBox<tokio::runtime::Runtime, SyncStorage>,
}

#[cfg(feature = "async")]
#[derive(Debug)]
pub struct AppContext<M: Model> {
    pub model: GenerationalBox<M, SyncStorage>,
    pub(crate) capabilities:
        GenerationalBox<FxHashMap<TypeId, Box<dyn Any + Send + Sync>>, SyncStorage>,
    pub(crate) resources:
        GenerationalBox<FxHashMap<TypeId, Box<dyn Any + Send + Sync>>, SyncStorage>,
    effect_tx: GenerationalBox<tokio::sync::mpsc::UnboundedSender<Effect<M>>, SyncStorage>,
    effect_rx: GenerationalBox<tokio::sync::mpsc::UnboundedReceiver<Effect<M>>, SyncStorage>,
    is_running: GenerationalBox<bool, SyncStorage>,
    #[cfg(feature = "rayon")]
    pool: GenerationalBox<rayon::ThreadPool, SyncStorage>,
    #[cfg(feature = "tokio")]
    rt: GenerationalBox<tokio::runtime::Runtime, SyncStorage>,
}

impl<M: Model> Clone for AppContext<M> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<M: Model> Copy for AppContext<M> {}

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

    #[cfg(not(feature = "async"))]
    pub fn next_effect(&self) -> Result<Effect<M>, crossbeam_channel::TryRecvError> {
        self.effect_rx.read().try_recv()
    }

    #[cfg(feature = "async")]
    pub fn next_effect(&self) -> Result<Effect<M>, tokio::sync::mpsc::error::TryRecvError> {
        self.effect_rx.write().try_recv()
    }

    #[cfg(not(feature = "async"))]
    pub fn handle_effects(&self) {
        while let Ok(effect) = self.next_effect() {
            (effect)(*self);
        }
    }

    #[cfg(feature = "async")]
    pub async fn handle_effects(&self) {
        while let Ok(effect) = self.next_effect() {
            (effect)(*self).await;
        }
    }

    #[cfg(not(feature = "async"))]
    pub fn run(&self) {
        let cx = *self;
        let effect_rx = self.effect_rx.manually_drop().unwrap();
        *self.is_running.write() = true;
        std::thread::spawn(move || {
            while cx.is_running() {
                crossbeam_channel::select! {
                    recv(effect_rx) -> maybe_effect => {
                        #[cfg(not(feature = "async"))]
                        if let Ok(effect) = maybe_effect {
                            (effect)(cx);
                        }
                    }
                    default(std::time::Duration::from_millis(100)) => continue,
                }
            }
        });
    }

    #[cfg(feature = "async")]
    pub fn run(&self) {
        let cx = *self;
        let mut effect_rx = self.effect_rx.manually_drop().unwrap();
        *self.is_running.write() = true;
        tokio::spawn(async move {
            while cx.is_running() {
                tokio::select! {
                    Some(effect) = effect_rx.recv() => {
                        #[cfg(feature = "async")]
                        (effect)(cx).await;
                    }
                    _ = tokio::time::sleep(std::time::Duration::from_millis(100)) => continue,
                }
            }
        });
    }

    #[inline]
    pub fn stop(&self) -> Result<(), EventideError> {
        if *self.is_running.read() {
            *self.is_running.write() = false;
            Ok(())
        } else {
            Err(EventideError::AlreadyStopped)
        }
    }

    #[inline]
    #[must_use]
    pub fn is_running(&self) -> bool {
        *self.is_running.read()
    }

    #[inline]
    #[cfg(not(feature = "async"))]
    pub fn dispatch<E>(&self, effect: E)
    where
        E: FnOnce(AppContext<M>) + Send + Sync + 'static,
    {
        self.effect_tx.read().send(Box::new(effect)).unwrap();
    }

    #[inline]
    #[cfg(feature = "async")]
    pub fn dispatch<E>(&self, effect: E)
    where
        E: FnOnce(AppContext<M>) -> Pin<Box<dyn Future<Output = ()> + Send + 'static>>
            + Send
            + 'static,
    {
        self.effect_tx.read().send(Box::new(effect)).unwrap()
    }

    #[must_use]
    #[inline]
    pub fn with_model<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&M, ModelContext<M>) -> R,
    {
        f(&self.model.read(), (*self).into())
    }

    #[inline]
    pub fn with_model_mut<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut M, ModelContext<M>) -> R,
    {
        f(&mut self.model.write(), (*self).into())
    }

    #[inline]
    pub fn provide_capability<T>(&self, capability: T) -> Result<(), CapabilityExistsError>
    where
        T: Clone + Send + Sync + 'static,
    {
        let mut capabilities = self.capabilities.write();
        let ty = TypeId::of::<T>();
        if capabilities.contains_key(&ty) {
            return Err(CapabilityExistsError(std::any::type_name::<T>()));
        }
        capabilities.insert(ty, Box::new(capability));
        Ok(())
    }

    #[inline]
    #[must_use]
    pub fn expect_capability<T>(&self) -> T
    where
        T: Clone + Send + Sync + 'static,
    {
        // SAFETY: This function is unsafe because it performs unchecked downcasting.
        // We assume that the type T matches the actual type stored in the capabilities map.
        // This assumption relies on the correct usage of `provide_capability` to insert
        // capabilities of the expected types. Violating this assumption may lead to
        // undefined behavior.
        unsafe {
            let ty = TypeId::of::<T>();
            self.capabilities
                .read()
                .get(&ty)
                .map(|boxed_value| boxed_value.downcast_ref_unchecked::<T>())
                .cloned()
                .unwrap()
        }
    }

    #[inline]
    pub fn provide_resource<T>(&self, capability: T) -> Result<(), ResourceExistsError>
    where
        T: Send + Sync + 'static,
    {
        let mut resources = self.resources.write();
        let ty = TypeId::of::<T>();
        if resources.contains_key(&ty) {
            return Err(ResourceExistsError(std::any::type_name::<T>()));
        }
        let gbox = STORAGE.get().unwrap().insert(capability);
        resources.insert(ty, Box::new(gbox));
        Ok(())
    }

    #[inline]
    #[must_use]
    pub fn expect_resource<T>(&self) -> GenerationalBox<T, SyncStorage>
    where
        T: Send + Sync + 'static,
    {
        // SAFETY: This function is unsafe because it performs unchecked downcasting.
        // We assume that the type T matches the actual type stored in the resources map.
        // This assumption relies on the correct usage of `provide_resource` to insert
        // capabilities of the expected types. Violating this assumption may lead to
        // undefined behavior.
        unsafe {
            let ty = TypeId::of::<T>();
            self.resources
                .read()
                .get(&ty)
                .map(|boxed_value| {
                    boxed_value.downcast_ref_unchecked::<GenerationalBox<T, SyncStorage>>()
                })
                .copied()
                .unwrap()
        }
    }

    #[inline]
    #[cfg(feature = "tokio")]
    pub fn spawn_async<F, Fut, R>(&self, f: F) -> tokio::task::JoinHandle<R>
    where
        F: FnOnce(AsyncTaskContext<M>) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = R> + Send,
        R: Send + 'static,
    {
        let cx = AsyncTaskContext::from(*self);
        self.rt.read().spawn(async move { f(cx).await })
    }

    #[inline]
    #[cfg(feature = "tokio")]
    pub fn spawn_blocking<F, R>(&self, f: F) -> tokio::task::JoinHandle<R>
    where
        F: FnOnce(AsyncTaskContext<M>) -> R + Send + Sync + 'static,
        R: Send + 'static,
    {
        let cx = AsyncTaskContext::from(*self);
        self.rt.read().spawn_blocking(move || f(cx))
    }

    #[inline]
    pub fn spawn<F, R>(&self, f: F) -> std::thread::JoinHandle<R>
    where
        F: FnOnce(TaskContext<M>) -> R + Send + 'static,
        R: Send + 'static,
    {
        let cx = TaskContext::from(*self);
        std::thread::spawn(move || f(cx))
    }

    #[inline]
    pub fn scope<F, T, R>(&self, f: F) -> R
    where
        F: FnOnce(&std::thread::Scope<'_, '_>, TaskContext<M>) -> R + Send,
        T: Send,
        R: Send + 'static,
    {
        let cx = TaskContext::from(*self);
        std::thread::scope(|s| f(s, cx))
    }

    #[inline]
    #[cfg(feature = "rayon")]
    pub fn rayon_spawn<F>(&self, f: F)
    where
        F: FnOnce(TaskContext<M>) + Send + 'static,
    {
        let cx = TaskContext::from(*self);
        self.pool.read().spawn(move || f(cx));
    }

    #[inline]
    #[cfg(feature = "rayon")]
    pub fn rayon_scope<F, T>(&self, f: F)
    where
        F: FnOnce(&rayon::Scope<'_>, TaskContext<M>) + Send,
        T: Send,
    {
        let cx = TaskContext::from(*self);
        self.pool.read().scope(|s| f(s, cx));
    }
}

pub struct ModelContext<M: Model>(AppContext<M>);

impl<M: Model> Clone for ModelContext<M> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<M: Model> Copy for ModelContext<M> {}

impl<M: Model> ModelContext<M> {
    #[inline]
    #[cfg(not(feature = "async"))]
    pub fn dispatch<E>(&self, effect: E)
    where
        E: FnOnce(AppContext<M>) + Send + Sync + 'static,
    {
        self.0.dispatch(effect);
    }

    #[inline]
    #[cfg(feature = "async")]
    pub fn dispatch<E>(&self, effect: E)
    where
        E: FnOnce(AppContext<M>) -> Pin<Box<dyn Future<Output = ()> + Send + 'static>>
            + Send
            + 'static,
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
    pub fn expect_resource<T>(&self) -> GenerationalBox<T, SyncStorage>
    where
        T: Send + Sync + 'static,
    {
        self.0.expect_resource::<T>()
    }
}

pub struct TaskContext<M: Model>(AppContext<M>);

impl<M: Model> Clone for TaskContext<M> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<M: Model> Copy for TaskContext<M> {}

impl<M: Model> TaskContext<M> {
    #[inline]
    #[cfg(not(feature = "async"))]
    pub fn dispatch<E>(&self, effect: E)
    where
        E: FnOnce(AppContext<M>) + Send + Sync + 'static,
    {
        self.0.dispatch(effect);
    }

    #[inline]
    #[cfg(feature = "async")]
    pub fn dispatch<E>(&self, effect: E)
    where
        E: FnOnce(AppContext<M>) -> Pin<Box<dyn Future<Output = ()> + Send + 'static>>
            + Send
            + 'static,
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
    pub fn expect_resource<T>(&self) -> GenerationalBox<T, SyncStorage>
    where
        T: Send + Sync + 'static,
    {
        self.0.expect_resource::<T>()
    }

    #[inline]
    pub fn spawn<F, R>(&self, f: F) -> std::thread::JoinHandle<R>
    where
        F: FnOnce(TaskContext<M>) -> R + Send + 'static,
        R: Send + 'static,
    {
        self.0.spawn(f)
    }

    #[inline]
    pub fn scope<F, T, R>(&self, f: F) -> R
    where
        F: FnOnce(&std::thread::Scope<'_, '_>, TaskContext<M>) -> R + Send,
        T: Send,
        R: Send + 'static,
    {
        self.0.scope::<F, T, R>(f)
    }

    #[inline]
    #[cfg(feature = "rayon")]
    pub fn rayon_spawn<F>(&self, f: F)
    where
        F: FnOnce(TaskContext<M>) + Send + 'static,
    {
        self.0.rayon_spawn(f);
    }

    #[inline]
    #[cfg(feature = "rayon")]
    pub fn rayon_scope<F, T>(&self, f: F)
    where
        F: FnOnce(&rayon::Scope<'_>, TaskContext<M>) + Send,
        T: Send,
    {
        self.0.rayon_scope::<F, T>(f);
    }
}

#[cfg(feature = "tokio")]
pub struct AsyncTaskContext<M: Model>(AppContext<M>);

#[cfg(feature = "tokio")]
impl<M: Model> Clone for AsyncTaskContext<M> {
    fn clone(&self) -> Self {
        *self
    }
}

#[cfg(feature = "tokio")]
impl<M: Model> Copy for AsyncTaskContext<M> {}

#[cfg(feature = "tokio")]
impl<M: Model> AsyncTaskContext<M> {
    #[inline]
    #[cfg(not(feature = "async"))]
    pub fn dispatch<E>(&self, effect: E)
    where
        E: FnOnce(AppContext<M>) + Send + Sync + 'static,
    {
        self.0.dispatch(effect);
    }

    #[inline]
    #[cfg(feature = "async")]
    pub fn dispatch<E>(&self, effect: E)
    where
        E: FnOnce(AppContext<M>) -> Pin<Box<dyn Future<Output = ()> + Send + 'static>>
            + Send
            + 'static,
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
    pub fn expect_resource<T>(&self) -> GenerationalBox<T, SyncStorage>
    where
        T: Send + Sync + 'static,
    {
        self.0.expect_resource::<T>()
    }

    #[inline]
    #[cfg(feature = "tokio")]
    pub fn spawn_async<F, Fut, R>(&self, f: F) -> tokio::task::JoinHandle<R>
    where
        F: FnOnce(AsyncTaskContext<M>) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = R> + Send,
        R: Send + 'static,
    {
        self.0.spawn_async(f)
    }

    #[inline]
    #[cfg(feature = "tokio")]
    pub fn spawn_blocking<F, R>(&self, f: F) -> tokio::task::JoinHandle<R>
    where
        F: FnOnce(AsyncTaskContext<M>) -> R + Send + Sync + 'static,
        R: Send + 'static,
    {
        self.0.spawn_blocking(f)
    }

    #[inline]
    pub fn spawn<F, R>(&self, f: F) -> std::thread::JoinHandle<R>
    where
        F: FnOnce(TaskContext<M>) -> R + Send + 'static,
        R: Send + 'static,
    {
        self.0.spawn(f)
    }

    #[inline]
    pub fn scope<F, T, R>(&self, f: F) -> R
    where
        F: FnOnce(&std::thread::Scope<'_, '_>, TaskContext<M>) -> R + Send,
        T: Send,
        R: Send + 'static,
    {
        self.0.scope::<F, T, R>(f)
    }

    #[inline]
    #[cfg(feature = "rayon")]
    pub fn rayon_spawn<F>(&self, f: F)
    where
        F: FnOnce(TaskContext<M>) + Send + 'static,
    {
        self.0.rayon_spawn(f);
    }
    #[inline]
    #[cfg(feature = "rayon")]
    pub fn rayon_scope<F, T>(&self, f: F)
    where
        F: FnOnce(&rayon::Scope<'_>, TaskContext<M>) + Send,
        T: Send,
    {
        self.0.rayon_scope::<F, T>(f);
    }
}

impl<M: Model> From<AppContext<M>> for ModelContext<M> {
    fn from(value: AppContext<M>) -> Self {
        Self(value)
    }
}

impl<M: Model> From<AppContext<M>> for TaskContext<M> {
    fn from(value: AppContext<M>) -> Self {
        Self(value)
    }
}

#[cfg(feature = "tokio")]
impl<M: Model> From<AppContext<M>> for AsyncTaskContext<M> {
    fn from(value: AppContext<M>) -> Self {
        Self(value)
    }
}

#[cfg(feature = "tokio")]
impl<M: Model> From<AsyncTaskContext<M>> for TaskContext<M> {
    fn from(value: AsyncTaskContext<M>) -> Self {
        Self(value.0)
    }
}
