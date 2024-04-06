use std::{
    marker::PhantomData,
    sync::{Arc, Mutex},
};

use eventide_derive::Event;
use rustc_hash::FxHashMap as HashMap;
use tracing::{error, info};

use crate::{
    error::RunEveError,
    event::{
        Effect, EffectError, ErrorHandler, ErrorHandlerFn, ErrorHandlerWrapper, Event, Eventable,
        ReadHandler, ReadHandlerFn, ReadHandlerWrapper, SideEffect, SideEffectHandler,
        SideEffectHandlerFn, SideEffectHandlerWrapper, WriteHandler, WriteHandlerFn,
        WriteHandlerWrapper,
    },
    id::Id,
};

pub type ReadHandlers<S> = HashMap<Id, Vec<Box<dyn ReadHandlerFn<S> + Send + Sync>>>;
pub type WriteHandlers<S> = HashMap<Id, Vec<Box<dyn WriteHandlerFn<S> + Send + Sync>>>;
pub type SideEffectHandlers<S> = HashMap<Id, Vec<Box<dyn SideEffectHandlerFn<S> + Send + Sync>>>;
pub type ErrorHandlers<S> = HashMap<Id, Box<dyn ErrorHandlerFn<S> + Send + Sync>>;

pub trait ToSnapshot {
    type Snapshot: Send + Sync + Clone + 'static;
    fn to_snapshot(&self) -> Self::Snapshot;
}

#[derive(Clone)]
pub struct Eve<S>
where
    S: Send + Sync + Clone + ToSnapshot + 'static,
{
    pub state: S,
    pub(crate) incoming_tx: tokio::sync::mpsc::UnboundedSender<Effect>,
    pub(crate) outgoing_tx: tokio::sync::mpsc::UnboundedSender<SideEffect>,
    pub(crate) error_tx: tokio::sync::mpsc::UnboundedSender<EffectError>,
    pub(crate) read_handlers: Arc<ReadHandlers<S>>,
    pub(crate) write_handlers: Arc<WriteHandlers<S>>,
    pub(crate) side_effect_handlers: Arc<SideEffectHandlers<S>>,
    pub(crate) error_handlers: Arc<ErrorHandlers<S>>,
}

pub struct EveBuilder<S>
where
    S: Send + Sync + Clone + ToSnapshot + 'static,
{
    pub state: Option<S>,
    pub(crate) incoming_tx: tokio::sync::mpsc::UnboundedSender<Effect>,
    pub(crate) incoming_rx: Option<tokio::sync::mpsc::UnboundedReceiver<Effect>>,
    pub(crate) outcoming_tx: tokio::sync::mpsc::UnboundedSender<SideEffect>,
    pub(crate) outcoming_rx: Option<tokio::sync::mpsc::UnboundedReceiver<SideEffect>>,
    pub(crate) error_tx: tokio::sync::mpsc::UnboundedSender<EffectError>,
    pub(crate) error_rx: Option<tokio::sync::mpsc::UnboundedReceiver<EffectError>>,
    pub(crate) read_handlers: ReadHandlers<S>,
    pub(crate) write_handlers: WriteHandlers<S>,
    pub(crate) side_effect_handlers: SideEffectHandlers<S>,
    pub(crate) error_handlers: ErrorHandlers<S>,
}

