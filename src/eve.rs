use std::{any::TypeId, marker::PhantomData, sync::Arc};

use parking_lot::Mutex;
use parking_lot::RwLock;
use rustc_hash::{FxHashMap as HashMap, FxHashSet as HashSet, FxHasher};
use tracing::error;

use crate::reactive::Reactive;
use crate::BoxableValue;
use crate::{
    errors::DispatchError,
    event::{
        Event, Eventable, ReadHandler, ReadHandlerFn, ReadHandlerWrapper, SideEffect,
        SideEffectHandler, SideEffectHandlerFn, SideEffectHandlerWrapper, SystemEvent,
        WriteHandler, WriteHandlerFn, WriteHandlerWrapper,
    },
    reactive::{NodeState, NodeValue},
};

pub type ReadHandlers<A> = HashMap<TypeId, Vec<Box<dyn ReadHandlerFn<A> + Send + Sync>>>;
pub type WriteHandlers<A> = HashMap<TypeId, Vec<Box<dyn WriteHandlerFn<A> + Send + Sync>>>;
pub type SideEffectHandlers<A> =
    HashMap<TypeId, Vec<Box<dyn SideEffectHandlerFn<A> + Send + Sync>>>;

#[derive(Debug)]
pub struct EveBuilder<A: Send + Sync + Clone> {
    pub app: A,
    pub(crate) incoming_tx: tokio::sync::mpsc::UnboundedSender<SystemEvent>,
    pub(crate) incoming_rx: tokio::sync::mpsc::UnboundedReceiver<SystemEvent>,
    pub(crate) outgoing_tx: tokio::sync::mpsc::UnboundedSender<SideEffect>,
    pub(crate) outgoing_rx: tokio::sync::mpsc::UnboundedReceiver<SideEffect>,
    pub(crate) read_handlers: ReadHandlers<A>,
    pub(crate) write_handlers: WriteHandlers<A>,
    pub(crate) side_effect_handlers: SideEffectHandlers<A>,
    //Reactively
    pub(crate) statuses: HashMap<u64, Arc<RwLock<NodeState>>>,
    pub(crate) sources: HashMap<u64, HashSet<u64>>,
    pub(crate) subscribers: HashMap<u64, HashSet<u64>>,
    pub(crate) reactive: HashMap<TypeId, Arc<dyn Reactive<A>>>,
    pub(crate) values: HashMap<u64, NodeValue>,
    pub(crate) subscription_mapping: HashMap<u64, TypeId>,
    pub(crate) subscriptions: HashMap<u64, Arc<dyn BoxableValue>>,
}

impl<A: Send + Sync + Clone + 'static> EveBuilder<A> {
    pub fn new(app: A) -> Self {
        let (input_tx, input_rx) = tokio::sync::mpsc::unbounded_channel();
        let (output_tx, output_rx) = tokio::sync::mpsc::unbounded_channel();

        Self {
            app,
            incoming_tx: input_tx,
            incoming_rx: input_rx,
            outgoing_tx: output_tx,
            outgoing_rx: output_rx,
            read_handlers: HashMap::default(),
            write_handlers: HashMap::default(),
            side_effect_handlers: HashMap::default(),

            statuses: HashMap::default(),
            sources: HashMap::default(),
            subscribers: HashMap::default(),
            reactive: HashMap::default(),
            values: HashMap::default(),
            subscription_mapping: HashMap::default(),
            subscriptions: HashMap::default(),
        }
    }

    #[must_use]
    pub fn reg_read_handler<E, H, T>(mut self, handler: H) -> Self
    where
        E: Eventable,
        H: ReadHandler<A, E, T> + Copy + 'static,
        T: Send + Sync + 'static,
    {
        let id = TypeId::of::<E>();
        let wrapper = ReadHandlerWrapper {
            handler,
            phantom: PhantomData::<(A, E, T)>,
        };
        self.read_handlers
            .entry(id)
            .or_default()
            .push(Box::new(wrapper));

        self
    }

    #[must_use]
    pub fn reg_write_handler<E, H, T>(mut self, handler: H) -> Self
    where
        E: Eventable,
        H: WriteHandler<A, E, T> + Copy + 'static,
        T: Send + Sync + 'static,
    {
        let id = TypeId::of::<E>();
        let wrapper = WriteHandlerWrapper {
            handler,
            phantom: PhantomData::<(A, E, T)>,
        };
        self.write_handlers
            .entry(id)
            .or_default()
            .push(Box::new(wrapper));

        self
    }

    #[must_use]
    pub fn reg_side_effect_handler<E, H, T>(mut self, handler: H) -> Self
    where
        E: Eventable + Clone,
        H: SideEffectHandler<A, E, T> + Copy + 'static,
        T: Send + Sync + 'static,
    {
        let id = TypeId::of::<E>();
        let wrapper = SideEffectHandlerWrapper {
            handler,
            phantom: PhantomData::<(A, E, T)>,
        };
        self.side_effect_handlers
            .entry(id)
            .or_default()
            .push(Box::new(wrapper));

        self
    }

    pub fn build(self) -> Eve<A> {
        let eve = Eve {
            app: Arc::new(RwLock::new(self.app)),
            incoming_tx: self.incoming_tx,
            outgoing_tx: self.outgoing_tx,
            read_handlers: Arc::new(self.read_handlers),
            write_handlers: Arc::new(self.write_handlers),
            side_effect_handlers: Arc::new(self.side_effect_handlers),

            statuses: Arc::default(),
            sources: Arc::default(),
            subscribers: Arc::default(),
            reactive: Arc::default(),
            values: Arc::default(),
            subscription_mapping: Arc::default(),
            subscriptions: Arc::default(),
        };
        run_events_loop(self.incoming_rx, self.outgoing_rx, eve.clone());
        eve
    }
}

