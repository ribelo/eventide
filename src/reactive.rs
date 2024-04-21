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
    subscription::{Subscription, SubscriptionContext},
    BoxableValue,
};

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
    pub fn is_initialized(&self) -> bool {
        matches!(self, NodeValue::Memoized(_))
    }
}

#[derive(Debug, Clone)]
pub struct Sub<A, S>
where
    A: Send + Sync + Clone + 'static,
    S: Subscription<A>,
{
    self_id: u64,
    sub_id: u64,
    eve: Eve<A>,
    _phantom: PhantomData<S>,
}

impl<A, S> Sub<A, S>
where
    A: Send + Sync + Clone + 'static,
    S: Subscription<A>,
{
    pub fn new(self_id: u64, sub_id: u64, eve: Eve<A>) -> Self {
        Self {
            self_id,
            sub_id,
            eve,
            _phantom: PhantomData,
        }
    }
    pub fn get(&self) -> S::Output {
        self.eve
            .get_node_by_id::<S>(self.sub_id)
            .unwrap()
            .as_ref()
            .clone()
    }
}

impl<A, S> Drop for Sub<A, S>
where
    A: Send + Sync + Clone + 'static,
    S: Subscription<A>,
{
    fn drop(&mut self) {
        self.eve.try_cleanup_node(self.self_id);
    }
}

pub trait Handler<A>: DynClone + Send + Sync + 'static
where
    A: Send + Sync + Clone,
{
    fn handle(&self, eve: &Eve<A>);
}
dyn_clone::clone_trait_object!(<A> Handler<A>);

impl<A> fmt::Debug for dyn Handler<A>
where
    A: Send + Sync + Clone,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<SubHandler>")
    }
}

#[derive(Clone, Debug)]
pub struct SubHandler<A, S>
where
    A: Send + Sync + Clone + 'static,
    S: Subscription<A>,
{
    id: u64,
    eve: Eve<A>,
    _phantom: PhantomData<S>,
}

impl<A, S> SubHandler<A, S>
where
    A: Send + Sync + Clone + 'static,
    S: Subscription<A>,
{
    pub fn new(id: u64, eve: Eve<A>) -> Self {
        Self {
            id,
            eve,
            _phantom: PhantomData,
        }
    }
}