impl<S> EveBuilder<S>
where
    S: Send + Sync + Clone + ToSnapshot + 'static,
{
    pub fn new(state: S) -> Self {
        let (input_tx, input_rx) = tokio::sync::mpsc::unbounded_channel();
        let (output_tx, output_rx) = tokio::sync::mpsc::unbounded_channel();
        let (error_tx, error_rx) = tokio::sync::mpsc::unbounded_channel();

        Self {
            state: Some(state),
            incoming_tx: input_tx,
            incoming_rx: Some(input_rx),
            outcoming_tx: output_tx,
            outcoming_rx: Some(output_rx),
            error_tx,
            error_rx: Some(error_rx),
            read_handlers: HashMap::default(),
            write_handlers: HashMap::default(),
            side_effect_handlers: HashMap::default(),
            error_handlers: HashMap::default(),
        }
    }

    pub fn build(&mut self) -> Result<Eve<S>, RunEveError> {
        let eve = Eve {
            state: self.state.take().unwrap(),
            incoming_tx: self.incoming_tx.clone(),
            outgoing_tx: self.outcoming_tx.clone(),
            error_tx: self.error_tx.clone(),
            read_handlers: Arc::new(std::mem::take(&mut self.read_handlers.clone())), // Clone before taking
            write_handlers: Arc::new(std::mem::take(&mut self.write_handlers.clone())), // Clone before taking
            side_effect_handlers: Arc::new(std::mem::take(&mut self.side_effect_handlers.clone())), // Clone before taking
            error_handlers: Arc::new(std::mem::take(&mut self.error_handlers.clone())), // Clone before taking
        };

        run_events_loop(
            self.incoming_rx.take().unwrap(),
            self.outcoming_rx.take().unwrap(),
            self.error_rx.take().unwrap(),
            eve.clone(),
        );

        Ok(eve)
    }

    #[must_use]
    pub fn reg_read_handler<T, H, E>(mut self, handler: H) -> Self
    where
        T: Eventable + Send + Sync + 'static,
        H: ReadHandler<S, T, E> + Copy + Send + Sync + 'static,
        E: std::error::Error + Send + Sync + Clone + 'static,
    {
        let id = Id::new::<T>();
        let wrapper = ReadHandlerWrapper {
            handler,
            phantom: PhantomData::<(S, T, E)>,
        };
        self.read_handlers
            .entry(id)
            .or_default()
            .push(Box::new(wrapper));

        self
    }

    #[must_use]
    pub fn reg_write_handler<T, H>(mut self, handler: H) -> Self
    where
        T: Eventable + Send + Sync + 'static,
        H: WriteHandler<S, T> + Copy + Send + Sync + 'static,
    {
        let id = Id::new::<T>();
        let wrapper = WriteHandlerWrapper {
            handler,
            phantom: PhantomData::<(S, T)>,
        };
        self.write_handlers
            .entry(id)
            .or_default()
            .push(Box::new(wrapper));

        self
    }

    #[must_use]
    pub fn reg_side_effect_handler<T, H>(mut self, handler: H) -> Self
    where
        T: Eventable + Send + Sync + Clone + 'static,
        H: SideEffectHandler<S, T> + Copy + Send + Sync + 'static,
    {
        let id = Id::new::<T>();
        let wrapper = SideEffectHandlerWrapper {
            handler,
            phantom: PhantomData::<(S, T)>,
        };
        self.side_effect_handlers
            .entry(id)
            .or_default()
            .push(Box::new(wrapper));

        self
    }

    #[must_use]
    pub fn reg_error_handler<T, H>(mut self, handler: H) -> Self
    where
        T: std::error::Error + Send + Sync + Clone + 'static,
        H: ErrorHandler<S, T> + Copy + Send + Sync + 'static,
    {
        let id = Id::new::<T>();
        let wrapper = ErrorHandlerWrapper {
            handler,
            phantom: PhantomData::<(S, T)>,
        };
        self.error_handlers.insert(id, Box::new(wrapper));

        self
    }
}

