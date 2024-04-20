use async_trait::async_trait;
use downcast_rs::{impl_downcast, DowncastSync};
use dyn_clone::DynClone;
use std::{any::TypeId, fmt, future::Future, marker::PhantomData, sync::Arc};

use crate::eve::Eve;

pub trait Eventable: fmt::Debug + Send + Sync + DowncastSync + DynClone + 'static {}
impl_downcast!(sync Eventable);
dyn_clone::clone_trait_object!(Eventable);
impl<T> Eventable for T where T: fmt::Debug + Send + Sync + DynClone + 'static {}

#[derive(Debug, Clone)]
pub struct SystemEvent {
    pub id: TypeId,
    pub data: Arc<dyn Eventable>,
}

impl SystemEvent {
    pub fn new<T>(event: T) -> Self
    where
        T: Eventable,
    {
        SystemEvent {
            id: TypeId::of::<T>(),
            data: Arc::new(event),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SideEffect {
    pub id: TypeId,
    pub data: Arc<dyn Eventable>,
}

impl SideEffect {
    pub fn new<T>(event: T) -> Self
    where
        T: Eventable,
    {
        SideEffect {
            id: TypeId::of::<T>(),
            data: Arc::new(event),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Event {
    System(SystemEvent),
    SideEffect(SideEffect),
}

impl Event {
    pub fn system<T>(event: T) -> Self
    where
        T: Eventable,
    {
        Event::System(SystemEvent::new(event))
    }
    pub fn side_effect<T>(event: T) -> Self
    where
        T: Eventable,
    {
        Event::SideEffect(SideEffect::new(event))
    }
}

impl From<SystemEvent> for Event {
    fn from(event: SystemEvent) -> Self {
        Event::System(event)
    }
}

impl From<SideEffect> for Event {
    fn from(event: SideEffect) -> Self {
        Event::SideEffect(event)
    }
}

pub struct Events(pub Vec<Event>);

impl From<Vec<Event>> for Events {
    fn from(events: Vec<Event>) -> Self {
        Events(events)
    }
}

impl From<Event> for Events {
    fn from(event: Event) -> Self {
        Events(vec![event])
    }
}

impl From<SystemEvent> for Events {
    fn from(event: SystemEvent) -> Self {
        Events(vec![Event::System(event)])
    }
}

impl From<SideEffect> for Events {
    fn from(event: SideEffect) -> Self {
        Events(vec![Event::SideEffect(event)])
    }
}

pub trait FromEve<A>: Send + Sync
where
    A: Send + Sync + Clone + 'static,
{
    fn from_eve(eve: Eve<A>) -> Self;
}

pub trait ReadHandler<A, E, T>: Send + Sync + DynClone
where
    A: Send + Sync + Clone + 'static,
{
    fn read(&self, event: &E, eve: Eve<A>) -> Option<Events>;
}
dyn_clone::clone_trait_object!(<A, E, T> ReadHandler<A, E, T>);

macro_rules! impl_handler {
    ($m:ident, ($($t:ident),*), $f:ident) => {
        $m!($($t),*; $f);
    };
}

macro_rules! read_handler_tuple_impls {
    ($($t:ident),*; $f:ident) => {
        impl<A, E, $($t),*, $f> ReadHandler<A, E, ($($t,)*)> for $f
        where
            A: Send + Sync + Clone + 'static,
            E: Eventable,
            $f: Fn(&E, &A, $($t),*) -> Option<Events> + Send + Sync + Clone,
            $($t: FromEve<A>,)*
        {
            fn read(&self, event: &E, eve: Eve<A>) -> Option<Events> {
                (self)(event, &eve.app.read(), $(<$t>::from_eve(eve.clone()),)*)
            }
        }
    }
}

impl_handler!(read_handler_tuple_impls, (T1), F);
impl_handler!(read_handler_tuple_impls, (T1, T2), F);
impl_handler!(read_handler_tuple_impls, (T1, T2, T3), F);
impl_handler!(read_handler_tuple_impls, (T1, T2, T3, T4), F);
impl_handler!(read_handler_tuple_impls, (T1, T2, T3, T4, T5), F);
impl_handler!(read_handler_tuple_impls, (T1, T2, T3, T4, T5, T6), F);
impl_handler!(read_handler_tuple_impls, (T1, T2, T3, T4, T5, T6, T7), F);
impl_handler!(
    read_handler_tuple_impls,
    (T1, T2, T3, T4, T5, T6, T7, T8),
    F
);
impl_handler!(
    read_handler_tuple_impls,
    (T1, T2, T3, T4, T5, T6, T7, T8, T9),
    F
);
impl_handler!(
    read_handler_tuple_impls,
    (T1, T2, T3, T4, T5, T6, T7, T8, T9, T10),
    F
);
impl_handler!(
    read_handler_tuple_impls,
    (T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11),
    F
);
impl_handler!(
    read_handler_tuple_impls,
    (T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12),
    F
);

pub trait ReadHandlerFn<A>: Send + Sync + DynClone
where
    A: Send + Sync + Clone + 'static,
{
    fn read(&self, event: Arc<dyn Eventable>, eve: Eve<A>) -> Option<Events>;
}
dyn_clone::clone_trait_object!(<A> ReadHandlerFn<A>);

impl<A> fmt::Debug for dyn ReadHandlerFn<A> + Send + Sync {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("<ReadHandlerFn>").finish()
    }
}

pub(crate) struct ReadHandlerWrapper<A, H, E, T>
where
    A: Send + Sync + Clone + 'static,
    H: ReadHandler<A, E, T> + Copy,
{
    pub handler: H,
    pub phantom: PhantomData<(A, E, T)>,
}

impl<A, H, E, T> Copy for ReadHandlerWrapper<A, H, E, T>
where
    A: Send + Sync + Clone + 'static,
    H: ReadHandler<A, E, T> + Copy,
    E: Eventable,
{
}

impl<A, H, E, T> Clone for ReadHandlerWrapper<A, H, E, T>
where
    A: Send + Sync + Clone + 'static,
    H: ReadHandler<A, E, T> + Copy,
    E: Eventable,
{
    fn clone(&self) -> Self {
        *self
    }
}

impl<A, H, E, T> ReadHandlerFn<A> for ReadHandlerWrapper<A, H, E, T>
where
    A: Send + Sync + Clone + 'static,
    H: ReadHandler<A, E, T> + Copy,
    E: Eventable,
    T: Send + Sync,
{
    fn read(&self, event: Arc<dyn Eventable>, eve: Eve<A>) -> Option<Events> {
        event
            .downcast_ref::<E>()
            .map(|event| self.handler.read(event, eve))
            .unwrap()
    }
}

pub trait WriteHandler<A, E, T>: Send + Sync + DynClone
where
    A: Send + Sync + Clone + 'static,
{
    fn write(&self, event: &E, eve: Eve<A>) -> Option<Events>;
}
dyn_clone::clone_trait_object!(<A, E, T> WriteHandler<A, E, T>);

macro_rules! write_handler_tuple_impls {
    ($($t:ident),*; $f:ident) => {
        impl<A, E, $($t),*, $f> WriteHandler<A, E, ($($t,)*)> for $f
        where
            A: Send + Sync + Clone + 'static,
            E: Eventable,
            $f: Fn(&E, &mut A, $($t),*) -> Option<Events> + Send + Sync + Clone,
            $($t: FromEve<A>,)*
        {
            fn write(&self, event: &E, eve: Eve<A>) -> Option<Events> {
                let eve_clone = eve.clone();
                (self)(event, &mut eve.app.write(), $(<$t>::from_eve(eve_clone.clone()),)*)
            }
        }
    }
}

impl_handler!(write_handler_tuple_impls, (T1), F);
impl_handler!(write_handler_tuple_impls, (T1, T2), F);
impl_handler!(write_handler_tuple_impls, (T1, T2, T3), F);
impl_handler!(write_handler_tuple_impls, (T1, T2, T3, T4), F);
impl_handler!(write_handler_tuple_impls, (T1, T2, T3, T4, T5), F);
impl_handler!(write_handler_tuple_impls, (T1, T2, T3, T4, T5, T6), F);
impl_handler!(write_handler_tuple_impls, (T1, T2, T3, T4, T5, T6, T7), F);
impl_handler!(
    write_handler_tuple_impls,
    (T1, T2, T3, T4, T5, T6, T7, T8),
    F
);
impl_handler!(
    write_handler_tuple_impls,
    (T1, T2, T3, T4, T5, T6, T7, T8, T9),
    F
);
impl_handler!(
    write_handler_tuple_impls,
    (T1, T2, T3, T4, T5, T6, T7, T8, T9, T10),
    F
);
impl_handler!(
    write_handler_tuple_impls,
    (T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11),
    F
);
impl_handler!(
    write_handler_tuple_impls,
    (T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12),
    F
);

pub trait WriteHandlerFn<A>: Send + Sync + DynClone
where
    A: Send + Sync + Clone + 'static,
{
    fn write(&self, event: Arc<dyn Eventable>, eve: Eve<A>) -> Option<Events>;
}
dyn_clone::clone_trait_object!(<A> WriteHandlerFn<A>);

impl<A> fmt::Debug for dyn WriteHandlerFn<A> + Send + Sync {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("<WriteHandlerFn>").finish()
    }
}

pub(crate) struct WriteHandlerWrapper<A, H, E, T>
where
    A: Send + Sync + Clone + 'static,
    H: WriteHandler<A, E, T> + Copy,
{
    pub handler: H,
    pub phantom: PhantomData<(A, E, T)>,
}

impl<A, H, E, T> Copy for WriteHandlerWrapper<A, H, E, T>
where
    A: Send + Sync + Clone + 'static,
    H: WriteHandler<A, E, T> + Copy,
    E: Eventable,
{
}

impl<A, H, E, T> Clone for WriteHandlerWrapper<A, H, E, T>
where
    A: Send + Sync + Clone + 'static,
    H: WriteHandler<A, E, T> + Copy,
    E: Eventable,
{
    fn clone(&self) -> Self {
        *self
    }
}

impl<A, H, E, T> WriteHandlerFn<A> for WriteHandlerWrapper<A, H, E, T>
where
    A: Send + Sync + Clone + 'static,
    H: WriteHandler<A, E, T> + Copy,
    E: Eventable,
    T: Send + Sync,
{
    fn write(&self, event: Arc<dyn Eventable>, eve: Eve<A>) -> Option<Events> {
        event
            .downcast_ref::<E>()
            .map(|event| self.handler.write(event, eve))
            .unwrap()
    }
}

#[async_trait]
pub trait SideEffectHandler<A, E, T>: Send + Sync + DynClone
where
    A: Send + Sync + Clone + 'static,
{
    async fn side_effect(&self, event: &E, eve: Eve<A>) -> Option<Events>;
}
dyn_clone::clone_trait_object!(<A, E, T> SideEffectHandler<A, E, T>);

macro_rules! side_effect_handler_tuple_impls {
    ($($t:ident),*; $f:ident) => {
        #[async_trait]
        impl<A, E, Fut, $($t),*, $f> SideEffectHandler<A, E, ($($t,)*)> for $f
        where
            A: Send + Sync + Clone + 'static,
            E: Eventable,
            Fut: Future<Output = Option<Events>> + Send,
            $f: Fn(&E, $($t),*) -> Fut + Send + Sync + Clone,
            $($t: FromEve<A>,)*
        {
            async fn side_effect(&self, event: &E, eve: Eve<A>) -> Option<Events> {
                (self)(event, $(<$t>::from_eve(eve.clone()),)*).await
            }
        }
    }
}

impl_handler!(side_effect_handler_tuple_impls, (T1), F);
impl_handler!(side_effect_handler_tuple_impls, (T1, T2), F);
impl_handler!(side_effect_handler_tuple_impls, (T1, T2, T3), F);
impl_handler!(side_effect_handler_tuple_impls, (T1, T2, T3, T4), F);
impl_handler!(side_effect_handler_tuple_impls, (T1, T2, T3, T4, T5), F);
impl_handler!(side_effect_handler_tuple_impls, (T1, T2, T3, T4, T5, T6), F);
impl_handler!(
    side_effect_handler_tuple_impls,
    (T1, T2, T3, T4, T5, T6, T7),
    F
);
impl_handler!(
    side_effect_handler_tuple_impls,
    (T1, T2, T3, T4, T5, T6, T7, T8),
    F
);
impl_handler!(
    side_effect_handler_tuple_impls,
    (T1, T2, T3, T4, T5, T6, T7, T8, T9),
    F
);
impl_handler!(
    side_effect_handler_tuple_impls,
    (T1, T2, T3, T4, T5, T6, T7, T8, T9, T10),
    F
);
impl_handler!(
    side_effect_handler_tuple_impls,
    (T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11),
    F
);
impl_handler!(
    side_effect_handler_tuple_impls,
    (T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12),
    F
);

#[async_trait]
pub trait SideEffectHandlerFn<A>: Send + Sync + DynClone
where
    A: Send + Sync + Clone + 'static,
{
    async fn side_effect(&self, event: Arc<dyn Eventable>, eve: Eve<A>) -> Option<Events>;
}
dyn_clone::clone_trait_object!(<A> SideEffectHandlerFn<A>);

impl<A> fmt::Debug for dyn SideEffectHandlerFn<A> + Send + Sync {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("<SideEffectHandlerFn>").finish()
    }
}

pub(crate) struct SideEffectHandlerWrapper<A, H, E, T>
where
    A: Send + Sync + Clone + 'static,
    H: SideEffectHandler<A, E, T> + Copy,
{
    pub handler: H,
    pub phantom: PhantomData<(A, E, T)>,
}

impl<A, H, E, T> Copy for SideEffectHandlerWrapper<A, H, E, T>
where
    A: Send + Sync + Clone + 'static,
    H: SideEffectHandler<A, E, T> + Copy,
    E: Eventable,
{
}

impl<A, H, E, T> Clone for SideEffectHandlerWrapper<A, H, E, T>
where
    A: Send + Sync + Clone + 'static,
    H: SideEffectHandler<A, E, T> + Copy,
    E: Eventable,
{
    fn clone(&self) -> Self {
        *self
    }
}

#[async_trait]
impl<A, H, E, T> SideEffectHandlerFn<A> for SideEffectHandlerWrapper<A, H, E, T>
where
    A: Send + Sync + Clone + 'static,
    H: SideEffectHandler<A, E, T> + Copy + Send + Sync,
    E: Eventable + Clone,
    T: Send + Sync,
{
    async fn side_effect(&self, event: Arc<dyn Eventable>, eve: Eve<A>) -> Option<Events> {
        let evt = event.downcast_ref::<E>().unwrap();
        self.handler.side_effect(&evt, eve).await
    }
}

mod tests {
    use super::*;

    #[derive(Debug, Clone)]
    struct TestEvent {
        value: i32,
    }

    #[test]
    fn test_system_event_new() {
        let event = TestEvent { value: 42 };
        let system_event = SystemEvent::new(event);

        assert_eq!(system_event.id, TypeId::of::<TestEvent>());

        let event_data = system_event.data.downcast_ref::<TestEvent>().unwrap();
        assert_eq!(event_data.value, 42);
    }

    #[test]
    fn test_side_effect_new() {
        let event = TestEvent { value: 123 };
        let side_effect = SideEffect::new(event);

        assert_eq!(side_effect.id, TypeId::of::<TestEvent>());

        let event_data = side_effect.data.downcast_ref::<TestEvent>().unwrap();
        assert_eq!(event_data.value, 123);
    }
}
