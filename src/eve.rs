use std::{
    any::{Any, TypeId},
    marker::PhantomData,
    sync::OnceLock,
};

use atomic_take::AtomicTake;
use parking_lot::RwLock;
use rustc_hash::FxHashMap;

pub type Effect<M> = Box<dyn FnOnce(AppContext<M>) + Send + Sync>;
pub trait Model: Send + Sync + 'static {}
impl<T> Model for T where T: Send + Sync + 'static {}

static RUNTIME: OnceLock<Runtime> = OnceLock::new();

#[derive(Debug)]
pub struct Runtime {
    model: RwLock<Box<dyn Any + Send + Sync>>,
    capabilities: RwLock<FxHashMap<TypeId, Box<dyn Any + Send + Sync>>>,
    resources: RwLock<FxHashMap<TypeId, Box<dyn Any + Send + Sync>>>,
    effect_tx: crossbeam_channel::Sender<Box<dyn Any + Send + Sync>>,
    cancelation_tx: AtomicTake<crossbeam_channel::Sender<()>>,
    #[cfg(feature = "rayon")]
    pool: rayon::ThreadPool,
    #[cfg(feature = "tokio")]
    rt: tokio::runtime::Runtime,
}

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
        let rt = self.rt.unwrap_or_else(|| {
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .unwrap()
        });

        let runtime = Runtime {
            model: RwLock::new(Box::new(self.model)),
            capabilities: RwLock::new(self.capabilities),
            resources: RwLock::new(self.resources),
            effect_tx,
            cancelation_tx: AtomicTake::new(cancelation_tx),
            #[cfg(feature = "rayon")]
            pool,
            #[cfg(feature = "tokio")]
            rt,
        };

        RUNTIME.set(runtime).unwrap();
        let cx = AppContext {
            phantom: PhantomData,
        };

        run_effect_loop(cx, effect_rx, cancelation_rx);

        cx
    }
}

#[derive(Debug)]
pub struct AppContext<M: Model> {
    phantom: PhantomData<M>,
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

    #[inline]
    pub fn stop(&mut self) -> Result<(), EventideError> {
        if let Some(cancelation_token) = RUNTIME.get().unwrap().cancelation_tx.take() {
            cancelation_token.send(()).unwrap();
            Ok(())
        } else {
            Err(EventideError::AlreadyStopped)
        }
    }

    #[inline]
    #[must_use]
    pub fn is_running(&self) -> bool {
        !RUNTIME.get().unwrap().cancelation_tx.is_taken()
    }

    #[inline]
    pub fn dispatch<E>(&self, effect: E)
    where
        E: FnOnce(AppContext<M>) + Send + Sync + 'static,
    {
        RUNTIME
            .get()
            .unwrap()
            .effect_tx
            .send(Box::new(Box::new(effect) as Effect<M>))
            .unwrap();
    }

    #[inline]
    pub fn provide_capability<T>(&self, capability: T)
    where
        T: Clone + Send + Sync + 'static,
    {
        let ty = TypeId::of::<T>();
        RUNTIME
            .get()
            .unwrap()
            .capabilities
            .write()
            .insert(ty, Box::new(capability));
    }

    #[inline]
    #[must_use]
    pub fn expect_capability<T>(&self) -> T
    where
        T: Clone + Send + Sync + 'static,
    {
        let ty = TypeId::of::<T>();
        RUNTIME
            .get()
            .unwrap()
            .capabilities
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
        RUNTIME
            .get()
            .unwrap()
            .resources
            .write()
            .insert(ty, Box::new(resource));
    }

    #[inline]
    #[must_use]
    pub fn expect_resource<T>(&self) -> T
    where
        T: Clone + Send + Sync + 'static,
    {
        let ty = TypeId::of::<T>();
        RUNTIME
            .get()
            .unwrap()
            .resources
            .read()
            .get(&ty)
            .map(|boxed_value| boxed_value.downcast_ref::<T>().unwrap())
            .cloned()
            .unwrap()
    }