impl<S> Eve<S>
where
    S: Send + Sync + Clone + ToSnapshot + 'static,
{
    #[must_use]
    pub fn get_read_handlers(
        &self,
        id: Id,
    ) -> Option<Vec<Box<dyn ReadHandlerFn<S> + Send + Sync>>> {
        self.read_handlers.get(&id).cloned()
    }

    #[must_use]
    pub fn get_write_handlers(
        &self,
        id: Id,
    ) -> Option<Vec<Box<dyn WriteHandlerFn<S> + Send + Sync>>> {
        self.write_handlers.get(&id).cloned()
    }

    #[must_use]
    pub fn get_side_effect_handlers(
        &self,
        id: Id,
    ) -> Option<Vec<Box<dyn SideEffectHandlerFn<S> + Send + Sync>>> {
        self.side_effect_handlers.get(&id).cloned()
    }

    #[must_use]
    pub fn get_error_handler(&self, id: Id) -> Option<Box<dyn ErrorHandlerFn<S> + Send + Sync>> {
        self.error_handlers.get(&id).cloned()
    }

    pub fn dispatch<T: Into<Effect>>(
        &self,
        event: T,
    ) -> Result<(), tokio::sync::mpsc::error::SendError<Effect>> {
        self.incoming_tx.send(event.into())
    }

    pub fn dispatch_side_effect<T: Into<SideEffect>>(
        &self,
        event: T,
    ) -> Result<(), tokio::sync::mpsc::error::SendError<SideEffect>> {
        self.outgoing_tx.send(event.into())
    }

    fn dispatch_error<E: Into<EffectError>>(
        &self,
        err: E,
    ) -> Result<(), tokio::sync::mpsc::error::SendError<EffectError>> {
        self.error_tx.send(err.into())
    }
}

fn run_events_loop<S>(
    mut input_rx: tokio::sync::mpsc::UnboundedReceiver<Effect>,
    mut output_rx: tokio::sync::mpsc::UnboundedReceiver<SideEffect>,
    mut error_rx: tokio::sync::mpsc::UnboundedReceiver<EffectError>,
    mut eve: Eve<S>,
) where
    S: Send + Sync + Clone + ToSnapshot + 'static,
{
    tokio::spawn(async move {
        let mut set = tokio::task::JoinSet::new();
        loop {
            tokio::select! {
                Some(event) = input_rx.recv() => {
                    if let Some(write_handlers) = eve.get_write_handlers(event.id) {
                        while (set.join_next().await).is_some() {};
                        for handler in write_handlers {
                            let event = event.clone();
                            match handler.write(event.inner, &mut eve.state) {
                                Ok(None) => {}
                                Ok(Some(events)) => {
                                    for effect in events.effects {
                                        if let Err(e) = eve.dispatch(effect) {
                                            error!("Dispatch effect error: {:?}", e);
                                        }
                                    }
                                    for side_effect in events.side_effects {
                                        if let Err(e) = eve.dispatch_side_effect(side_effect) {
                                            error!("Dispatch side effect error: {:?}", e);
                                        }
                                    }
                                }
                                Err(err) => {
                                    if let Err(e) = eve.dispatch_error(err) {
                                        error!("Dispatch error error: {:?}", e);
                                    }
                                }
                            }
                        }
                    } else {
                        println!("no write handler");
                    }
                    if let Some(read_handlers) = eve.get_read_handlers(event.id) {
                        let snap = eve.state.to_snapshot();
                        for handler in read_handlers {
                            let eve = eve.clone();
                            let event = event.clone();
                            let snap = snap.clone();
                            set.spawn(async move {
                                match handler.read(event.inner, snap) {
                                    Ok(None) => {}
                                    Ok(Some(events)) => {
                                        for effect in events.effects {
                                            if let Err(e) = eve.dispatch(effect) {
                                                error!("Dispatch effect error: {:?}", e);
                                            }
                                        }
                                        for side_effect in events.side_effects {
                                            if let Err(e) = eve.dispatch_side_effect(side_effect) {
                                                error!("Dispatch side effect error: {:?}", e);
                                            }
                                        }
                                    }
                                    Err(err) => {
                                        if let Err(e) = eve.dispatch_error(err) {
                                            error!("Dispatch error error: {:?}", e);
                                        }
                                    }
                                }
                            });
                        }
                    }
                }
                Some(event) = output_rx.recv() => {
                    if let Some(handlers) = eve.get_side_effect_handlers(event.id) {
                        let snap = eve.state.to_snapshot();
                        for handler in handlers {
                            let eve = eve.clone();
                            let event = event.clone();
                            let snap = snap.clone();
                            set.spawn(async move {
                                match handler.side_effect(event.inner, snap).await {
                                    Ok(None) => {}
                                    Ok(Some(events)) => {
                                        for effect in events.effects {
                                            if let Err(e) = eve.dispatch(effect) {
                                                error!("Dispatch effect error: {:?}", e);
                                            }
                                        }
                                        for side_effect in events.side_effects {
                                            if let Err(e) = eve.dispatch_side_effect(side_effect) {
                                                error!("Dispatch side effect error: {:?}", e);
                                            }
                                        }
                                    }
                                    Err(err) => {
                                        if let Err(e) = eve.dispatch_error(err) {
                                            error!("Dispatch error error: {:?}", e);
                                        }
                                    }
                                }
                            });
                        }
                    }

                }
                Some(error) = error_rx.recv() => {
                    if let Some(handler) = eve.get_error_handler(error.id) {
                        let snap = eve.state.to_snapshot();
                        let eve = eve.clone();
                        let snap = snap.clone();
                        set.spawn(async move {
                            match handler.handle_error(error.inner, snap) {
                                Ok(None) => {}
                                Ok(Some(events)) => {
                                    for effect in events.effects {
                                        if let Err(e) = eve.dispatch(effect) {
                                            error!("Dispatch effect error: {:?}", e);
                                        }
                                    }
                                    for side_effect in events.side_effects {
                                        if let Err(e) = eve.dispatch_side_effect(side_effect) {
                                            error!("Dispatch side effect error: {:?}", e);
                                        }
                                    }
                                }
                                Err(err) => {
                                    if let Err(e) = eve.dispatch_error(err) {
                                        error!("Dispatch error error: {:?}", e);
                                    }
                                }
                            }
                        });
                    }

                }
            }
        }
    });
}

