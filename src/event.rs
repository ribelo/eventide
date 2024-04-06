use async_trait::async_trait;
use downcast_rs::{impl_downcast, DowncastSync};
use dyn_clone::DynClone;
use std::{fmt, marker::PhantomData, sync::Arc};

use crate::{eve::ToSnapshot, id::Id};

#[doc(hidden)]
pub trait Eventable: fmt::Debug + DynClone + DowncastSync {}
impl_downcast!(sync Eventable);
dyn_clone::clone_trait_object!(Eventable);

#[derive(Debug, Clone)]
pub struct Effect {
    pub id: Id,
    pub inner: Arc<dyn Eventable>,
}

#[derive(Debug, Clone)]
pub struct SideEffect {
    pub id: Id,
    pub inner: Arc<dyn Eventable>,
}

#[derive(Debug, Clone)]
pub struct EffectError {
    pub id: Id,
    pub inner: Arc<dyn std::error::Error + Send + Sync>,
}

impl<T: std::error::Error + Send + Sync + 'static> From<T> for EffectError {
    fn from(error: T) -> EffectError {
        EffectError {
            id: Id::new::<T>(),
            inner: Arc::new(error),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Event {
    Effect(Effect),
    SideEffect(SideEffect),
    Error(EffectError),
}

impl From<Effect> for Event {
    fn from(effect: Effect) -> Event {
        Event::Effect(effect)
    }
}

impl From<SideEffect> for Event {
    fn from(side_effect: SideEffect) -> Event {
        Event::SideEffect(side_effect)
    }
}

impl From<EffectError> for Event {
    fn from(error: EffectError) -> Event {
        Event::Error(error)
    }
}

impl<E: Eventable> From<E> for Effect {
    fn from(event: E) -> Effect {
        Effect {
            id: Id::new::<E>(),
            inner: Arc::new(event),
        }
    }
}

impl<E: Eventable> From<E> for SideEffect {
    fn from(event: E) -> SideEffect {
        SideEffect {
            id: Id::new::<E>(),
            inner: Arc::new(event),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Events {
    pub effects: Vec<Effect>,
    pub side_effects: Vec<SideEffect>,
}

impl Events {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    pub fn push_effect<T: Into<Effect>>(&mut self, effect: T) {
        self.effects.push(effect.into());
    }
    pub fn push_side_effect<T: Into<SideEffect>>(&mut self, side_effect: T) {
        self.side_effects.push(side_effect.into());
    }
    #[must_use]
    pub fn with_effect<T: Into<Effect>>(mut self, event: T) -> Self {
        self.effects.push(event.into());
        self
    }
    #[must_use]
    pub fn with_side_effect<T: Into<SideEffect>>(mut self, side_effect: T) -> Self {
        self.side_effects.push(side_effect.into());
        self
    }
}

pub type EffectResult = Result<Option<Events>, EffectError>;
pub type EventResult<E> = Result<Option<Events>, E>;

pub trait ReadHandler<S, T, E>: DynClone
where
    S: Send + Sync + Clone + ToSnapshot + 'static,
    E: std::error::Error + Send + Sync + 'static,
{
    fn read(&self, event: &T, eve: <S as ToSnapshot>::Snapshot) -> EventResult<E>;
}
dyn_clone::clone_trait_object!(<S, T, E> ReadHandler<S, T, E>);

impl<S, T, F, E> ReadHandler<S, T, E> for F
where
    S: Send + Sync + Clone + ToSnapshot + 'static,
    F: Fn(&T, <S as ToSnapshot>::Snapshot) -> EventResult<E> + Clone,
    T: Eventable + 'static,
    E: std::error::Error + Send + Sync + 'static,
{
    fn read(&self, event: &T, snapshot: <S as ToSnapshot>::Snapshot) -> EventResult<E> {
        (self)(event, snapshot)
    }
}

pub trait ReadHandlerFn<S>: DynClone
where
    S: Send + Sync + Clone + ToSnapshot + 'static,
{
    fn read(
        &self,
        event: Arc<dyn Eventable>,
        state: <S as ToSnapshot>::Snapshot,
    ) -> Result<Option<Events>, Box<dyn std::error::Error + Send + Sync + 'static>>;
}
dyn_clone::clone_trait_object!(<S> ReadHandlerFn<S>);

impl<S> fmt::Debug for dyn ReadHandlerFn<S> + Send + Sync {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("<ReadHandlerFn>").finish()
    }
}

pub(crate) struct ReadHandlerWrapper<S, H, T, E>
where
    S: Send + Sync + Clone + ToSnapshot + 'static,
    H: ReadHandler<S, T, E> + Copy,
    E: std::error::Error + Send + Sync + Clone + 'static,
{
    pub handler: H,
    pub phantom: PhantomData<(S, T, E)>,
}

impl<S, H, T, E> Copy for ReadHandlerWrapper<S, H, T, E>
where
    S: Send + Sync + Clone + ToSnapshot + 'static,
    H: ReadHandler<S, T, E> + Copy,
    T: Eventable,
    E: std::error::Error + Send + Sync + Clone + 'static,
{
}

impl<S, H, T, E> Clone for ReadHandlerWrapper<S, H, T, E>
where
    S: Send + Sync + Clone + ToSnapshot + 'static,
    H: ReadHandler<S, T, E> + Copy,
    T: Eventable,
    E: std::error::Error + Send + Sync + Clone + 'static,
{
    fn clone(&self) -> Self {
        *self
    }
}

impl<S, H, T, E> ReadHandlerFn<S> for ReadHandlerWrapper<S, H, T, E>
where
    S: Send + Sync + Clone + ToSnapshot + 'static,
    H: ReadHandler<S, T, E> + Copy,
    T: Eventable,
    E: std::error::Error + Send + Sync + Clone + 'static,
{
    fn read(
        &self,
        event: Arc<dyn Eventable>,
        state: <S as ToSnapshot>::Snapshot,
    ) -> Result<Option<Events>, Box<dyn std::error::Error + Send + Sync + 'static>> {
        Ok(event
            .downcast_ref::<T>()
            .map(|event| self.handler.read(event, state))
            .unwrap()?)
    }
}

pub trait WriteHandler<S, T>: DynClone
where
    S: Send + Sync + Clone + 'static,
{
    fn write(&self, event: &T, state: &mut S) -> EffectResult;
}
dyn_clone::clone_trait_object!(<S, E> WriteHandler<S, E>);

impl<S, T, F> WriteHandler<S, T> for F
where
    S: Send + Sync + Clone + 'static,
    F: Fn(&T, &mut S) -> EffectResult + Clone,
    T: Eventable + 'static,
{
    fn write(&self, event: &T, state: &mut S) -> EffectResult {
        (self)(event, state)
    }
}

pub trait WriteHandlerFn<S>: DynClone
where
    S: Send + Sync + Clone + 'static,
{
    fn write(&self, event: Arc<dyn Eventable>, state: &mut S) -> EffectResult;
}
dyn_clone::clone_trait_object!(<S> WriteHandlerFn<S>);

impl<S> fmt::Debug for dyn WriteHandlerFn<S> + Send + Sync {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("<WriteHandlerFn>").finish()
    }
}

pub(crate) struct WriteHandlerWrapper<S, H, E>
where
    S: Send + Sync + Clone + 'static,
    H: WriteHandler<S, E> + Copy,
{
    pub handler: H,
    pub phantom: PhantomData<(S, E)>,
}

impl<S, H, E> Copy for WriteHandlerWrapper<S, H, E>
where
    S: Send + Sync + Clone + 'static,
    H: WriteHandler<S, E> + Copy,
    E: Eventable,
{
}

impl<S, H, E> Clone for WriteHandlerWrapper<S, H, E>
where
    S: Send + Sync + Clone + 'static,
    H: WriteHandler<S, E> + Copy,
    E: Eventable,
{
    fn clone(&self) -> Self {
        *self
    }
}

impl<S, H, E> WriteHandlerFn<S> for WriteHandlerWrapper<S, H, E>
where
    S: Send + Sync + Clone + 'static,
    H: WriteHandler<S, E> + Copy,
    E: Eventable,
{
    fn write(&self, event: Arc<dyn Eventable>, state: &mut S) -> EffectResult {
        event
            .downcast_ref::<E>()
            .map(|event| self.handler.write(event, state))
            .unwrap()
    }
}

#[async_trait]
pub trait SideEffectHandler<S, T>: DynClone
where
    S: Send + Sync + Clone + ToSnapshot + 'static,
{
    async fn side_effect(&self, event: T, snapshot: <S as ToSnapshot>::Snapshot) -> EffectResult;
}
dyn_clone::clone_trait_object!(<S, E> SideEffectHandler<S, E>);

#[async_trait]
impl<S, T, F, Fut> SideEffectHandler<S, T> for F
where
    S: Send + Sync + Clone + ToSnapshot + 'static,
    T: Eventable + 'static,
    F: Fn(T, <S as ToSnapshot>::Snapshot) -> Fut + Send + Sync + Clone,
    Fut: std::future::Future<Output = EffectResult> + Send,
{
    async fn side_effect(&self, event: T, state: <S as ToSnapshot>::Snapshot) -> EffectResult {
        (self)(event, state).await
    }
}

#[async_trait]
pub trait SideEffectHandlerFn<S>: DynClone
where
    S: Send + Sync + Clone + ToSnapshot + 'static,
{
    async fn side_effect(
        &self,
        event: Arc<dyn Eventable>,
        state: <S as ToSnapshot>::Snapshot,
    ) -> EffectResult;
}
dyn_clone::clone_trait_object!(<S> SideEffectHandlerFn<S>);

impl<S> fmt::Debug for dyn SideEffectHandlerFn<S> + Send + Sync {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("<SideEffectHandlerFn>").finish()
    }
}

pub(crate) struct SideEffectHandlerWrapper<S, H, T>
where
    S: Send + Sync + Clone + ToSnapshot + 'static,
    H: SideEffectHandler<S, T> + Copy,
{
    pub handler: H,
    pub phantom: PhantomData<(S, T)>,
}

impl<S, H, T> Copy for SideEffectHandlerWrapper<S, H, T>
where
    S: Send + Sync + Clone + ToSnapshot + 'static,
    H: SideEffectHandler<S, T> + Copy,
    T: Eventable,
{
}

impl<S, H, T> Clone for SideEffectHandlerWrapper<S, H, T>
where
    S: Send + Sync + Clone + ToSnapshot + 'static,
    H: SideEffectHandler<S, T> + Copy,
    T: Eventable,
{
    fn clone(&self) -> Self {
        *self
    }
}

#[async_trait]
impl<S, H, T> SideEffectHandlerFn<S> for SideEffectHandlerWrapper<S, H, T>
where
    S: Send + Sync + Clone + ToSnapshot + 'static,
    H: SideEffectHandler<S, T> + Copy + Send + Sync,
    T: Eventable + Clone,
{
    async fn side_effect(
        &self,
        event: Arc<dyn Eventable>,
        snapshot: <S as ToSnapshot>::Snapshot,
    ) -> EffectResult {
        let evt = event.downcast_ref::<T>().unwrap();
        self.handler.side_effect(evt.clone(), snapshot).await
    }
}

pub trait ErrorHandler<S, E>: DynClone
where
    S: Send + Sync + Clone + ToSnapshot + 'static,
{
    fn handle(&self, event: &E, eve: <S as ToSnapshot>::Snapshot) -> EffectResult;
}
dyn_clone::clone_trait_object!(<S, T> ErrorHandler<S, T>);

impl<S, E, F> ErrorHandler<S, E> for F
where
    S: Send + Sync + Clone + ToSnapshot + 'static,
    F: Fn(&E, <S as ToSnapshot>::Snapshot) -> EffectResult + Clone,
    E: std::error::Error + Send + Sync,
{
    fn handle(&self, event: &E, snapshot: <S as ToSnapshot>::Snapshot) -> EffectResult {
        (self)(event, snapshot)
    }
}

pub trait ErrorHandlerFn<S>: DynClone
where
    S: Send + Sync + Clone + ToSnapshot + 'static,
{
    fn handle_error(
        &self,
        error: Arc<dyn std::error::Error + Send + Sync>,
        state: <S as ToSnapshot>::Snapshot,
    ) -> EffectResult;
}
dyn_clone::clone_trait_object!(<S> ErrorHandlerFn<S>);

impl<S> fmt::Debug for dyn ErrorHandlerFn<S> + Send + Sync {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("<ErrorHandlerFn>").finish()
    }
}

