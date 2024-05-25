use std::{fmt, sync::Arc};

use async_trait::async_trait;
use parking_lot::RwLock;

#[async_trait]
pub trait Eventide: Sized + Copy + Send + Sync + 'static {
    type Model: Send + Sync + 'static;
    type Capabilities: Send + Sync + 'static;

    type Effect: Send + fmt::Debug + 'static;

    fn eve(model: Self::Model, caps: Self::Capabilities) -> Eve<Self> {
        let (effect_tx, effect_rx) = tokio::sync::mpsc::unbounded_channel();
        let cancelation_token = tokio_util::sync::CancellationToken::new();
        let model = Arc::new(RwLock::new(model));
        #[cfg(feature = "rayon")]
        let pool = rayon::ThreadPoolBuilder::new().build().unwrap();

        let eve = Eve {
            model: Arc::clone(&model),
            capabilities: Arc::new(caps),
            effect_tx,
            cancelation_token: Some(cancelation_token.clone()),
            #[cfg(feature = "rayon")]
            pool: Arc::new(pool),
        };

        run_effect_loop(eve.clone(), effect_rx, cancelation_token);

        eve
    }

    async fn handle_effect(event: Self::Effect, ctx: EffectContext<Self>);
}

#[derive(Debug)]
pub struct Eve<A: Eventide> {
    pub(crate) model: Arc<RwLock<A::Model>>,
    pub(crate) capabilities: Arc<A::Capabilities>,
    pub(crate) effect_tx: tokio::sync::mpsc::UnboundedSender<A::Effect>,
    pub(crate) cancelation_token: Option<tokio_util::sync::CancellationToken>,
    #[cfg(feature = "rayon")]
    pub(crate) pool: Arc<rayon::ThreadPool>,
}

impl<A: Eventide> Clone for Eve<A> {
    fn clone(&self) -> Self {
        Self {
            model: Arc::clone(&self.model),
            capabilities: Arc::clone(&self.capabilities),
            effect_tx: self.effect_tx.clone(),
            cancelation_token: self.cancelation_token.clone(),
            #[cfg(feature = "rayon")]
            pool: Arc::clone(&self.pool),
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[error("Event loop already stopped")]
pub struct AlreadyStopped;

impl<A: Eventide> Eve<A> {
    pub fn stop(&mut self) -> Result<(), AlreadyStopped> {
        if let Some(cancelation_token) = self.cancelation_token.take() {
            cancelation_token.cancel();
            Ok(())
        } else {
            Err(AlreadyStopped)
        }
    }

    #[must_use]
    pub fn is_running(&self) -> bool {
        self.cancelation_token.is_some()
    }

    pub fn dispatch<T>(&self, effect: T)
    where
        T: Into<A::Effect>,
    {
        self.effect_tx.send(effect.into()).unwrap();
    }

    pub fn model(&self) -> parking_lot::RwLockReadGuard<A::Model> {
        self.model.read()
    }

    pub fn model_mut(&self) -> parking_lot::RwLockWriteGuard<A::Model> {
        self.model.write()
    }

    pub fn with_model<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&A::Model) -> R,
    {
        f(&*self.model.read())
    }

    pub fn with_model_mut<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut A::Model) -> R,
    {
        f(&mut *self.model.write())
    }

    #[must_use]
    pub fn capabilities(&self) -> &A::Capabilities {
        self.capabilities.as_ref()
    }

    pub fn spawn<F, Fut, R>(&self, f: F) -> tokio::task::JoinHandle<R>
    where
        F: FnOnce(AsyncTaskContext<A>) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = R> + Send,
        R: Send + 'static,
    {
        let ctx = AsyncTaskContext::from(self);
        tokio::spawn(async move { f(ctx).await })
    }

    pub fn spawn_blocking<F, R>(&self, f: F) -> tokio::task::JoinHandle<R>
    where
        F: FnOnce(TaskContext<A>) -> R + Send + Sync + 'static,
        R: Send + 'static,
    {
        let ctx = TaskContext::from(self);
        tokio::task::spawn_blocking(|| f(ctx))
    }

    #[cfg(feature = "rayon")]
    pub fn spawn_rayon<F>(&self, f: F)
    where
        F: FnOnce(TaskContext<A>) + Send + 'static,
    {
        let ctx = TaskContext::from(self);
        self.pool.spawn(move || f(ctx));
    }

    #[cfg(feature = "rayon")]
    pub fn scope<F, T>(&self, f: F)
    where
        F: FnOnce(&rayon::Scope<'_>, TaskContext<A>) + Send,
        T: Send,
    {
        let ctx = TaskContext::from(self);
        self.pool.scope(|s| f(s, ctx));
    }
}

pub struct EffectContext<A: Eventide> {
    eve: Eve<A>,
}

impl<A: Eventide> EffectContext<A> {
    #[inline]
    pub fn stop(&mut self) -> Result<(), AlreadyStopped> {
        self.eve.stop()
    }

    #[inline]
    pub fn dispatch<T>(&self, effect: T)
    where
        T: Into<A::Effect>,
    {
        self.eve.dispatch(effect);
    }

    #[inline]
    pub fn with_model<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&A::Model) -> R,
    {
        self.eve.with_model(f)
    }

    #[inline]
    pub fn with_model_mut<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut A::Model) -> R,
    {
        self.eve.with_model_mut(f)
    }