#[derive(Debug, Clone)]
pub struct Eve<A: Send + Sync + Clone> {
    pub app: Arc<RwLock<A>>,
    pub(crate) incoming_tx: tokio::sync::mpsc::UnboundedSender<SystemEvent>,
    pub(crate) outgoing_tx: tokio::sync::mpsc::UnboundedSender<SideEffect>,
    pub(crate) read_handlers: Arc<ReadHandlers<A>>,
    pub(crate) write_handlers: Arc<WriteHandlers<A>>,
    pub(crate) side_effect_handlers: Arc<SideEffectHandlers<A>>,

    // Reactively
    pub(crate) statuses: Arc<RwLock<HashMap<u64, NodeState>>>,
    pub(crate) sources: Arc<RwLock<HashMap<u64, HashSet<u64>>>>,
    pub(crate) subscribers: Arc<RwLock<HashMap<u64, HashSet<u64>>>>,
    pub(crate) reactive: Arc<RwLock<HashMap<TypeId, Arc<dyn Reactive<A>>>>>,
    pub(crate) values: Arc<RwLock<HashMap<u64, NodeValue>>>,
    pub(crate) subscription_mapping: Arc<RwLock<HashMap<u64, TypeId>>>,
    // Props
    pub(crate) subscriptions: Arc<RwLock<HashMap<u64, Arc<dyn BoxableValue>>>>,
}

impl<A> Eve<A>
where
    A: Send + Sync + Clone + 'static,
{
    #[must_use]
    pub fn get_read_handlers(
        &self,
        id: TypeId,
    ) -> Option<Vec<Box<dyn ReadHandlerFn<A> + Send + Sync>>> {
        self.read_handlers.get(&id).cloned()
    }

    #[must_use]
    pub fn get_write_handlers(
        &self,
        id: TypeId,
    ) -> Option<Vec<Box<dyn WriteHandlerFn<A> + Send + Sync>>> {
        self.write_handlers.get(&id).cloned()
    }

    #[must_use]
    pub fn get_side_effect_handlers(
        &self,
        id: TypeId,
    ) -> Option<Vec<Box<dyn SideEffectHandlerFn<A> + Send + Sync>>> {
        self.side_effect_handlers.get(&id).cloned()
    }

    pub fn dispatch<T: Into<Event>>(
        &self,
        event: T,
    ) -> Result<(), DispatchError<SystemEvent, SideEffect>> {
        match event.into() {
            Event::System(e) => self
                .incoming_tx
                .send(e)
                .map_err(DispatchError::SendSystemEventError),
            Event::SideEffect(e) => self
                .outgoing_tx
                .send(e)
                .map_err(DispatchError::SendSideEffectError),
        }
    }
}

fn run_events_loop<S>(
    mut input_rx: tokio::sync::mpsc::UnboundedReceiver<SystemEvent>,
    mut output_rx: tokio::sync::mpsc::UnboundedReceiver<SideEffect>,
    eve: Eve<S>,
) where
    S: Send + Sync + Clone + 'static,
{
    tokio::spawn(async move {
        loop {
            tokio::select! {
                Some(SystemEvent { id, data }) = input_rx.recv() => {
                    if let Some(write_handlers) = eve.get_write_handlers(id) {
                        for handler in write_handlers {
                            match handler.write(Arc::clone(&data), eve.clone()) {
                                None => {}
                                Some(events) => {
                                    for event in events.0 {
                                        if let Err(e) = eve.dispatch(event) {
                                            error!("Dispatch effect error: {:?}", e);
                                        }
                                    }
                                }
                            }
                        }
                    } else {
                        error!("No write handler for event id: {:?}", id);
                    }
                    if let Some(read_handlers) = eve.get_read_handlers(id) {
                        for handler in read_handlers {
                            let eve = eve.clone();
                            let data = Arc::clone(&data);
                            tokio::spawn(async move {
                                match handler.read(data, eve.clone()) {
                                    None => {}
                                    Some(events) => {
                                        for effect in events.0 {
                                            if let Err(e) = eve.dispatch(effect) {
                                                error!("Dispatch effect error: {:?}", e);
                                            }
                                        }
                                    }
                                }
                            });
                        }
                    }
                }
                Some(event) = output_rx.recv() => {
                    if let Some(handlers) = eve.get_side_effect_handlers(event.id) {
                        for handler in handlers {
                            let eve = eve.clone();
                            let event = event.clone();
                            tokio::spawn(async move {
                                match handler.side_effect(event.data, eve.clone()).await {
                                    None => {}
                                    Some(events) => {
                                        for effect in events.0 {
                                            if let Err(e) = eve.dispatch(effect) {
                                                error!("Dispatch effect error: {:?}", e);
                                            }
                                        }
                                    }
                                }
                            });
                        }
                    }

                }
            }
        }
    });
}

#[cfg(test)]
mod test {
    use std::hash::{Hash, Hasher};

    use crate::event::FromEve;

    use super::*;

    #[derive(Clone)]
    pub struct App;
    #[test]
    fn fx_hasher_test() {
        #[derive(Hash, PartialEq, Eq)]
        struct MyStruct {
            x: i32,
        }

        let mut hasher = FxHasher::default();
        let my_struct = MyStruct { x: 42 };
        my_struct.hash(&mut hasher);
        let first_hash = hasher.finish();
        hasher = FxHasher::default();
        my_struct.hash(&mut hasher);
        let second_hash = hasher.finish();
        assert_eq!(first_hash, second_hash);
    }
}
