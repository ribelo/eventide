use std::{
    any::{Any, TypeId},
    fmt,
    sync::{atomic::AtomicBool, Arc, OnceLock},
};
#[cfg(feature = "tokio")]
use std::{future::Future, pin::Pin};

use parking_lot::RwLock;
use rustc_hash::FxHashMap;

pub type Effect<M> = Box<dyn FnOnce(AppContextRef<App, M>) + Send + Sync>;

pub trait Model: fmt::Debug + Send + Sync + 'static {}
impl<T> Model for T where T: fmt::Debug + Send + Sync + 'static {}

static APP_CONTEXT: OnceLock<Box<dyn Any + Send + Sync + 'static>> = OnceLock::new();

mod private {
    use super::ctx;

    pub trait Sealed {}
    impl Sealed for ctx::App {}
    impl Sealed for ctx::Model {}
    impl Sealed for ctx::Task {}
    #[cfg(feature = "tokio")]
    impl Sealed for ctx::AsyncTask {}
}

pub mod ctx {
    use super::private;

    #[derive(Debug)]
    pub struct App;
    #[derive(Debug)]
    pub struct Model;

    #[derive(Debug)]
    pub struct Task;

    #[cfg(feature = "tokio")]
    #[derive(Debug)]
    pub struct AsyncTask;

    pub trait Context: private::Sealed + 'static {}

    impl Context for App {}
    impl Context for Model {}
    impl Context for Task {}
    #[cfg(feature = "tokio")]
    impl Context for AsyncTask {}
}
#[allow(clippy::wildcard_imports)]
use ctx::*;

pub mod capabilities {
    #[allow(clippy::wildcard_imports)]
    use super::ctx::*;

    pub trait CanReadModel: Context + 'static {}
    impl CanReadModel for App {}
    impl CanReadModel for Task {}
    #[cfg(feature = "tokio")]
    impl CanReadModel for AsyncTask {}

    pub trait CanModifyModel: CanReadModel {}
    impl CanModifyModel for App {}

    pub trait CanDispatch: Context {}
    impl CanDispatch for App {}
    impl CanDispatch for Task {}
    #[cfg(feature = "tokio")]
    impl CanDispatch for AsyncTask {}

    pub trait CanSpawnTasks: Context {}
    impl CanSpawnTasks for App {}
    impl CanSpawnTasks for Task {}

    pub trait CanReadResources: Context {}
    impl CanReadResources for App {}
    impl CanReadResources for Task {}
    #[cfg(feature = "tokio")]
    impl CanReadResources for AsyncTask {}

    pub trait CanModifyResources: CanReadResources {}
    impl CanModifyResources for App {}
    impl CanModifyResources for Task {}
    #[cfg(feature = "tokio")]
    impl CanModifyResources for AsyncTask {}

    #[cfg(feature = "tokio")]
    pub trait CanSpawnAsyncTask: Context {}
    #[cfg(feature = "tokio")]
    impl CanSpawnAsyncTask for App {}
    #[cfg(feature = "tokio")]
    impl CanSpawnAsyncTask for AsyncTask {}

    #[cfg(feature = "rayon")]
    pub trait CanSpawnRayonTask: Context {}
    #[cfg(feature = "rayon")]
    impl CanSpawnRayonTask for App {}
    #[cfg(feature = "rayon")]
    impl CanSpawnRayonTask for Task {}
}
#[allow(clippy::wildcard_imports)]
use capabilities::*;

pub struct AppContextBuilder<M: Model> {
    pub(crate) model: M,
    pub(crate) resources: FxHashMap<TypeId, Box<dyn Any + Send + Sync>>,
    #[cfg(feature = "rayon")]
    pub(crate) pool: Option<rayon::ThreadPool>,
    #[cfg(feature = "tokio")]
    pub(crate) handle: Option<tokio::runtime::Handle>,
}