    #[inline]
    #[must_use]
    pub fn capabilities(&self) -> &A::Capabilities {
        self.eve.capabilities()
    }

    pub async fn with_capabilities<F, Fut, R>(&self, f: F) -> R
    where
        F: FnOnce(Arc<A::Capabilities>) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = R> + Send,
        R: Send + 'static,
    {
        let caps = Arc::clone(&self.eve.capabilities);
        f(caps).await
    }

    #[inline]
    pub fn spawn<F, Fut, R>(&self, f: F) -> tokio::task::JoinHandle<R>
    where
        F: FnOnce(AsyncTaskContext<A>) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = R> + Send,
        R: Send + 'static,
    {
        self.eve.spawn(f)
    }

    #[inline]
    pub fn spawn_blocking<F, R>(&self, f: F) -> tokio::task::JoinHandle<R>
    where
        F: FnOnce(TaskContext<A>) -> R + Send + Sync + 'static,
        R: Send + 'static,
    {
        self.eve.spawn_blocking(f)
    }

    #[inline]
    #[cfg(feature = "rayon")]
    pub fn spawn_rayon<F>(&self, f: F)
    where
        F: FnOnce(TaskContext<A>) + Send + 'static,
    {
        self.eve.spawn_rayon(f);
    }

    #[inline]
    #[cfg(feature = "rayon")]
    pub fn scope<F, T>(&self, f: F)
    where
        F: FnOnce(&rayon::Scope<'_>, TaskContext<A>) + Send,
        T: Send,
    {
        self.eve.scope::<F, T>(f);
    }
}

impl<A: Eventide> Clone for EffectContext<A> {
    fn clone(&self) -> Self {
        Self {
            eve: self.eve.clone(),
        }
    }
}

impl<A: Eventide> From<&EffectContext<A>> for EffectContext<A> {
    fn from(value: &EffectContext<A>) -> Self {
        value.clone()
    }
}

impl<A: Eventide> From<&Eve<A>> for EffectContext<A> {
    fn from(value: &Eve<A>) -> Self {
        Self { eve: value.clone() }
    }
}

pub struct TaskContext<A: Eventide> {
    eve: Eve<A>,
}

impl<A: Eventide> TaskContext<A> {
    #[inline]
    pub fn dispatch<T>(&self, effect: T)
    where
        T: Into<A::Effect>,
    {
        self.eve.dispatch(effect);
    }

    #[inline]
    pub fn with_model<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&A::Model) -> R,
    {
        self.eve.with_model(f)
    }

    #[inline]
    pub fn with_model_mut<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut A::Model) -> R,
    {
        self.eve.with_model_mut(f)
    }

    #[inline]
    #[must_use]
    pub fn capabilities(&self) -> &A::Capabilities {
        self.eve.capabilities()
    }

    #[inline]
    #[cfg(feature = "rayon")]
    pub fn spawn_rayon<F>(&self, f: F)
    where
        F: FnOnce(TaskContext<A>) + Send + 'static,
    {
        self.eve.spawn_rayon(f);
    }

    #[inline]
    #[cfg(feature = "rayon")]
    pub fn scope<F, T>(&self, f: F)
    where
        F: FnOnce(&rayon::Scope<'_>, TaskContext<A>) + Send,
        T: Send,
    {
        self.eve.scope::<F, T>(f);
    }
}

impl<A: Eventide> Clone for TaskContext<A> {
    fn clone(&self) -> Self {
        Self {
            eve: self.eve.clone(),
        }
    }
}

