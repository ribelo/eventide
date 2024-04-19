use std::{any::TypeId, marker::PhantomData, sync::Arc};

use parking_lot::{Mutex, RwLock};
use rustc_hash::FxHashMap as HashMap;
use tracing::error;

use crate::event::{
    Event, Eventable, ReadHandler, ReadHandlerFn, ReadHandlerWrapper, SideEffectHandler,
    SideEffectHandlerFn, SideEffectHandlerWrapper, WriteHandler, WriteHandlerFn,
    WriteHandlerWrapper,
};

pub type ReadHandlers<A> = HashMap<TypeId, Vec<Box<dyn ReadHandlerFn<A> + Send + Sync>>>;
pub type WriteHandlers<A> = HashMap<TypeId, Vec<Box<dyn WriteHandlerFn<A> + Send + Sync>>>;
pub type SideEffectHandlers<A> =
    HashMap<TypeId, Vec<Box<dyn SideEffectHandlerFn<A> + Send + Sync>>>;

#[derive(Debug, Clone)]
pub struct Eve<A: Send + Sync + Clone> {
    pub app: A,
    pub(crate) incoming_tx: tokio::sync::mpsc::UnboundedSender<Event>,
    pub(crate) outgoing_tx: tokio::sync::mpsc::UnboundedSender<Event>,
    pub(crate) read_handlers: Arc<RwLock<ReadHandlers<A>>>,
    pub(crate) write_handlers: Arc<RwLock<WriteHandlers<A>>>,
    pub(crate) side_effect_handlers: Arc<RwLock<SideEffectHandlers<A>>>,
}

pub trait ToSnapshot {
    type Snapshot: Send + Sync + 'static;
    fn to_snapshot(&self) -> Self::Snapshot;
}

pub trait Eve {
    type App: Send + Sync + Clone + ToSnapshot + 'static;
    type Context: Send + Sync + 'static;
    fn new(app: Self::App) -> Self;
}