pub(crate) struct ErrorHandlerWrapper<S, H, E>
where
    S: Send + Sync + Clone + ToSnapshot + 'static,
    H: ErrorHandler<S, E> + Copy,
{
    pub handler: H,
    pub phantom: PhantomData<(S, E)>,
}

impl<S, H, E> Copy for ErrorHandlerWrapper<S, H, E>
where
    S: Send + Sync + Clone + ToSnapshot + 'static,
    H: ErrorHandler<S, E> + Copy,
    E: std::error::Error + Send + Sync,
{
}

impl<S, H, E> Clone for ErrorHandlerWrapper<S, H, E>
where
    S: Send + Sync + Clone + ToSnapshot + 'static,
    H: ErrorHandler<S, E> + Copy,
    E: std::error::Error + Send + Sync,
{
    fn clone(&self) -> Self {
        *self
    }
}

impl<S, H, E> ErrorHandlerFn<S> for ErrorHandlerWrapper<S, H, E>
where
    S: Send + Sync + Clone + ToSnapshot + 'static,
    H: ErrorHandler<S, E> + Copy,
    E: std::error::Error + Send + Sync + 'static,
{
    fn handle_error(
        &self,
        error: Arc<dyn std::error::Error + Send + Sync>,
        state: <S as ToSnapshot>::Snapshot,
    ) -> EffectResult {
        error
            .downcast_ref::<E>()
            .map(|event| self.handler.handle(event, state))
            .unwrap()
    }
}
