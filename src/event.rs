use async_trait::async_trait;
use downcast_rs::{impl_downcast, DowncastSync};
use dyn_clone::DynClone;
use std::{any::TypeId, fmt, marker::PhantomData, sync::Arc};

use crate::eve::{Eve, ToSnapshot};

pub trait Eventable: fmt::Debug + Send + Sync + DowncastSync + DynClone + 'static {}
impl_downcast!(sync Eventable);
dyn_clone::clone_trait_object!(Eventable);
impl<T> Eventable for T where T: fmt::Debug + Send + Sync + Clone + 'static {}

#[derive(Debug, Clone)]
pub struct Event {
    pub id: TypeId,
    pub inner: Arc<dyn Eventable>,
}

impl Event {
    pub fn new<T>(event: T) -> Self
    where
        T: Eventable,
    {
        Event {
            id: TypeId::of::<T>(),
            inner: Arc::new(event),
        }
    }
}

pub struct Events(pub Vec<Event>);

pub trait FromEve<'a, A>: Send + Sync
where
    A: Send + Sync + Clone + ToSnapshot + 'static,
{
    fn from_eve(eve: &'a Eve<A>) -> Self;
}

pub trait ReadHandler<A, E>: Send + Sync + DynClone
where
    A: Send + Sync + Clone + ToSnapshot + 'static,
{
    fn read(&self, event: &E, snap: <A as ToSnapshot>::Snapshot) -> Option<Events>;
}
dyn_clone::clone_trait_object!(<A, E> ReadHandler<A, E>);

// macro_rules! tuple_impls {
//     ($($t:ident),*; $f:ident) => {
//         #[async_trait]
//         impl<S, $($t),*, $f, Fut> EventHandler< S, ($($t,)*)> for $f
//         where
//             S: Send + Sync + Clone + 'static,
//             $f: Fn($($t),*) -> Fut + Send + Sync + Clone,
//             $($t: FromEventContext<S>,)*
//             Fut: Future<Output = ()> + Send,
//         {
//             async fn call(&self, context: &EventContext<S>) {
//                 (self)($(<$t>::from_context(&context).await,)*).await;
//             }
//         }
//     }
// }
//
// macro_rules! impl_handler {
//     (($($t:ident),*), $f:ident) => {
//         tuple_impls!($($t),*; $f);
//     };
// }
//
// impl_handler!((T1), F);
// impl_handler!((T1, T2), F);
// impl_handler!((T1, T2, T3), F);
// impl_handler!((T1, T2, T3, T4), F);
// impl_handler!((T1, T2, T3, T4, T5), F);
// impl_handler!((T1, T2, T3, T4, T5, T6), F);
// impl_handler!((T1, T2, T3, T4, T5, T6, T7), F);
// impl_handler!((T1, T2, T3, T4, T5, T6, T7, T8), F);
// impl_handler!((T1, T2, T3, T4, T5, T6, T7, T8, T9), F);
// impl_handler!((T1, T2, T3, T4, T5, T6, T7, T8, T9, T10), F);
// impl_handler!((T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11), F);
// impl_handler!((T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12), F);

impl<A, E, F> ReadHandler<A, E> for F
where
    A: Send + Sync + Clone + ToSnapshot + 'static,
    F: Fn(&E, <A as ToSnapshot>::Snapshot) -> Option<Events> + Send + Sync + Clone,
    E: Eventable,
{
    fn read(&self, event: &E, snap: <A as ToSnapshot>::Snapshot) -> Option<Events> {
        (self)(event, snap)
    }
}

pub trait ReadHandlerFn<A>: Send + Sync + DynClone
where
    A: Send + Sync + Clone + ToSnapshot + 'static,
{
    fn read(&self, event: Arc<dyn Eventable>, snap: <A as ToSnapshot>::Snapshot) -> Option<Events>;
}
dyn_clone::clone_trait_object!(<A> ReadHandlerFn<A>);

impl<A> fmt::Debug for dyn ReadHandlerFn<A> + Send + Sync {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("<ReadHandlerFn>").finish()
    }
}

pub(crate) struct ReadHandlerWrapper<A, H, E>
where
    A: Send + Sync + Clone + ToSnapshot + 'static,
    H: ReadHandler<A, E> + Copy,
{
    pub handler: H,
    pub phantom: PhantomData<(A, E)>,
}

impl<A, H, E> Copy for ReadHandlerWrapper<A, H, E>
where
    A: Send + Sync + Clone + ToSnapshot + 'static,
    H: ReadHandler<A, E> + Copy,
    E: Eventable,
{
}

impl<A, H, E> Clone for ReadHandlerWrapper<A, H, E>
where
    A: Send + Sync + Clone + ToSnapshot + 'static,
    H: ReadHandler<A, E> + Copy,
    E: Eventable,
{
    fn clone(&self) -> Self {
        *self
    }
}

impl<A, H, E> ReadHandlerFn<A> for ReadHandlerWrapper<A, H, E>
where
    A: Send + Sync + Clone + ToSnapshot + 'static,
    H: ReadHandler<A, E> + Copy,
    E: Eventable,
{
    fn read(&self, event: Arc<dyn Eventable>, snap: <A as ToSnapshot>::Snapshot) -> Option<Events> {
        event
            .downcast_ref::<E>()
            .map(|event| self.handler.read(event, snap))
            .unwrap()
    }
}