    #[inline]
    pub fn model(
        &self,
    ) -> parking_lot::lock_api::MappedRwLockReadGuard<'_, parking_lot::RawRwLock, M> {
        let model_guard = RUNTIME.get().unwrap().model.read();
        parking_lot::RwLockReadGuard::map(model_guard, |model| unsafe {
            model.downcast_ref_unchecked::<M>()
        })
    }

    #[inline]
    pub fn model_mut(&self) -> parking_lot::MappedRwLockWriteGuard<'_, M> {
        let model_guard = RUNTIME
            .get()
            .expect("Runtime not initialized")
            .model
            .write();
        parking_lot::RwLockWriteGuard::map(model_guard, |model| unsafe {
            model.downcast_mut_unchecked::<M>()
        })
    }

    #[inline]
    pub fn with_model<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&M) -> R,
    {
        f(&self.model())
    }

    #[inline]
    pub fn with_model_mut<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut M) -> R,
    {
        f(&mut self.model_mut())
    }

    #[inline]
    #[cfg(feature = "tokio")]
    pub fn task<F, Fut, R>(&self, f: F) -> tokio::task::JoinHandle<R>
    where
        F: FnOnce(AsyncTaskContext<M>) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = R> + Send,
        R: Send + 'static,
    {
        let cx = AsyncTaskContext::from(*self);
        RUNTIME.get().unwrap().rt.spawn(async move { f(cx).await })
    }

    #[inline]
    #[cfg(feature = "tokio")]
    pub fn task_blocking<F, R>(&self, f: F) -> tokio::task::JoinHandle<R>
    where
        F: FnOnce(TaskContext<M>) -> R + Send + Sync + 'static,
        R: Send + 'static,
    {
        let cx = TaskContext::from(*self);
        RUNTIME.get().unwrap().rt.spawn_blocking(move || f(cx))
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
        RUNTIME.get().unwrap().pool.spawn(move || f(cx));
    }

    #[inline]
    #[cfg(feature = "rayon")]
    pub fn rayon_scope<F, T>(&self, f: F)
    where
        F: FnOnce(&rayon::Scope<'_>, TaskContext<M>) + Send,
        T: Send,
    {
        let cx = TaskContext::from(*self);
        RUNTIME.get().unwrap().pool.scope(|s| f(s, cx));
    }
}

pub struct TaskContext<M: Model>(AppContext<M>);

impl<M: Model> Clone for TaskContext<M> {
    fn clone(&self) -> Self {
        Self(self.0)
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
        F: FnOnce(AppContext<M>) + Send + Sync + 'static,
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
        Self(self.0)
    }
}

#[cfg(feature = "tokio")]
impl<M: Model> AsyncTaskContext<M> {
    #[inline]
    pub fn dispatch<F>(&self, effect: F)
    where
        F: FnOnce(AppContext<M>) + Send + Sync + 'static,
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
        F: FnOnce(AsyncTaskContext<M>) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = R> + Send,
        R: Send + 'static,
    {
        self.0.task(f)
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

fn run_effect_loop<M: Model>(
    cx: AppContext<M>,
    effect_rx: crossbeam_channel::Receiver<Box<dyn Any + Send + Sync>>,
    cancelation_rx: crossbeam_channel::Receiver<()>,
) {
    std::thread::spawn(move || {
        let mut is_canceled = false;
        while !is_canceled {
            crossbeam_channel::select! {
                recv(cancelation_rx) -> _ => is_canceled = true,
                recv(effect_rx) -> maybe_effect => {
                    if let Ok(boxed_effect) = maybe_effect {
                        unsafe {
                            let effect = boxed_effect.downcast_unchecked::<Effect<M>>();
                            (effect)(cx);
                        }
                    }
                }
            }
        }
    });
}

#[cfg(test)]
mod test {
    use std::{any::Any, sync::atomic::AtomicBool};

    use crate::{
        eve::{Effect, Model},
        prelude::*,
    };

    #[test]
    fn effect_test() {
        struct MyModel;

        // // Tworzymy Box<dyn Any> przechowujący MyEffect
        // let effect: Box<dyn Any + Send + Sync> =
        //     Box::new(
        //         Box::new(|cx: AppContext<MyModel>| println!("hello from effect"))
        //             as Effect<MyModel>,
        //     );
        //
        // // Downcast do MyEffect
        // let downcasted = effect.downcast::<Effect<MyModel>>();
        //
        // // Teraz downcasted.is_ok() zwróci true
        // println!("{}", downcasted.is_ok());
        //
        // // Możemy teraz wywołać funkcję
        // if let Ok(effect) = downcasted {
        //     (effect)(cx);
        // }

        // Tworzymy Box<dyn Any> przechowujący MyEffect

        // // Downcast do MyEffect
        // let downcasted = effect.downcast::<Effect<MyModel>>();
        //
        // // Teraz downcasted.is_ok() zwróci true
        // println!("{}", downcasted.is_ok());
        //
        // let cx = AppContext::builder(MyModel).build();
        // // Możemy teraz wywołać funkcję
        // if let Ok(effect) = downcasted {
        //     (effect)(cx);
        // }

        let cx = AppContext::builder(MyModel).build();
        let mut durations = Vec::new();
        for _ in 0..100 {
            let start = std::time::Instant::now();
            let is_done = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            for _ in 0..1_000_000 {
                cx.dispatch(|_| {})
            }

            cx.dispatch({
                let is_done = is_done.clone();
                move |_| {
                    is_done.store(true, std::sync::atomic::Ordering::SeqCst);
                }
            });

            // Wait for the dispatch to complete
            while !is_done.load(std::sync::atomic::Ordering::SeqCst) {}

            let duration = start.elapsed();
            println!("Time elapsed in dispatching: {:?}", duration);
            durations.push(duration);
        }
        let total_duration: u128 = durations.iter().map(|x| x.as_millis()).sum();
        let avg = total_duration / durations.len() as u128;
        println!("Average time: {avg} ms");
        // std::thread::sleep(std::time::Duration::from_secs(1))
    }
}