#[derive(Debug, thiserror::Error)]
#[error("Resource of type '{0}' already exists")]
pub struct ResourceExistsError(&'static str);

impl<M: Model> AppContextBuilder<M> {
    #[must_use]
    fn new(model: M) -> Self {
        AppContextBuilder {
            model,
            resources: FxHashMap::default(),
            #[cfg(feature = "rayon")]
            pool: None,
            #[cfg(feature = "tokio")]
            handle: None,
        }
    }

    #[must_use]
    pub fn resource<T>(mut self, resource: T) -> Self
    where
        T: Send + Sync + 'static,
    {
        self.resources
            .insert(std::any::TypeId::of::<T>(), Box::new(resource));
        self
    }

    #[must_use]
    #[cfg(feature = "tokio")]
    pub fn handle(mut self, handle: tokio::runtime::Handle) -> Self {
        self.handle = Some(handle);

        self
    }

    #[must_use]
    #[cfg(feature = "rayon")]
    pub fn pool(mut self, pool: rayon::ThreadPool) -> Self {
        self.pool = Some(pool);

        self
    }

    pub fn build(self) -> AppContextRef<App, M> {
        let (effect_tx, effect_rx) = crossbeam_channel::unbounded();
        let (stop_tx, stop_rx) = crossbeam_channel::unbounded();

        #[cfg(feature = "rayon")]
        let pool = self
            .pool
            .unwrap_or_else(|| rayon::ThreadPoolBuilder::new().build().unwrap());

        #[cfg(feature = "tokio")]
        let handle = self
            .handle
            .unwrap_or_else(|| tokio::runtime::Handle::current());

        let cx = AppContext {
            model: RwLock::new(self.model),
            resources: RwLock::new(self.resources),
            effect_tx,
            effect_rx,
            stop_tx,
            stop_rx,
            is_running: Arc::new(AtomicBool::new(false)),
            #[cfg(feature = "rayon")]
            pool,
            #[cfg(feature = "tokio")]
            handle,
            _phantom: std::marker::PhantomData::<App>,
        };

        APP_CONTEXT.get_or_init(|| Box::new(cx));

        AppContextRef {
            _phantom: std::marker::PhantomData,
        }
    }
}

#[derive(Debug)]
pub struct AppContext<S: Context, M: Model> {
    model: RwLock<M>,
    resources: RwLock<FxHashMap<TypeId, Box<dyn Any + Send + Sync>>>,
    effect_tx: crossbeam_channel::Sender<Effect<M>>,
    effect_rx: crossbeam_channel::Receiver<Effect<M>>,
    stop_tx: crossbeam_channel::Sender<()>,
    stop_rx: crossbeam_channel::Receiver<()>,
    is_running: Arc<AtomicBool>,

    #[cfg(feature = "tokio")]
    handle: tokio::runtime::Handle,
    #[cfg(feature = "rayon")]
    pool: rayon::ThreadPool,

    _phantom: std::marker::PhantomData<S>,
}

impl<M: Model> AppContext<App, M> {
    pub fn builder(model: M) -> AppContextBuilder<M> {
        AppContextBuilder::new(model)
    }
}

#[derive(Debug)]
pub struct AppContextRef<S: Context, M: Model> {
    _phantom: std::marker::PhantomData<(S, M)>,
}

impl<S: Context, M: Model> Clone for AppContextRef<S, M> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<S: Context, M: Model> Copy for AppContextRef<S, M> {}

#[derive(Debug, thiserror::Error)]
pub enum BindewerkError {
    #[error("Event loop already stopped")]
    AlreadyStopped,
    #[error("Context dropped")]
    ContextDropped,
}

#[derive(Debug, thiserror::Error)]
pub enum DispatchError {
    #[error("Channel closed")]
    ChannelClosed,
    #[error("Runtime stopped")]
    RuntimeStopped,
}

#[allow(clippy::type_complexity)]
pub struct EffectBuilder<M: Model> {
    cx: AppContextRef<App, M>,
    updates: Vec<Box<dyn FnOnce(&mut M) + Send + Sync>>,
    tasks: Vec<Box<dyn FnOnce(AppContextRef<Task, M>) + Send + Sync>>,
    blocking_tasks: Vec<Box<dyn FnOnce(AppContextRef<Task, M>) + Send + Sync>>,
    scoped_tasks: Vec<
        Box<
            dyn for<'scope> FnOnce(&'scope std::thread::Scope<'scope, '_>, AppContextRef<Task, M>)
                + Send
                + Sync,
        >,
    >,
    #[cfg(feature = "tokio")]
    async_tasks: Vec<
        Box<
            dyn FnOnce(AppContextRef<AsyncTask, M>) -> Pin<Box<dyn Future<Output = ()> + Send>>
                + Send
                + Sync,
        >,
    >,
    #[cfg(feature = "rayon")]
    rayon_tasks: Vec<Box<dyn FnOnce(AppContextRef<Task, M>) + Send + Sync>>,
    #[cfg(feature = "rayon")]
    rayon_scoped_tasks:
        Vec<Box<dyn FnOnce(&rayon::Scope<'_>, AppContextRef<Task, M>) + Send + Sync>>,
}

impl<M: Model> EffectBuilder<M> {
    #[must_use]
    pub fn update<F>(mut self, f: F) -> Self
    where
        F: FnOnce(&mut M) + Send + Sync + 'static,
    {
        self.updates.push(Box::new(f));
        self
    }

    #[must_use]
    pub fn spawn<F>(mut self, f: F) -> Self
    where
        F: FnOnce(AppContextRef<Task, M>) + Send + Sync + 'static,
    {
        self.tasks.push(Box::new(f));
        self
    }

    #[must_use]
    #[inline]
    pub fn scope<F>(mut self, f: F) -> Self
    where
        F: for<'scope> FnOnce(&'scope std::thread::Scope<'scope, '_>, AppContextRef<Task, M>)
            + Send
            + Sync
            + 'static,
    {
        self.scoped_tasks.push(Box::new(f));
        self
    }

    #[must_use]
    pub fn spawn_blocking<F>(mut self, f: F) -> Self
    where
        F: FnOnce(AppContextRef<Task, M>) + Send + Sync + 'static,
    {
        self.blocking_tasks.push(Box::new(f));
        self
    }

    #[cfg(feature = "tokio")]
    #[must_use]
    pub fn spawn_async<F, Fut>(mut self, f: F) -> Self
    where
        F: FnOnce(AppContextRef<AsyncTask, M>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        self.async_tasks.push(Box::new(move |cx| {
            Box::pin(f(cx)) as Pin<Box<dyn Future<Output = ()> + Send>>
        }));
        self
    }

    #[cfg(feature = "rayon")]
    #[must_use]
    pub fn spawn_rayon<F>(mut self, f: F) -> Self
    where
        F: FnOnce(AppContextRef<Task, M>) + Send + Sync + 'static,
    {
        self.rayon_tasks.push(Box::new(f));
        self
    }

    #[cfg(feature = "rayon")]
    #[must_use]
    pub fn rayon_scope<F>(mut self, f: F) -> Self
    where
        F: FnOnce(&rayon::Scope<'_>, AppContextRef<Task, M>) + Send + Sync + 'static,
    {
        self.rayon_scoped_tasks.push(Box::new(f));
        self
    }

    pub fn dispatch(self) {
        let updates = self.updates;
        let tasks = self.tasks;

        #[cfg(feature = "tokio")]
        let blocking_tasks = self.blocking_tasks;
        let scoped_tasks = self.scoped_tasks;
        #[cfg(feature = "tokio")]
        let async_tasks = self.async_tasks;
        #[cfg(feature = "rayon")]
        let parallel_tasks = self.rayon_tasks;
        #[cfg(feature = "rayon")]
        let parallel_scoped_tasks = self.rayon_scoped_tasks;

        self.cx.dispatch(move |cx| {
            // Apply model updates first
            cx.update(move |m| {
                for update in updates {
                    update(m);
                }
            });

            // Spawn regular tasks
            for task in tasks {
                cx.spawn(task);
            }

            // Execute scoped tasks
            for task in scoped_tasks {
                cx.scope(task);
            }

            // Spawn blocking tasks using tokio
            #[cfg(feature = "tokio")]
            for task in blocking_tasks {
                cx.spawn_blocking(task);
            }

            // Spawn async tasks if tokio enabled
            #[cfg(feature = "tokio")]
            for task in async_tasks {
                let task = Box::new(task)
                    as Box<
                        dyn FnOnce(
                                AppContextRef<AsyncTask, M>,
                            )
                                -> Pin<Box<dyn Future<Output = ()> + Send>>
                            + Send
                            + Sync,
                    >;
                cx.spawn_async(task);
            }

            // Spawn parallel tasks if rayon enabled
            #[cfg(feature = "rayon")]
            for task in parallel_tasks {
                cx.spawn_rayon(task);
            }

            // Execute parallel scoped tasks if rayon enabled
            #[cfg(feature = "rayon")]
            for task in parallel_scoped_tasks {
                cx.rayon_scope(|s, task_cx| task(s, task_cx));
            }
        });
    }
}

impl<S: Context, M: Model> AppContext<S, M> {
    #[must_use]
    pub fn current() -> AppContextRef<S, M> {
        AppContextRef {
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<M: Model> AppContextRef<App, M> {
    #[inline]
    pub fn builder(model: M) -> AppContextBuilder<M> {
        AppContextBuilder::new(model)
    }

    pub fn next_effect(&self) -> Result<Effect<M>, crossbeam_channel::TryRecvError> {
        let app_cx = APP_CONTEXT
            .get()
            .map(|any| unsafe { any.downcast_ref_unchecked::<AppContext<App, M>>() })
            .unwrap();

        app_cx.effect_rx.try_recv()
    }

    pub fn handle_effects(&self) {
        while let Ok(effect) = self.next_effect() {
            (effect)(*self);
        }
    }

    pub fn run(&self) {
        let app_cx = APP_CONTEXT
            .get()
            .map(|any| unsafe { any.downcast_ref_unchecked::<AppContext<App, M>>() })
            .unwrap();

        let effect_rx = app_cx.effect_rx.clone();
        app_cx
            .is_running
            .store(true, std::sync::atomic::Ordering::SeqCst);

        let self_ref = *self;
        let is_running = Arc::clone(&app_cx.is_running);
        std::thread::spawn(move || {
            while is_running.load(std::sync::atomic::Ordering::SeqCst) {
                crossbeam_channel::select! {
                    recv(&effect_rx) -> maybe_effect => {
                        if let Ok(effect) = maybe_effect {
                            (effect)(self_ref);
                        }
                    }
                }
            }
        });
    }

    #[inline]
    pub fn stop(&self) -> Result<(), BindewerkError> {
        let app_cx = APP_CONTEXT
            .get()
            .map(|any| unsafe { any.downcast_ref_unchecked::<AppContext<App, M>>() })
            .unwrap();

        if app_cx.is_running.load(std::sync::atomic::Ordering::SeqCst) {
            app_cx
                .is_running
                .store(false, std::sync::atomic::Ordering::SeqCst);
            app_cx
                .stop_tx
                .send(())
                .map_err(|_| BindewerkError::ContextDropped)?;
            Ok(())
        } else {
            Err(BindewerkError::AlreadyStopped)
        }
    }

    #[inline]
    pub fn wait_for_stop(&self) -> Result<(), BindewerkError> {
        let app_cx = APP_CONTEXT
            .get()
            .map(|any| unsafe { any.downcast_ref_unchecked::<AppContext<App, M>>() })
            .unwrap();
        app_cx
            .stop_rx
            .recv()
            .map_err(|_| BindewerkError::ContextDropped)
    }

    #[inline]
    pub fn dispatch_sync<E>(&self, effect: E)
    where
        E: FnOnce(AppContextRef<App, M>) + Send + Sync + 'static,
    {
        effect(*self);
    }
}

impl<S: Context, M: Model> AppContextRef<S, M> {
    #[inline]
    #[must_use]
    pub fn is_running(&self) -> bool {
        let app_cx = APP_CONTEXT
            .get()
            .map(|any| unsafe { any.downcast_ref_unchecked::<AppContext<App, M>>() })
            .unwrap();
        app_cx.is_running.load(std::sync::atomic::Ordering::SeqCst)
    }

    fn convert<T: Context>(self) -> AppContextRef<T, M> {
        AppContextRef {
            _phantom: std::marker::PhantomData,
        }
    }

    #[must_use]
    pub fn effect(&self) -> EffectBuilder<M> {
        EffectBuilder {
            cx: self.convert(),
            updates: Vec::new(),
            tasks: Vec::new(),
            blocking_tasks: Vec::new(),
            scoped_tasks: Vec::new(),
            #[cfg(feature = "tokio")]
            async_tasks: Vec::new(),
            #[cfg(feature = "rayon")]
            rayon_tasks: Vec::new(),
            #[cfg(feature = "rayon")]
            rayon_scoped_tasks: Vec::new(),
        }
    }

    #[inline]
    pub fn dispatch<E>(&self, effect: E)
    where
        S: capabilities::CanDispatch + 'static,
        E: FnOnce(AppContextRef<App, M>) + Send + Sync + 'static,
    {
        let app_cx = APP_CONTEXT
            .get()
            .map(|any| unsafe { any.downcast_ref_unchecked::<AppContext<App, M>>() })
            .unwrap();
        app_cx
            .effect_tx
            .send(Box::new(effect))
            .map_err(|_| DispatchError::ChannelClosed)
            .unwrap();
    }

    #[must_use]
    pub fn model(&self) -> parking_lot::RwLockReadGuard<'_, M>
    where
        S: capabilities::CanReadModel,
    {
        let app_cx = APP_CONTEXT
            .get()
            .map(|any| unsafe { any.downcast_ref_unchecked::<AppContext<S, M>>() })
            .unwrap();
        app_cx.model.read()
    }

    #[must_use]
    pub fn model_mut(&self) -> parking_lot::RwLockWriteGuard<'_, M>
    where
        S: capabilities::CanModifyModel,
    {
        let app_cx = APP_CONTEXT
            .get()
            .map(|any| unsafe { any.downcast_ref_unchecked::<AppContext<S, M>>() })
            .unwrap();
        app_cx.model.write()
    }

    #[must_use]
    #[inline]
    pub fn query<F, R>(&self, f: F) -> R
    where
        S: capabilities::CanReadModel + 'static,
        F: FnOnce(&M) -> R,
    {
        f(&self.model())
    }

    #[inline]
    pub fn update<F, R>(&self, f: F) -> R
    where
        S: capabilities::CanModifyModel,
        F: FnOnce(&mut M) -> R,
    {
        let mut model = self.model_mut();
        f(&mut *model)
    }

    #[inline]
    pub fn add_resource<T>(&self, resource: T) -> Result<(), ResourceExistsError>
    where
        S: capabilities::CanModifyResources,
        T: Clone + Send + Sync + 'static,
    {
        let app_cx = APP_CONTEXT
            .get()
            .map(|any| unsafe { any.downcast_ref_unchecked::<AppContext<S, M>>() })
            .unwrap();
        let mut resources = app_cx.resources.write();
        let ty = TypeId::of::<T>();
        if resources.contains_key(&ty) {
            return Err(ResourceExistsError(std::any::type_name::<T>()));
        }
        resources.insert(ty, Box::new(resource));
        Ok(())
    }

    #[inline]
    #[must_use]
    pub fn resource<T>(&self) -> Option<T>
    where
        S: CanReadResources,
        T: Clone + Send + Sync + 'static,
    {
        let app_cx = APP_CONTEXT
            .get()
            .map(|any| unsafe { any.downcast_ref_unchecked::<AppContext<S, M>>() })
            .unwrap();
        let resources = app_cx.resources.read();
        let ty = TypeId::of::<T>();
        resources
            .get(&ty)
            .map(|boxed_value| unsafe { boxed_value.downcast_ref_unchecked::<T>() })
            .cloned()
    }

    #[inline]
    pub fn spawn<F, R>(&self, f: F) -> std::thread::JoinHandle<R>
    where
        S: CanSpawnTasks + Send + Sync + 'static,
        F: FnOnce(AppContextRef<Task, M>) -> R + Send + 'static,
        R: Send + 'static,
    {
        let cx = self.convert();
        std::thread::spawn(move || f(cx))
    }

    #[inline]
    pub fn scope<F, R>(&self, f: F) -> R
    where
        S: CanSpawnTasks + Send + Sync + 'static,
        F: for<'scope> FnOnce(&'scope std::thread::Scope<'scope, '_>, AppContextRef<Task, M>) -> R
            + Send
            + Sync,
        R: Send,
    {
        let cx = self.convert();
        std::thread::scope(|s| f(s, cx))
    }

    #[inline]
    #[cfg(feature = "tokio")]
    pub fn spawn_async<F, Fut, R>(&self, f: F) -> tokio::sync::oneshot::Receiver<R>
    where
        S: CanSpawnAsyncTask,
        F: FnOnce(AppContextRef<AsyncTask, M>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = R> + Send + 'static,
        R: Send + 'static,
    {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let cx = self.convert();

        let app_cx = APP_CONTEXT
            .get()
            .map(|any| unsafe { any.downcast_ref_unchecked::<AppContext<App, M>>() })
            .unwrap();

        app_cx.handle.spawn(async move {
            let result = f(cx).await;
            let _ = tx.send(result); // Ignore error if receiver dropped
        });

        rx
    }

    #[inline]
    #[cfg(feature = "tokio")]
    pub fn spawn_blocking<F, R>(&self, f: F) -> tokio::task::JoinHandle<R>
    where
        S: CanSpawnAsyncTask,
        F: FnOnce(AppContextRef<Task, M>) -> R + Send + Sync + 'static,
        R: Send + 'static,
    {
        let cx = self.convert();
        let app_cx = APP_CONTEXT
            .get()
            .map(|any| unsafe { any.downcast_ref_unchecked::<AppContext<App, M>>() })
            .unwrap();

        app_cx.handle.spawn_blocking(move || f(cx))
    }

    #[inline]
    #[cfg(feature = "rayon")]
    pub fn spawn_rayon<F>(&self, f: F)
    where
        S: CanSpawnRayonTask,
        F: FnOnce(AppContextRef<Task, M>) + Send + 'static,
    {
        let cx = self.convert();
        let app_cx = APP_CONTEXT
            .get()
            .map(|any| unsafe { any.downcast_ref_unchecked::<AppContext<App, M>>() })
            .unwrap();
        app_cx.pool.spawn(move || f(cx));
    }

    #[inline]
    #[cfg(feature = "rayon")]
    pub fn rayon_scope<F, R>(&self, f: F) -> R
    where
        S: CanSpawnRayonTask + Send + Sync + 'static,
        F: FnOnce(&rayon::Scope<'_>, AppContextRef<Task, M>) -> R + Send + 'static,
        R: Send + 'static,
    {
        let cx = self.convert();
        let app_cx = APP_CONTEXT
            .get()
            .map(|any| unsafe { any.downcast_ref_unchecked::<AppContext<App, M>>() })
            .unwrap();
        app_cx.pool.scope(|s| f(s, cx))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[derive(Debug)]
    struct TestModel {
        counter: i32,
    }

    #[test]
    #[cfg(not(feature = "tokio"))]
    fn test_model() {
        let model = TestModel { counter: 0 };
        let cx = AppContext::builder(model)
            .resource("test_resource".to_string())
            .build();
        cx.run();
        let x = cx.model().counter;
        assert!(x == 0);
        cx.model_mut().counter += 1;
        assert!(cx.model().counter == 1);
    }
    #[test]
    #[cfg(not(feature = "tokio"))]
    fn test_app_context_current() {
        let model = TestModel { counter: 0 };
        let cx = AppContext::builder(model).build();
        cx.run();

        let current_cx: AppContextRef<App, TestModel> = AppContext::current();
        assert_eq!(current_cx.query(|m| m.counter), 0);

        cx.update(|m| m.counter += 1);
        assert_eq!(current_cx.query(|m| m.counter), 1);

        // Verify both contexts reference same underlying storage
        current_cx.update(|m| m.counter += 1);
        assert_eq!(cx.query(|m| m.counter), 2);
    }

    #[test]
    #[cfg(not(feature = "tokio"))]
    fn test_app_context_builder() {
        let model = TestModel { counter: 0 };
        let cx = AppContext::builder(model)
            .resource("test_resource".to_string())
            .build();

        assert_eq!(cx.query(|m| m.counter), 0);
        assert_eq!(cx.resource::<String>().unwrap(), "test_resource");
    }

    #[test]
    #[cfg(not(feature = "tokio"))]
    fn test_app_context_query() {
        let model = TestModel { counter: 0 };
        let cx = AppContext::builder(model).build();

        cx.update(|m| m.counter += 1);
        assert_eq!(cx.query(|m| m.counter), 1);
    }

    #[test]
    #[cfg(not(feature = "tokio"))]
    fn test_app_context_resource_management() {
        let model = TestModel { counter: 0 };
        let cx = AppContext::builder(model).build();

        cx.add_resource(42i32).unwrap();
        assert_eq!(cx.resource::<i32>().unwrap(), 42);

        assert!(cx.add_resource(43i32).is_err());
    }

    #[test]
    #[cfg(not(feature = "tokio"))]
    fn test_app_context_run_and_stop() {
        let model = TestModel { counter: 0 };
        let cx = AppContext::builder(model).build();

        cx.run();
        assert!(cx.is_running());

        cx.stop().unwrap();
        assert!(!cx.is_running());

        assert!(cx.stop().is_err());
    }

    #[test]
    #[cfg(not(feature = "tokio"))]
    fn test_task_context() {
        let model = TestModel { counter: 0 };
        let cx = AppContext::builder(model).build();
        cx.run();

        let handle = cx.spawn(|task_cx| {
            task_cx.dispatch(|app_cx| {
                app_cx.update(|m| {
                    m.counter += 1;
                });
            });
            42
        });
        std::thread::sleep(std::time::Duration::from_millis(1000));

        assert_eq!(handle.join().unwrap(), 42);
        assert_eq!(cx.query(|m| m.counter), 1);
    }

    #[test]
    #[cfg(feature = "rayon")]
    fn test_rayon_integration() {
        use rayon::prelude::*;

        let model = TestModel { counter: 0 };
        let cx = AppContext::builder(model).build();
        cx.run();

        let counter = Arc::new(std::sync::Mutex::new(0));

        (0..100).into_par_iter().for_each(|_| {
            let counter_clone = Arc::clone(&counter);
            cx.spawn_rayon(move |task_cx| {
                let mut lock = counter_clone.lock().unwrap();
                *lock += 1;
                task_cx.dispatch(|app_cx| {
                    app_cx.update(|m| m.counter += 1);
                });
            });
        });

        // Wait for all tasks to complete
        while cx.query(|m| m.counter) < 100 {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        assert_eq!(*counter.lock().unwrap(), 100);
        assert_eq!(cx.query(|m| m.counter), 100);
    }


    #[test]
    #[cfg(not(feature = "tokio"))]
    fn test_threading_safety() {
        let model = TestModel { counter: 0 };
        let cx = AppContext::builder(model).build();
        cx.run();

        let counter = Arc::new(std::sync::atomic::AtomicI32::new(0));
        let threads: Vec<_> = (0..10)
            .map(|_| {
                let counter_clone = Arc::clone(&counter);
                std::thread::spawn(move || {
                    for _ in 0..1000 {
                        counter_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        cx.model_mut().counter += 1;
                        cx.dispatch(|app_cx| {
                            app_cx.update(|m| {
                                m.counter += 1;
                            });
                        });
                    }
                })
            })
            .collect();

        for thread in threads {
            thread.join().unwrap();
        }
        cx.dispatch(|cx| cx.stop().unwrap());
        cx.wait_for_stop().unwrap();

        // Wait for dispatched effects
        std::thread::sleep(std::time::Duration::from_secs(3));

        assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 10000);
        assert_eq!(cx.query(|m| m.counter), 20000);
    }

    #[test]
    #[cfg(not(feature = "tokio"))]
    fn test_scope() {
        let model = TestModel { counter: 0 };
        let cx = AppContext::builder(model).build();
        cx.run();

        let counter = Arc::new(std::sync::atomic::AtomicI32::new(0));

        let counter_clone = Arc::clone(&counter);
        cx.scope(move |scope, task_cx| {
            scope.spawn(move || {
                counter_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            });
            task_cx.dispatch(|app_cx| {
                app_cx.update(|m| {
                    m.counter += 1;
                });
            });
        });

        // Wait for effects to complete
        std::thread::sleep(std::time::Duration::from_secs(1));

        assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert_eq!(cx.query(|m| m.counter), 1);
    }

    #[test]
    fn test_capabilities() {
        fn test_read_model<C: CanReadModel>(cx: AppContextRef<C, TestModel>) {
            assert_eq!(cx.query(|m| m.counter), 0);
        }

        fn test_modify_model<C: CanModifyModel>(cx: AppContextRef<C, TestModel>) {
            cx.update(|m| m.counter += 1);
            assert_eq!(cx.query(|m| m.counter), 1);
        }

        fn test_add_resource<C: CanModifyResources>(cx: AppContextRef<C, TestModel>) {
            cx.add_resource(42i32).unwrap();
            assert_eq!(cx.resource::<i32>().unwrap(), 42);
        }

        fn test_read_resource<C: CanReadResources>(cx: AppContextRef<C, TestModel>) {
            assert_eq!(cx.resource::<i32>().unwrap(), 42);
        }

        let model = TestModel { counter: 0 };
        let cx = AppContext::builder(model).build();
        cx.run();

        test_read_model(cx);
        test_modify_model(cx);
        test_add_resource(cx);
        test_read_resource(cx);
    }

    #[tokio::test]
    #[cfg(feature = "tokio")]
    async fn test_tokio_integration() {
        let model = TestModel { counter: 0 };
        let cx = AppContext::builder(model)
            .handle(tokio::runtime::Handle::current())
            .build();
        cx.run();

        let counter = Arc::new(tokio::sync::Mutex::new(0));

        let handles: Vec<_> = (0..100)
            .map(|_| {
                let counter_clone = Arc::clone(&counter);
                cx.spawn_async(move |async_cx| async move {
                    let mut lock = counter_clone.lock().await;
                    *lock += 1;
                    async_cx.dispatch(move |app_cx| {
                        app_cx.update(|m| m.counter += 1);
                    });
                })
            })
            .collect();

        for handle in handles {
            handle.await.unwrap();
        }

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        assert_eq!(*counter.lock().await, 100);
        assert_eq!(cx.query(|m| m.counter), 100);
    }

    #[tokio::test]
    #[cfg(feature = "tokio")]
    async fn test_app_context_async() {
        use parking_lot::Mutex;

        let model = TestModel { counter: 0 };
        let cx = AppContext::builder(model).build();

        let counter = Arc::new(Mutex::new(0));
        let counter_clone = Arc::clone(&counter);

        cx.dispatch(move |_| {
            let mut lock = counter_clone.lock();
            *lock += 1;
        });

        cx.handle_effects();

        assert_eq!(*counter.lock(), 1);
    }

    #[tokio::test]
    #[cfg(feature = "tokio")]
    async fn test_effect_builder() {
        let model = TestModel { counter: 0 };
        let cx = AppContext::builder(model).build();

        cx.run();

        let shared_counter = Arc::new(std::sync::atomic::AtomicI32::new(0));
        let counter = Arc::clone(&shared_counter);

        let effect = cx
            .effect()
            .update(|m| m.counter += 1)
            .spawn(|task_cx| {
                task_cx.dispatch(|app_cx| {
                    app_cx.update(|m| m.counter += 1);
                });
            })
            .spawn_blocking(|task_cx| {
                task_cx.dispatch(|app_cx| {
                    app_cx.update(|m| m.counter += 1);
                });
            })
            .scope(move |scope, task_cx| {
                for _ in 0..5 {
                    let counter = Arc::clone(&counter);
                    scope.spawn(move || {
                        counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        task_cx.dispatch(|app_cx| {
                            app_cx.update(|m| m.counter += 1);
                        });
                    });
                }
            })
            .spawn_async(|async_cx| async move {
                async_cx.dispatch(|app_cx| {
                    app_cx.update(|m| m.counter += 1);
                });
            });

        #[cfg(feature = "rayon")]
        let effect = effect
            .spawn_rayon(|task_cx| {
                task_cx.dispatch(|app_cx| {
                    app_cx.update(|m| m.counter += 1);
                });
            })
            .rayon_scope(|_scope, task_cx| {
                task_cx.dispatch(|app_cx| {
                    app_cx.update(|m| m.counter += 1);
                });
            });

        effect.dispatch();

        // Wait for effects to complete
        std::thread::sleep(std::time::Duration::from_secs(1));

        assert_eq!(shared_counter.load(std::sync::atomic::Ordering::SeqCst), 5);

        #[cfg(all(feature = "tokio", feature = "rayon"))]
        assert_eq!(cx.query(|m| m.counter), 10);

        #[cfg(all(feature = "tokio", not(feature = "rayon")))]
        assert_eq!(cx.query(|m| m.counter), 8);
    }

    #[test]
    #[cfg(not(feature = "tokio"))]
    fn test_wait_for_stop() {
        let model = TestModel { counter: 0 };
        let cx = AppContext::builder(model).build();

        cx.run();
        assert!(cx.is_running());

        let cx_clone = cx;
        let handle = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(100));
            cx_clone.stop().unwrap();
        });

        cx.wait_for_stop().unwrap();
        handle.join().unwrap();
        assert!(!cx.is_running());
    }
    // #[ignore]
    #[test]
    fn benchmark_dispatch_model_read() {
        use std::time::Instant;

        let model = TestModel { counter: 0 };
        let cx = AppContext::builder(model).build();
        cx.run();

        const ITERATIONS: usize = 1_000_000;
        const RUNS: usize = 10;

        let mut best_duration = std::time::Duration::from_secs(u64::MAX);

        for _ in 0..RUNS {
            let start = Instant::now();

            for _ in 0..ITERATIONS {
                cx.dispatch(|app_cx| {
                    app_cx.query(|m| m.counter);
                });
            }

            let duration = start.elapsed();
            best_duration = best_duration.min(duration);
        }

        cx.dispatch(|cx| {
            cx.stop();
        });

        cx.wait_for_stop().unwrap();

        let ops_per_sec = ITERATIONS as f64 / best_duration.as_secs_f64();

        println!(
            "Dispatch with model read benchmark:\n\
             {} iterations in {:?} (best of {} runs)\n\
             {:.2} ops/sec",
            ITERATIONS, best_duration, RUNS, ops_per_sec
        );
    }
}