pub trait WriteHandler<A, T>: Send + Sync + DynClone
where
    A: Send + Sync + Clone + 'static,
{
    fn write(&self, event: &T, app: &mut A) -> Option<Events>;
}
dyn_clone::clone_trait_object!(<A, E> WriteHandler<A, E>);

impl<A, E, F> WriteHandler<A, E> for F
where
    A: Send + Sync + Clone + 'static,
    F: Fn(&E, &mut A) -> Option<Events> + Send + Sync + Clone,
    E: Eventable,
{
    fn write(&self, event: &E, app: &mut A) -> Option<Events> {
        (self)(event, app)
    }
}

pub trait WriteHandlerFn<A>: Send + Sync + DynClone
where
    A: Send + Sync + Clone + 'static,
{
    fn write(&self, event: Arc<dyn Eventable>, app: &mut A) -> Option<Events>;
}
dyn_clone::clone_trait_object!(<A> WriteHandlerFn<A>);

impl<A> fmt::Debug for dyn WriteHandlerFn<A> + Send + Sync {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("<WriteHandlerFn>").finish()
    }
}

pub(crate) struct WriteHandlerWrapper<A, H, E>
where
    A: Send + Sync + Clone + 'static,
    H: WriteHandler<A, E> + Copy,
{
    pub handler: H,
    pub phantom: PhantomData<(A, E)>,
}

impl<A, H, E> Copy for WriteHandlerWrapper<A, H, E>
where
    A: Send + Sync + Clone + 'static,
    H: WriteHandler<A, E> + Copy,
    E: Eventable,
{
}

impl<A, H, E> Clone for WriteHandlerWrapper<A, H, E>
where
    A: Send + Sync + Clone + 'static,
    H: WriteHandler<A, E> + Copy,
    E: Eventable,
{
    fn clone(&self) -> Self {
        *self
    }
}

impl<A, H, E> WriteHandlerFn<A> for WriteHandlerWrapper<A, H, E>
where
    A: Send + Sync + Clone + 'static,
    H: WriteHandler<A, E> + Copy,
    E: Eventable,
{
    fn write(&self, event: Arc<dyn Eventable>, app: &mut A) -> Option<Events> {
        event
            .downcast_ref::<E>()
            .map(|event| self.handler.write(event, app))
            .unwrap()
    }
}

#[async_trait]
pub trait SideEffectHandler<A, E>: Send + Sync + DynClone
where
    A: Send + Sync + Clone + ToSnapshot + 'static,
{
    async fn side_effect(&self, event: E, snapshot: <A as ToSnapshot>::Snapshot) -> Option<Events>;
}
dyn_clone::clone_trait_object!(<A, E> SideEffectHandler<A, E>);

#[async_trait]
impl<A, T, F, Fut> SideEffectHandler<A, T> for F
where
    A: Send + Sync + Clone + ToSnapshot + 'static,
    T: Eventable,
    F: Fn(T, <A as ToSnapshot>::Snapshot) -> Fut + Send + Sync + Clone,
    Fut: std::future::Future<Output = Option<Events>> + Send,
{
    async fn side_effect(&self, event: T, snap: <A as ToSnapshot>::Snapshot) -> Option<Events> {
        (self)(event, snap).await
    }
}

#[async_trait]
pub trait SideEffectHandlerFn<A>: Send + Sync + DynClone
where
    A: Send + Sync + Clone + ToSnapshot + 'static,
{
    async fn side_effect(
        &self,
        event: Arc<dyn Eventable>,
        snap: <A as ToSnapshot>::Snapshot,
    ) -> Option<Events>;
}
dyn_clone::clone_trait_object!(<A> SideEffectHandlerFn<A>);

impl<A> fmt::Debug for dyn SideEffectHandlerFn<A> + Send + Sync {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("<SideEffectHandlerFn>").finish()
    }
}

pub(crate) struct SideEffectHandlerWrapper<A, H, E>
where
    A: Send + Sync + Clone + ToSnapshot + 'static,
    H: SideEffectHandler<A, E> + Copy,
{
    pub handler: H,
    pub phantom: PhantomData<(A, E)>,
}

impl<A, H, E> Copy for SideEffectHandlerWrapper<A, H, E>
where
    A: Send + Sync + Clone + ToSnapshot + 'static,
    H: SideEffectHandler<A, E> + Copy,
    E: Eventable,
{
}

impl<A, H, E> Clone for SideEffectHandlerWrapper<A, H, E>
where
    A: Send + Sync + Clone + ToSnapshot + 'static,
    H: SideEffectHandler<A, E> + Copy,
    E: Eventable,
{
    fn clone(&self) -> Self {
        *self
    }
}

#[async_trait]
impl<A, H, E> SideEffectHandlerFn<A> for SideEffectHandlerWrapper<A, H, E>
where
    A: Send + Sync + Clone + ToSnapshot + 'static,
    H: SideEffectHandler<A, E> + Copy + Send + Sync,
    E: Eventable + Clone,
{
    async fn side_effect(
        &self,
        event: Arc<dyn Eventable>,
        snapshot: <A as ToSnapshot>::Snapshot,
    ) -> Option<Events> {
        let evt = event.downcast_ref::<E>().unwrap();
        self.handler.side_effect(evt.clone(), snapshot).await
    }
}
