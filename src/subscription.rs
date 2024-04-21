use std::{
    any::TypeId,
    fmt,
    hash::{Hash, Hasher},
    marker::PhantomData,
    sync::Arc,
};

use crate::{eve::Eve, reactive::Sub, BoxableValue};
use dyn_clone::DynClone;
use rustc_hash::{FxHashSet as HashSet, FxHasher};

#[derive(Clone)]
pub struct SubscriptionContext<'a, A, S>
where
    A: Send + Sync + Clone + 'static,
    S: Subscription<A>,
{
    eve: Eve<A>,
    sub: &'a S,
}

impl<'a, A, S> SubscriptionContext<'a, A, S>
where
    A: Send + Sync + Clone + 'static,
    S: Subscription<A>,
{
    pub(crate) fn new(sub: &'a S, eve: Eve<A>) -> Self {
        SubscriptionContext { eve, sub }
    }
    pub fn subscribe<O: Subscription<A> + 'static>(&self, sub: O) -> Sub<A, O> {
        self.eve.reg_subscribe(self.sub.id(), sub)
    }
}

pub trait Subscription<A>: fmt::Debug + Hash + Send + Sync + Clone
where
    Self: 'static,
    A: Send + Sync + Clone + 'static,
{
    type Output: BoxableValue + PartialEq + Clone;

    fn id(&self) -> u64 {
        let mut hasher = FxHasher::default();
        TypeId::of::<Self>().hash(&mut hasher);
        self.hash(&mut hasher);
        hasher.finish()
    }

    fn handle(&self, app: &A, ctx: SubscriptionContext<A, Self>) -> Self::Output;
}