#[cfg(test)]
mod tests {

    use crate::event::{Effect, EffectResult, Events};

    use super::*;

    #[tokio::test]
    async fn event_test() {
        #[derive(Debug, Clone, Default)]
        pub struct State {
            pub i: i32,
        }

        #[derive(Debug, Clone)]
        pub struct StateSnapshot {
            pub i: i32,
        }

        impl ToSnapshot for State {
            type Snapshot = StateSnapshot;
            fn to_snapshot(&self) -> Self::Snapshot {
                StateSnapshot { i: self.i }
            }
        }

        #[derive(Debug, Clone)]
        struct Ping {
            i: i32,
        }

        impl Eventable for Ping {}

        #[derive(Debug, Clone)]
        struct Pong;

        #[derive(Debug, Clone, thiserror::Error)]
        #[error("Pong error")]
        struct PongError;

        impl Eventable for Pong {}

        fn ping_read_handler(event: &Ping, state: StateSnapshot) -> Result {
            println!("ping_read_handler event:{:?} state:{:?}", event.i, state.i);

            Ok(None)
        }

        fn ping_write_handler(event: &Ping, state: &mut State) -> EffectResult {
            println!("ping_write_handler event:{:?} state:{:?}", event.i, state.i);
            state.i += event.i;

            if state.i < 10 {
                Ok(Some(Events::new().with_effect(event.clone())))
            } else {
                Ok(Some(Events::new().with_side_effect(Pong)))
            }
        }

        async fn pong_side_effect_handler(event: Pong, state: StateSnapshot) -> EffectResult {
            println!("pong_side_effect_handler  state:{:?}", state.i);

            Err(PongError.into())
        }

        fn pong_error_handler(error: &PongError, state: StateSnapshot) -> EffectResult {
            println!("pong_error_handler  state:{:?}", state.i);
            Ok(None)
        }

        let eve = EveBuilder::new(State::default())
            .reg_read_handler(ping_read_handler)
            .reg_write_handler(ping_write_handler)
            .reg_side_effect_handler(pong_side_effect_handler)
            .reg_error_handler(pong_error_handler)
            .build()
            .unwrap();
        eve.dispatch(Ping { i: 1 }).unwrap();

        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
}