// impl<A> Eve<A>
// where
//     A: Send + Sync + Clone + ToSnapshot + 'static,
// {
//     pub fn new(app: A) -> Self {
//         let (input_tx, input_rx) = tokio::sync::mpsc::unbounded_channel();
//         let (output_tx, output_rx) = tokio::sync::mpsc::unbounded_channel();
//
//         Self {
//             app,
//             incoming_tx: input_tx,
//             outgoing_tx: output_tx,
//             read_handlers: Arc::default(),
//             write_handlers: Arc::default(),
//             side_effect_handlers: Arc::default(),
//         }
//     }
//
//     #[must_use]
//     pub fn reg_read_handler<E, H>(self, handler: H) -> Self
//     where
//         E: Eventable,
//         H: ReadHandler<A, E> + Copy + 'static,
//     {
//         let id = TypeId::of::<E>();
//         let wrapper = ReadHandlerWrapper {
//             handler,
//             phantom: PhantomData::<(A, E)>,
//         };
//         self.read_handlers
//             .write()
//             .entry(id)
//             .or_default()
//             .push(Box::new(wrapper));
//
//         self
//     }
//
//     #[must_use]
//     pub fn reg_write_handler<E, H>(self, handler: H) -> Self
//     where
//         E: Eventable,
//         H: WriteHandler<A, E> + Copy + 'static,
//     {
//         let id = TypeId::of::<E>();
//         let wrapper = WriteHandlerWrapper {
//             handler,
//             phantom: PhantomData::<(A, E)>,
//         };
//         self.write_handlers
//             .write()
//             .entry(id)
//             .or_default()
//             .push(Box::new(wrapper));
//
//         self
//     }
//
//     #[must_use]
//     pub fn reg_side_effect_handler<E, H>(self, handler: H) -> Self
//     where
//         E: Eventable + Clone,
//         H: SideEffectHandler<A, E> + Copy + 'static,
//     {
//         let id = TypeId::of::<E>();
//         let wrapper = SideEffectHandlerWrapper {
//             handler,
//             phantom: PhantomData::<(A, E)>,
//         };
//         self.side_effect_handlers
//             .write()
//             .entry(id)
//             .or_default()
//             .push(Box::new(wrapper));
//
//         self
//     }
// }
//
// impl<A> Eve<A>
// where
//     A: Send + Sync + Clone + ToSnapshot + 'static,
// {
//     #[must_use]
//     pub fn get_read_handlers(
//         &self,
//         id: TypeId,
//     ) -> Option<Vec<Box<dyn ReadHandlerFn<A> + Send + Sync>>> {
//         self.read_handlers.read().get(&id).cloned()
//     }
//
//     #[must_use]
//     pub fn get_write_handlers(
//         &self,
//         id: TypeId,
//     ) -> Option<Vec<Box<dyn WriteHandlerFn<A> + Send + Sync>>> {
//         self.write_handlers.read().get(&id).cloned()
//     }
//
//     #[must_use]
//     pub fn get_side_effect_handlers(
//         &self,
//         id: TypeId,
//     ) -> Option<Vec<Box<dyn SideEffectHandlerFn<A> + Send + Sync>>> {
//         self.side_effect_handlers.read().get(&id).cloned()
//     }
//
//     pub fn dispatch<T: Eventable>(
//         &self,
//         event: T,
//     ) -> Result<(), tokio::sync::mpsc::error::SendError<Event>> {
//         self.incoming_tx.send(Event::new(event))
//     }
// }
//
// fn run_events_loop<S>(
//     mut input_rx: tokio::sync::mpsc::UnboundedReceiver<Event>,
//     mut output_rx: tokio::sync::mpsc::UnboundedReceiver<Event>,
//     mut eve: Eve<S>,
// ) where
//     S: Send + Sync + Clone + ToSnapshot + 'static,
// {
//     tokio::spawn(async move {
//         loop {
//             tokio::select! {
//                 Some(event) = input_rx.recv() => {
//                     if let Some(write_handlers) = eve.get_write_handlers(event.id) {
//                         for handler in write_handlers {
//                             let event = event.clone();
//                             match handler.write(event.inner, &mut eve.app) {
//                                 None => {}
//                                 Some(events) => {
//                                     for effect in events.0 {
//                                         if let Err(e) = eve.dispatch(effect) {
//                                             error!("Dispatch effect error: {:?}", e);
//                                         }
//                                     }
//                                 }
//                             }
//                         }
//                     } else {
//                         println!("no write handler");
//                     }
//                     if let Some(read_handlers) = eve.get_read_handlers(event.id) {
//                         for handler in read_handlers {
//                             let eve = eve.clone();
//                             let event = event.clone();
//                             let snap = eve.app.to_snapshot();
//                             tokio::spawn(async move {
//                                 match handler.read(event.inner, snap) {
//                                     None => {}
//                                     Some(events) => {
//                                         for effect in events.0 {
//                                             if let Err(e) = eve.dispatch(effect) {
//                                                 error!("Dispatch effect error: {:?}", e);
//                                             }
//                                         }
//                                     }
//                                 }
//                             });
//                         }
//                     }
//                 }
//                 Some(event) = output_rx.recv() => {
//                     if let Some(handlers) = eve.get_side_effect_handlers(event.id) {
//                         for handler in handlers {
//                             let eve = eve.clone();
//                             let event = event.clone();
//                             let snap = eve.app.to_snapshot();
//                             tokio::spawn(async move {
//                                 match handler.side_effect(event.inner, snap).await {
//                                     None => {}
//                                     Some(events) => {
//                                         for effect in events.0 {
//                                             if let Err(e) = eve.dispatch(effect) {
//                                                 error!("Dispatch effect error: {:?}", e);
//                                             }
//                                         }
//                                     }
//                                 }
//                             });
//                         }
//                     }
//
//                 }
//             }
//         }
//     });
// }

#[cfg(test)]
mod test {
    use super::*;
    use std::sync::LazyLock;

    #[derive(Clone)]
    pub struct App;

    impl ToSnapshot for App {
        type Snapshot = App;

        fn to_snapshot(&self) -> Self::Snapshot {
            self.clone()
        }
    }
}