impl<A, S> Handler<A> for SubHandler<A, S>
where
    A: Send + Sync + Clone + 'static,
    S: Subscription<A> + 'static,
{
    fn handle(&self, eve: &Eve<A>) {
        if let Some(sub) = eve
            .get_subscription_by_id(self.id)
            .and_then(|boxed| boxed.downcast_arc::<S>().ok())
        {
            sub.handle(&eve.app.read(), SubscriptionContext::new(&sub, eve.clone()));
        }
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
    pub fn get_node<S: Subscription<A>>(&self, props: &S) -> Option<Arc<S::Output>> {
        let id = props.id();
        self.update_node_if_necessary(id);
        self.get_node_by_id::<S>(id)
    }

    #[must_use]
    pub fn get_node_by_id<S: Subscription<A>>(&self, id: u64) -> Option<Arc<S::Output>> {
        self.update_node_if_necessary(id);
        self.values.read().get(&id)?.get()
    }

    #[must_use]
    pub fn get_raw_node_by_id(&self, id: u64) -> Option<NodeValue> {
        self.values.read().get(&id).cloned()
    }

    pub fn set_node_by_id<S: Subscription<A>>(&self, id: u64, value: S::Output) {
        let node_value = NodeValue::new(value);
        self.values.write().insert(id, node_value);
        self.mark_subs_as_dirty(id);
    }

    #[must_use]
    pub fn get_sub_handler_by_id(&self, id: u64) -> Option<Arc<dyn Handler<A>>> {
        self.sub_handlers.read().get(&id).cloned()
    }

    pub fn remove_node_by_id(&self, id: u64) {
        self.statuses.write().remove(&id);
        self.sources.write().remove(&id);
        self.values.write().remove(&id);
    }

    fn get_node_sources(&self, id: u64) -> Option<HashSet<u64>> {
        self.sources.read().get(&id).cloned()
    }

    fn get_node_subscribers(&self, id: u64) -> Option<HashSet<u64>> {
        self.subscribers.read().get(&id).cloned()
    }

    fn try_cleanup_node(&self, id: u64) {
        println!("Cleaning up node: {}", id);
        let mut stack = vec![id];
        while let Some(node_id) = stack.pop() {
            if let Some(sources) = self.sources.write().remove(&node_id) {
                for source_id in sources {
                    if let Some(subs) = self.subscribers.write().get_mut(&source_id) {
                        subs.remove(&node_id);
                        if subs.is_empty() {
                            stack.push(source_id);
                        }
                    }
                }
            }

            if let Some(subs) = self.subscribers.read().get(&node_id) {
                if subs.is_empty() {
                    self.remove_node_by_id(node_id);
                }
            }
        }
    }

    pub fn reg_subscribe<S>(&self, self_id: u64, sub: S) -> Sub<A, S>
    where
        S: Subscription<A> + 'static,
    {
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

        match self.get_raw_node_by_id(sub_id) {
            None => {
                let value = sub.handle(
                    &self.app.read(),
                    SubscriptionContext::new(&sub, self.clone()),
                );
                self.set_node_state(sub_id, NodeState::Clean);
                self.set_node_by_id::<S>(sub_id, value);
                self.subscriptions.write().insert(sub_id, Arc::new(sub));

                let handler = SubHandler::<A, S>::new(sub_id, self.clone());
                self.sub_handlers.write().insert(sub_id, Arc::new(handler));

                Sub::new(self_id, sub_id, self.clone())
            }
            Some(NodeValue::Memoized(_) | NodeValue::Uninitialized) => {
                Sub::new(self_id, sub_id, self.clone())
            }
        }
    }

    pub fn subscribe<S>(&self, sub: S) -> Sub<A, S>
    where
        S: Subscription<A> + 'static,
    {
        let self_id = rand::random();
        self.reg_subscribe(self_id, sub)
    }

    fn update_node_if_necessary(&self, root_id: u64) {
        let state = self.get_node_state(root_id);
        if matches!(state, None | Some(NodeState::Clean)) {
            return;
        }

        let mut queue: Vec<u64> = vec![root_id];
        while let Some(id) = queue.pop() {
            if let Some(handler) = self.get_sub_handler_by_id(id) {
                handler.handle(self);
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

    #[tokio::test]
    async fn diamond_test() {
        #[derive(Debug, Clone)]
        struct App {
            start: i32,
        };

        #[derive(Debug, Clone, Hash)]
        pub struct NodeA;

        impl Subscription<App> for NodeA {
            type Output = i32;
            fn handle(&self, app: &App, ctx: SubscriptionContext<NodeA, Self>) -> Self::Output {
                println!("Update Node A");
                0
            }
        }

        #[derive(Debug, Clone, Hash)]
        pub struct NodeB;

        impl Subscription<App> for NodeA {
            type Output = i32;
            fn handle(&self, app: &NodeA, ctx: SubscriptionContext<NodeB, Self>) -> Self::Output {
                println!("Update Node A: {}", self.0);
                let a = ctx.subscribe(NodeA);
            }
        }

        #[derive(Debug, Clone, Hash)]
        pub struct NodeC(i32);
        #[derive(Debug, Clone, Hash)]
        pub struct NodeD(i32);
        #[derive(Debug, Clone, Hash)]
        pub struct NodeE(i32);
        #[derive(Debug, Clone, Hash)]
        pub struct NodeF(i32);
        #[derive(Debug, Clone, Hash)]
        pub struct NodeG(i32);
        #[derive(Debug, Clone, Hash)]
        pub struct NodeH(i32);

        let eve = EveBuilder::new(())
            .reg_signal(|| NodeA(0))
            .reg_memo(|a: Node<NodeA>| async move {
                println!("update B a:{}", a.0 .0);
                NodeB(a.0 .0 + 1)
            })
            .reg_memo(|a: Node<NodeA>| async move {
                println!("update C");
                NodeC(a.0 .0 + 1)
            })
            .reg_memo(|b: Node<NodeB>| async move {
                println!("update D");
                NodeD(b.0 .0 + 1)
            })
            .reg_memo(|b: Node<NodeB>| async move {
                println!("update E");
                NodeE(b.0 .0 + 1)
            })
            .reg_memo(|c: Node<NodeC>| async move {
                println!("update F");
                NodeF(c.0 .0 + 1)
            })
            .reg_memo(|c: Node<NodeC>| async move {
                println!("update G");
                NodeG(c.0 .0 + 1)
            })
            .reg_memo(
                |d: Node<NodeD>, e: Node<NodeE>, f: Node<NodeF>, g: Node<NodeG>| async move {
                    println!("update H");
                    NodeH(d.0 .0 + e.0 .0 + f.0 .0 + g.0 .0)
                },
            )
            .build()
            .unwrap();

        assert_eq!(
            *eve.get_node::<NodeH>().await.unwrap(),
            NodeH(8),
            "Initial NodeH value should be 8"
        );

        println!("set new value to  A");
        eve.set_node_value(NodeA(2)).await;

        assert_eq!(
            *eve.get_node::<NodeB>().await.unwrap(),
            NodeB(3),
            "NodeB value should be 3"
        );
        assert_eq!(
            *eve.get_node::<NodeC>().await.unwrap(),
            NodeC(3),
            "NodeC value should be 3"
        );
        assert_eq!(
            *eve.get_node::<NodeD>().await.unwrap(),
            NodeD(4),
            "NodeD value should be 4"
        );
        assert_eq!(
            *eve.get_node::<NodeE>().await.unwrap(),
            NodeE(4),
            "NodeE value should be 4"
        );
        assert_eq!(
            *eve.get_node::<NodeF>().await.unwrap(),
            NodeF(4),
            "NodeF value should be 4"
        );
        assert_eq!(
            *eve.get_node::<NodeG>().await.unwrap(),
            NodeG(4),
            "NodeG value should be 4"
        );
        assert_eq!(
            *eve.get_node::<NodeH>().await.unwrap(),
            NodeH(16),
            "NodeH value should be 16"
        );
    }
}
