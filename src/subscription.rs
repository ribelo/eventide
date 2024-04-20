use std::{
    fmt,
    hash::{Hash, Hasher},
    marker::PhantomData,
    sync::Arc,
};

use crate::{eve::Eve, BoxableValue};
use dyn_clone::DynClone;
use rustc_hash::{FxHashSet as HashSet, FxHasher};

#[derive(Clone)]
pub struct SubscriptionContext<A, S>
where
    A: Send + Sync + Clone + 'static,
    S: Subscription,
{
    eve: Eve<A>,
    sub: Arc<S>,
}

impl<A, S> SubscriptionContext<A, S>
where
    A: Send + Sync + Clone + 'static,
    S: Subscription,
{
    pub(crate) fn new(sub: Arc<S>, eve: Eve<A>) -> Self {
        SubscriptionContext { eve, sub }
    }
    pub fn subscribe<O: Subscription>(&self, sub: &O) {
        self.eve.subscribe(self.sub.id(), sub);
    }
}

pub trait FromContext<A, S>: Send + Sync + 'static
where
    A: Send + Sync + Clone + 'static,
    S: Subscription,
{
    fn from_context(ctx: &SubscriptionContext<A, S>) -> Self;
    fn collect_dependencies(_deps: &mut HashSet<u64>) {}
}

pub trait Subscription: fmt::Debug + Hash + Send + Sync + Clone {
    type Output: BoxableValue + PartialEq;

    fn id(&self) -> u64 {
        let mut hasher = FxHasher::default();
        self.hash(&mut hasher);
        hasher.finish()
    }
}

pub trait SubscriptionHandler<A, S, T>: Send + Sync + DynClone
where
    A: Send + Sync + Clone + 'static,
    S: Subscription,
{
    fn call(&self, context: SubscriptionContext<A, S>) -> S::Output;
    fn collect_dependencies(&self, deps: &mut HashSet<u64>);
}
dyn_clone::clone_trait_object!(<A, T, R> SubscriptionHandler<A, T, R>);

impl<A, S, F> SubscriptionHandler<A, S, ()> for F
where
    A: Send + Sync + Clone + 'static,
    S: Subscription,
    F: Fn(&S, &A, &SubscriptionContext<A, S>) -> S::Output + Send + Sync + Clone,
{
    fn call(&self, ctx: SubscriptionContext<A, S>) -> S::Output {
        (self)(&ctx.sub, &ctx.eve.app.read(), &ctx)
    }

    fn collect_dependencies(&self, _deps: &mut HashSet<u64>) {}
}

macro_rules! impl_handler {
    ($m:ident, ($($t:ident),*), $f:ident) => {
        $m!($($t),*; $f);
    };
}

macro_rules! subscription_handler_tuple_impls {
    ($($t:ident),*; $f:ident) => {
        impl<A, S, $($t),*, $f> SubscriptionHandler<A, S, ($($t,)*)> for $f
        where
            A: Send + Sync + Clone + 'static,
            S: Subscription,
            $f: Fn(&S, &A, &SubscriptionContext<A, S>, $($t),*) -> S::Output + Send + Sync + Clone,
            $($t: FromContext<A, S>,)*
        {
            fn call(&self, ctx: SubscriptionContext<A, S>) -> S::Output {
                (self)(&ctx.sub, &ctx.eve.app.read(), &ctx, $(<$t>::from_context(&ctx),)*)
            }
            fn collect_dependencies(&self, deps: &mut HashSet<u64>) {
                $(
                    <$t as FromContext<A, S>>::collect_dependencies(deps);
                )*
            }
        }
    }
}

impl_handler!(subscription_handler_tuple_impls, (T1), F);
impl_handler!(subscription_handler_tuple_impls, (T1, T2), F);
impl_handler!(subscription_handler_tuple_impls, (T1, T2, T3), F);
impl_handler!(subscription_handler_tuple_impls, (T1, T2, T3, T4), F);
impl_handler!(subscription_handler_tuple_impls, (T1, T2, T3, T4, T5), F);
impl_handler!(
    subscription_handler_tuple_impls,
    (T1, T2, T3, T4, T5, T6),
    F
);
impl_handler!(
    subscription_handler_tuple_impls,
    (T1, T2, T3, T4, T5, T6, T7),
    F
);
impl_handler!(
    subscription_handler_tuple_impls,
    (T1, T2, T3, T4, T5, T6, T7, T8),
    F
);
impl_handler!(
    subscription_handler_tuple_impls,
    (T1, T2, T3, T4, T5, T6, T7, T8, T9),
    F
);
impl_handler!(
    subscription_handler_tuple_impls,
    (T1, T2, T3, T4, T5, T6, T7, T8, T9, T10),
    F
);
impl_handler!(
    subscription_handler_tuple_impls,
    (T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11),
    F
);
impl_handler!(
    subscription_handler_tuple_impls,
    (T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12),
    F
);

pub trait SubscriptionHandlerFn<A, S>: Send + Sync + DynClone
where
    A: Send + Sync + Clone + 'static,
    S: Subscription,
{
    fn call(&self, context: SubscriptionContext<A, S>) -> S::Output;
    fn collect_dependencies(&self, deps: &mut HashSet<u64>);
}
dyn_clone::clone_trait_object!(<A, S> SubscriptionHandlerFn<A, S>);

impl<A, S> fmt::Debug for dyn SubscriptionHandlerFn<A, S>
where
    S: Subscription,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("").finish()
    }
}

pub(crate) struct SubscriptionHandlerWrapper<A, S, H, T>
where
    A: Send + Sync + Clone + 'static,
    S: Subscription,
    H: SubscriptionHandler<A, S, T> + Copy,
{
    pub handler: H,
    pub phantom: PhantomData<(A, S, T)>,
}

impl<A, S, H, T> Copy for SubscriptionHandlerWrapper<A, S, H, T>
where
    A: Send + Sync + Clone + 'static,
    S: Subscription,
    H: SubscriptionHandler<A, S, T> + Copy,
    T: Send + Sync,
{
}

impl<A, S, H, T> Clone for SubscriptionHandlerWrapper<A, S, H, T>
where
    A: Send + Sync + Clone + 'static,
    S: Subscription,
    H: SubscriptionHandler<A, S, T> + Copy,
    T: Send + Sync,
{
    fn clone(&self) -> Self {
        *self
    }
}

impl<A, S, H, T> SubscriptionHandlerFn<A, S> for SubscriptionHandlerWrapper<A, S, H, T>
where
    A: Send + Sync + Clone + 'static,
    S: Subscription,
    H: SubscriptionHandler<A, S, T> + Copy,
    T: Send + Sync,
{
    fn call(&self, ctx: SubscriptionContext<A, S>) -> S::Output {
        self.handler.call(ctx)
    }
    fn collect_dependencies(&self, deps: &mut HashSet<u64>) {
        self.handler.collect_dependencies(deps);
    }
}