impl<A: Eventide> From<&Eve<A>> for TaskContext<A> {
    fn from(value: &Eve<A>) -> Self {
        Self { eve: value.clone() }
    }
}

impl<A: Eventide> From<&EffectContext<A>> for TaskContext<A> {
    fn from(value: &EffectContext<A>) -> Self {
        Self {
            eve: value.eve.clone(),
        }
    }
}

impl<A: Eventide> From<&TaskContext<A>> for EffectContext<A> {
    fn from(value: &TaskContext<A>) -> Self {
        Self {
            eve: value.eve.clone(),
        }
    }
}

pub struct AsyncTaskContext<A: Eventide> {
    eve: Eve<A>,
}

impl<A: Eventide> AsyncTaskContext<A> {
    #[inline]
    pub fn dispatch<T>(&self, effect: T)
    where
        T: Into<A::Effect>,
    {
        self.eve.dispatch(effect);
    }

    #[inline]
    pub fn with_model<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&A::Model) -> R,
    {
        self.eve.with_model(f)
    }

    #[inline]
    pub fn with_model_mut<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut A::Model) -> R,
    {
        self.eve.with_model_mut(f)
    }

    #[inline]
    #[must_use]
    pub fn capabilities(&self) -> &A::Capabilities {
        self.eve.capabilities()
    }

    #[inline]
    pub fn spawn<F, Fut, R>(&self, f: F) -> tokio::task::JoinHandle<R>
    where
        F: FnOnce(AsyncTaskContext<A>) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = R> + Send,
        R: Send + 'static,
    {
        self.eve.spawn(f)
    }

    #[inline]
    pub fn spawn_blocking<F, R>(&self, f: F) -> tokio::task::JoinHandle<R>
    where
        F: FnOnce(TaskContext<A>) -> R + Send + Sync + 'static,
        R: Send + 'static,
    {
        self.eve.spawn_blocking(f)
    }

    #[inline]
    #[cfg(feature = "rayon")]
    pub fn spawn_rayon<F>(&self, f: F)
    where
        F: FnOnce(TaskContext<A>) + Send + 'static,
    {
        self.eve.spawn_rayon(f);
    }

    #[inline]
    #[cfg(feature = "rayon")]
    pub fn scope<F, T>(&self, f: F)
    where
        F: FnOnce(&rayon::Scope<'_>, TaskContext<A>) + Send,
        T: Send,
    {
        self.eve.scope::<F, T>(f);
    }
}

impl<A: Eventide> Clone for AsyncTaskContext<A> {
    fn clone(&self) -> Self {
        Self {
            eve: self.eve.clone(),
        }
    }
}

impl<A: Eventide> From<&Eve<A>> for AsyncTaskContext<A> {
    fn from(value: &Eve<A>) -> Self {
        Self { eve: value.clone() }
    }
}

impl<A: Eventide> From<&EffectContext<A>> for AsyncTaskContext<A> {
    fn from(value: &EffectContext<A>) -> Self {
        Self {
            eve: value.eve.clone(),
        }
    }
}

impl<A: Eventide> From<&AsyncTaskContext<A>> for TaskContext<A> {
    fn from(value: &AsyncTaskContext<A>) -> Self {
        Self {
            eve: value.eve.clone(),
        }
    }
}

impl<A: Eventide> From<&AsyncTaskContext<A>> for EffectContext<A> {
    fn from(value: &AsyncTaskContext<A>) -> Self {
        Self {
            eve: value.eve.clone(),
        }
    }
}

fn run_effect_loop<A: Eventide + 'static>(
    eve: Eve<A>,
    mut effect_rx: tokio::sync::mpsc::UnboundedReceiver<A::Effect>,
    cancelation_token: tokio_util::sync::CancellationToken,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            tokio::select! {
                () = cancelation_token.cancelled() => break,
                Some(request) = effect_rx.recv() => {
                    A::handle_effect(request, EffectContext::from(&eve)).await;
                }
            }
        }
    })
}

#[cfg(test)]
mod test {

    #[tokio::test]
    async fn spawn_test() {
        std::thread::spawn(|| {
            println!("hello from thread");
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async move {
                tokio::spawn(async move {
                    println!("hello from tokio");
                })
                .await
                .unwrap();
            });
        });
        std::thread::sleep(std::time::Duration::from_secs(3));
    }
}
