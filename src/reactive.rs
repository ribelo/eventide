use std::{
    any::TypeId,
    collections::BTreeMap,
    fmt,
    hash::{Hash, Hasher},
    marker::PhantomData,
    ops::Deref,
    sync::Arc,
};

use dyn_clone::DynClone;
use parking_lot::RwLock;
use rustc_hash::{FxHashSet as HashSet, FxHasher};

use crate::{
    eve::{Eve, EveBuilder},
    subscription::{Subscription, SubscriptionContext, SubscriptionHandler},
    BoxableValue,
};

pub trait Reactive<A>: DynClone + Send + Sync + 'static
where
    A: Send + Sync + Clone,
{
    fn run(&self, eve: Eve<A>, sub: Arc<dyn BoxableValue>);
}
dyn_clone::clone_trait_object!(<S> Reactive<S>);

impl<S> fmt::Debug for dyn Reactive<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("<Reactive>").finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeState {
    Clean,
    Dirty,
}

#[derive(Debug, Clone)]
pub enum NodeValue {
    Uninitialized,
    Memoized(Arc<dyn BoxableValue>),
}

impl NodeValue {
    pub fn new<T>(value: T) -> Self
    where
        T: BoxableValue,
    {
        Self::Memoized(Arc::new(value))
    }
    pub fn get<T>(&self) -> Option<Arc<T>>
    where
        T: BoxableValue,
    {
        match self {
            NodeValue::Uninitialized => None,
            NodeValue::Memoized(value) => Arc::clone(&value).downcast_arc::<T>().ok(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Memo<A, S>
where
    A: Send + Sync + Clone + 'static,
    S: Subscription,
{
    pub id: u64,
    phantom: PhantomData<S>,
    eve: Eve<A>,
}

impl<A, S> Memo<A, S>
where
    A: Send + Sync + Clone + 'static,
    S: Subscription,
{
    pub fn new(id: u64, eve: Eve<A>) -> Self {
        Self {
            id,
            phantom: PhantomData,
            eve,
        }
    }

    pub fn get(&self) -> Arc<S::Output> {
        self.eve.get_node_by_id::<S>(self.id).unwrap()
    }
}

pub struct MemoAnchor<A, S, H, T>
where
    A: Send + Sync + Clone + 'static,
    S: Subscription,
    H: SubscriptionHandler<A, S, T>,
{
    handler: H,
    phantom: PhantomData<(A, S, T)>,
}

impl<A, S, H, T> MemoAnchor<A, S, H, T>
where
    A: Send + Sync + Clone + 'static,
    S: Subscription,
    H: SubscriptionHandler<A, S, T> + Clone,
{
    pub fn new(handler: H) -> Self {
        Self {
            handler,
            phantom: PhantomData,
        }
    }
}

impl<A, S, H, T> Clone for MemoAnchor<A, S, H, T>
where
    A: Send + Sync + Clone + 'static,
    S: Subscription,
    H: SubscriptionHandler<A, S, T> + Clone,
{
    fn clone(&self) -> Self {
        Self {
            handler: self.handler.clone(),
            phantom: PhantomData,
        }
    }
}

impl<A, S, H, T> Reactive<A> for MemoAnchor<A, S, H, T>
where
    A: Send + Sync + Clone + 'static,
    S: Subscription + 'static,
    T: Send + Sync + 'static,
    H: SubscriptionHandler<A, S, T> + Clone + 'static,
{
    fn run(&self, eve: Eve<A>, sub: Arc<dyn BoxableValue>) {
        let sub = sub.downcast_arc::<S>().unwrap();
        let id = sub.id();
        let context = SubscriptionContext::new(sub, eve.clone());
        let new_value = self.handler.call(context);

        // Update the node value if it's different from the existing one
        if eve
            .get_raw_node::<S>(id)
            .and_then(|node_value| node_value.get::<S::Output>())
            .map_or(true, |old_value| new_value != *old_value)
        {
            eve.set_node_by_id::<S>(id, new_value);
        }

        // Mark node as clean in either case
        eve.set_node_state(id, NodeState::Clean);
    }
}

impl<A> EveBuilder<A>
where
    A: Send + Sync + Clone + 'static,
{
    pub fn reg_sub<S, H, T>(mut self, handler: H) -> Self
    where
        S: Subscription + 'static,
        H: SubscriptionHandler<A, S, T> + Clone + 'static,
        T: Send + Sync + 'static,
    {
        let mut sources = HashSet::default();
        handler.collect_dependencies(&mut sources);

        let anchor: MemoAnchor<A, S, H, T> = MemoAnchor::new(handler);

        self.reactive.insert(TypeId::of::<S>(), Arc::new(anchor));

        self
    }
}

impl<A> Eve<A>
where
    A: Send + Sync + Clone + 'static,
{
    fn get_node_state(&self, id: u64) -> Option<NodeState> {
        self.statuses.read().get(&id).copied()
    }

    fn set_node_state(&self, id: u64, state: NodeState) {
        self.statuses.write().insert(id, state);
    }

    #[must_use]
    pub fn get_subscription_by_id(&self, id: u64) -> Option<Arc<dyn BoxableValue>> {
        self.subscriptions.read().get(&id).cloned()
    }

    #[must_use]
    pub fn get_node<S: Subscription>(&self, props: &S) -> Option<Arc<S::Output>> {
        let id = props.id();
        self.update_node_if_necessary(id);
        self.get_node_by_id::<S>(id)
    }

    #[must_use]
    pub fn get_node_by_id<S: Subscription>(&self, id: u64) -> Option<Arc<S::Output>> {
        self.update_node_if_necessary(id);
        self.values.read().get(&id)?.get()
    }

    #[must_use]
    pub(crate) fn get_raw_node<S: Subscription>(&self, id: u64) -> Option<NodeValue> {
        self.values.read().get(&id).cloned()
    }

    pub fn set_node_by_id<S: Subscription>(&self, id: u64, value: S::Output) {
        let node_value = NodeValue::new(value);
        self.values.write().insert(id, node_value);
        self.mark_subs_as_dirty(id);
    }

    fn get_reactive_by_id(&self, id: u64) -> Option<Arc<dyn Reactive<A>>> {
        self.subscription_mapping
            .read()
            .get(&id)
            .and_then(|type_id| self.reactive.read().get(type_id).cloned())
    }

    fn get_node_sources(&self, id: u64) -> Option<HashSet<u64>> {
        self.sources.read().get(&id).cloned()
    }

    fn get_node_subscribers(&self, id: u64) -> Option<HashSet<u64>> {
        self.subscribers.read().get(&id).cloned()
    }

    pub(crate) fn subscribe<S: Subscription>(
        &self,
        self_id: u64,
        sub: &S,
    ) -> Option<Arc<S::Output>> {
        let sub_id = sub.id();
        self.sources
            .write()
            .entry(self_id)
            .or_default()
            .insert(sub_id);

        self.subscribers
            .write()
            .entry(sub_id)
            .or_default()
            .insert(self_id);

        self.get_node_by_id::<S>(sub_id)
    }

    fn update_node_if_necessary(&self, root_id: u64) {
        let state = self.get_node_state(root_id);
        if matches!(state, None | Some(NodeState::Clean)) {
            return;
        }

        let mut queue: Vec<u64> = vec![root_id];
        while let Some(id) = queue.pop() {
            let node = self.get_reactive_by_id(id);
            let sub = self.get_subscription_by_id(id);
            if let (Some(sub), Some(node)) = (sub, node) {
                node.run(self.clone(), sub);
            }

            if let Some(sources) = self.get_node_sources(id) {
                for source_id in sources {
                    if self.get_node_state(source_id) == Some(NodeState::Dirty) {
                        queue.push(source_id);
                    }
                }
            }
        }
    }

    pub(crate) fn mark_subs_as_dirty(&self, root_id: u64) {
        let mut stack = Vec::new();

        if let Some(subs) = self.get_node_subscribers(root_id) {
            for sub_id in subs {
                stack.push(sub_id);
            }
        }

        while let Some(id) = stack.pop() {
            self.set_node_state(id, NodeState::Dirty);

            if let Some(subs) = self.get_node_subscribers(id) {
                for sub_id in subs {
                    stack.push(sub_id);
                }
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::eve::EveBuilder;

        struct App {
            msg: String,
        }
        impl App {
            fn new() -> Self {
                Self {
                    msg: "hello world".to_string(),
                }
            }
        }


        #[derive(Debug, Clone, Hash)]
        pub struct FirstSub;

        impl Subscription for FirstSub {
            type Output;
        }

        #[derive(Debug, Clone, Hash)]
        pub struct SecondSub;
        
        fn first_sub_handler<S: FirstSub>(sub: S, app: &App, ctx: &SubscriptionContext<App, S>)

    fn subscriptions_test() {
        let eve = EveBuilder::new(App::new()).
    }
}
