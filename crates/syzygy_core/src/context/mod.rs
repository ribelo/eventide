#[cfg(feature = "async")]
use std::future::Future;

#[cfg(feature = "async")]
pub mod r#async;
pub mod event;
pub mod thread;

pub trait Context: Sized + Clone + 'static {
    fn execute<H, T, R>(&self, h: H) -> R
    where
        H: ContextExecutor<Self, T, R>,
    {
        h.call(self)
    }
}

pub trait FromContext<C>
where
    C: Context,
{
    fn from_context(cx: &C) -> Self;
}

pub trait ContextExecutor<C, T, R>
where
    C: Context,
{
    fn call(self, cx: &C) -> R;
}

impl<C, F, R> ContextExecutor<C, (), R> for F
where
    C: Context,
    F: FnOnce() -> R,
{
    fn call(self, _cx: &C) -> R {
        (self)()
    }
}

macro_rules! impl_context_executor {
    ($($t:ident),*) => {
        impl<C, F, $($t,)* R> ContextExecutor<C, ($($t,)*), R> for F
        where
            C: Context,
            F: FnOnce($($t,)*) -> R,
            $($t: FromContext<C>,)*
        {
            fn call(self, cx: &C) -> R {
                (self)($($t::from_context(cx),)*)
            }
        }
    }
}

impl_context_executor!(T1);
impl_context_executor!(T1, T2);
impl_context_executor!(T1, T2, T3);
impl_context_executor!(T1, T2, T3, T4);
impl_context_executor!(T1, T2, T3, T4, T5);
impl_context_executor!(T1, T2, T3, T4, T5, T6);
impl_context_executor!(T1, T2, T3, T4, T5, T6, T7);
impl_context_executor!(T1, T2, T3, T4, T5, T6, T7, T8);
impl_context_executor!(T1, T2, T3, T4, T5, T6, T7, T8, T9);
impl_context_executor!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10);
impl_context_executor!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11);
impl_context_executor!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12);

#[cfg(feature = "async")]
pub trait AsyncContextExecutor<C, T, Fut, R>
where
    C: Context,
    Fut: Future<Output = R>,
{
    fn call(self, cx: &C) -> Fut;
}

#[cfg(feature = "async")]
impl<C, F, Fut, R> AsyncContextExecutor<C, (), Fut, R> for F
where
    C: Context,
    F: FnOnce() -> Fut,
    Fut: Future<Output = R>,
{
    fn call(self, _cx: &C) -> Fut {
        (self)()
    }
}

#[cfg(feature = "async")]
macro_rules! impl_async_context_executor {
    ($($t:ident),*) => {
        impl<C, F, Fut, $($t,)* R> AsyncContextExecutor<C, ($($t,)*), Fut, R> for F
        where
            C: Context,
            F: FnOnce($($t,)*) -> Fut,
            Fut: Future<Output = R>,
            $($t: FromContext<C>,)*
        {
            fn call(self, cx: &C) -> Fut {
                (self)($($t::from_context(cx),)*)
            }
        }
    }
}

#[cfg(feature = "async")]
impl_async_context_executor!(T1);
#[cfg(feature = "async")]
impl_async_context_executor!(T1, T2);
#[cfg(feature = "async")]
impl_async_context_executor!(T1, T2, T3);
#[cfg(feature = "async")]
impl_async_context_executor!(T1, T2, T3, T4);
#[cfg(feature = "async")]
impl_async_context_executor!(T1, T2, T3, T4, T5);
#[cfg(feature = "async")]
impl_async_context_executor!(T1, T2, T3, T4, T5, T6);
#[cfg(feature = "async")]
impl_async_context_executor!(T1, T2, T3, T4, T5, T6, T7);
#[cfg(feature = "async")]
impl_async_context_executor!(T1, T2, T3, T4, T5, T6, T7, T8);
#[cfg(feature = "async")]
impl_async_context_executor!(T1, T2, T3, T4, T5, T6, T7, T8, T9);
#[cfg(feature = "async")]
impl_async_context_executor!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10);
#[cfg(feature = "async")]
impl_async_context_executor!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11);
#[cfg(feature = "async")]
impl_async_context_executor!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12);

#[cfg(all(feature = "async", feature = "parallel"))]
#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::effect_bus::DispatchEffect;
    use crate::model::{Model, ModelMut};
    use crate::resource::Resource;
    use crate::spawn::{SpawnAsync, SpawnParallel, SpawnThread};
    use crate::{context::Context, syzygy::Syzygy};

    #[test]
    fn test_context_executor() {
        pub struct TestModel {
            counter: i32,
        }
        #[derive(Clone)]
        pub struct TestResource {
            name: String,
        }

        let model = TestModel { counter: 0 };
        let resource = TestResource {
            name: "test".to_string(),
        };
        let tokio_rt = tokio::runtime::Runtime::new().unwrap();
        let tokio_handle = tokio_rt.handle().clone();
        let rayon_pool = Arc::new(rayon::ThreadPoolBuilder::new().build().unwrap());

        let syzygy = Syzygy::builder()
            .model(model)
            .resource(resource)
            .tokio_handle(tokio_handle)
            .rayon_pool(rayon_pool)
            .build();

        syzygy.execute(|| println!("nop"));
        syzygy.execute(|Model(model): Model<TestModel>| println!("counter is {}", model.counter));
        syzygy.execute(|ModelMut(mut model): ModelMut<TestModel>| model.counter += 1);
        syzygy.execute(|Model(model): Model<TestModel>| println!("counter is {}", model.counter));
        syzygy.execute(|Resource(resource): Resource<TestResource>| {
            println!("name is {}", resource.name);
        });

        syzygy.dispatch(|| println!("empty dispatch")).unwrap();

        syzygy
            .dispatch(|Model(model): Model<TestModel>| {
                println!("counter from dispatch is {}", model.counter);
            })
            .unwrap();

        syzygy.spawn(|| println!("Hello from thread"));
        syzygy.spawn_task(|| async { println!("Hello from async") });
        syzygy.spawn_parallel(|| println!("Hello from rayon"));

        syzygy.handle_effects();

        syzygy.execute(|| println!("Hello, world!"));
        // std::thread::sleep(Duration::from_secs(1));
    }
}
