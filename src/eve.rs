use std::{panic::AssertUnwindSafe, sync::Arc};

use parking_lot::RwLock;

use crate::event::{EffectContext, EffectHandler, EffectMsg, EventHandler, EventMsg, Message};

pub trait Eventide: Sized + 'static {
    type Model: Send + Sync + 'static;
    type Capabilities: Send + Sync + 'static;
    type Event: EventHandler<Self> + Send + 'static;
    type Effect: EffectHandler<Self> + Send + 'static;

    fn run(model: Self::Model, caps: Self::Capabilities) -> Eve<Self> {
        let (incoming_tx, incoming_rx) = tokio::sync::mpsc::unbounded_channel();
        let (outgoing_tx, outgoing_rx) = tokio::sync::mpsc::unbounded_channel();
        let cancel_token = tokio_util::sync::CancellationToken::new();
        let state = Arc::new(RwLock::new(model));

        let eve = Eve {
            model: Arc::clone(&state),
            capabilities: Arc::new(caps),
            incoming_tx,
            outgoing_tx,
            cancel_token: Some(cancel_token.clone()),
        };

        run_event_loop(eve.clone(), incoming_rx, cancel_token.clone());
        run_effect_loop(eve.clone(), outgoing_rx, cancel_token);
        eve
    }
}

#[derive(Debug)]
pub struct Eve<A: Eventide> {
    pub model: Arc<RwLock<A::Model>>,
    pub capabilities: Arc<A::Capabilities>,
    pub(crate) incoming_tx: tokio::sync::mpsc::UnboundedSender<EventMsg<A>>,
    pub(crate) outgoing_tx: tokio::sync::mpsc::UnboundedSender<EffectMsg<A>>,
    // pub(crate) ports: HashMap<TypeId,
    pub(crate) cancel_token: Option<tokio_util::sync::CancellationToken>,
}

impl<A: Eventide> Clone for Eve<A> {
    fn clone(&self) -> Self {
        Self {
            model: Arc::clone(&self.model),
            capabilities: Arc::clone(&self.capabilities),
            incoming_tx: self.incoming_tx.clone(),
            outgoing_tx: self.outgoing_tx.clone(),
            cancel_token: self.cancel_token.clone(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DispatchError<A: Eventide> {
    #[error("Error sending event: {0}")]
    SendEventError(tokio::sync::mpsc::error::SendError<EventMsg<A>>),
    #[error("Error sending effect: {0}")]
    SendEffectError(tokio::sync::mpsc::error::SendError<EffectMsg<A>>),
}

#[derive(Debug, thiserror::Error)]
#[error("Event loop already stopped")]
pub struct AlreadyStopped;

impl<A: Eventide> Eve<A> {
    pub fn stop(&mut self) -> Result<(), AlreadyStopped> {
        if let Some(cancel_token) = self.cancel_token.as_ref() {
            cancel_token.cancel();
            self.cancel_token = None;
            Ok(())
        } else {
            Err(AlreadyStopped)
        }
    }

    #[must_use]
    pub fn is_running(&self) -> bool {
        self.cancel_token
            .as_ref()
            .map_or(false, |t| !t.is_cancelled())
    }

    pub fn dispatch<T: Into<Message<A>>>(&self, msg: T) -> Result<(), DispatchError<A>> {
        match msg.into() {
            Message::EventMsg(event) => self
                .incoming_tx
                .send(event)
                .map_err(|e| DispatchError::SendEventError(e)),
            Message::EffectMsg(effect) => self
                .outgoing_tx
                .send(effect)
                .map_err(|e| DispatchError::SendEffectError(e)),
        }
    }

    pub fn model(&self) -> parking_lot::RwLockReadGuard<A::Model> {
        self.model.read()
    }

    pub fn model_mut(&self) -> parking_lot::RwLockWriteGuard<A::Model> {
        self.model.write()
    }

    pub fn with_model<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&A::Model) -> R,
    {
        f(&*self.model.read())
    }

    pub fn with_model_mut<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut A::Model) -> R,
    {
        f(&mut *self.model.write())
    }

    pub fn with_caps<F, Fut>(&self, f: F) -> tokio::task::JoinHandle<()>
    where
        F: FnOnce(&A::Capabilities) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = ()> + Send,
    {
        let caps = Arc::clone(&self.capabilities);
        tokio::spawn(async move {
            f(&caps).await;
        })
    }

    #[must_use]
    pub fn from_model<T>(&self) -> T
    where
        for<'a> T: From<&'a A::Model>,
    {
        T::from(&*self.model.read())
    }

    pub fn spawn<F, Fut, R>(&self, f: F) -> tokio::task::JoinHandle<R>
    where
        F: FnOnce(&Self) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = R> + Send + 'static,
        R: Send + 'static,
    {
        tokio::spawn(f(self))
    }
}

fn run_event_loop<A: Eventide + 'static>(
    eve: Eve<A>,
    mut event_rx: tokio::sync::mpsc::UnboundedReceiver<EventMsg<A>>,
    cancel_token: tokio_util::sync::CancellationToken,
) {
    tokio::spawn(async move {
        loop {
            tokio::select! {
                () = cancel_token.cancelled() => break,
                msg = event_rx.recv() => {
                    if let Some(msg) = msg {
                        let parent_id = msg.id;
                        let src_id = msg.src_id().unwrap_or(parent_id);
                        // Wrap the event handling in a catch_unwind to prevent panics from terminating the loop
                        if let Err(err) = std::panic::catch_unwind(AssertUnwindSafe(|| {
                            if let Some(msgs) = msg.event.handle(&mut eve.model.write()) {
                                for mut msg in msgs {
                                    msg.set_parent_id(parent_id);
                                    msg.set_src_id(src_id);
                                    if let Err(err) = eve.dispatch(msg) {
                                        tracing::error!("Error dispatching message: {err}");
                                        return;
                                    }
                                }
                            }
                        })) {
                            tracing::error!("Panic occurred during event handling: {:?}", err);
                        }
                    }
                }
            }
        }
    });
}

fn run_effect_loop<A: Eventide + 'static>(
    eve: Eve<A>,
    mut rx: tokio::sync::mpsc::UnboundedReceiver<EffectMsg<A>>,
    cancel_token: tokio_util::sync::CancellationToken,
) {
    tokio::spawn(async move {
        loop {
            tokio::select! {
                () = cancel_token.cancelled() => break,
                msg = rx.recv() => {
                    if let Some(msg) = msg {
                        let parent_id = msg.id;
                        let src_id = msg.src_id().unwrap_or(parent_id);
                        let eve = eve.clone();
                        let ctx = EffectContext::from((&eve, parent_id));
                        let handle = tokio::spawn(async move {
                            if let Some(msgs) = msg.effect.handle(ctx).await {
                                for mut msg in msgs {
                                    msg.set_parent_id(parent_id);
                                    msg.set_src_id(src_id);
                                    if let Err(err) = eve.dispatch(msg) {
                                        tracing::error!("Error dispatching message: {err}");
                                        return;
                                    }
                                }
                            }
                        });
                        match handle.await {
                            Ok(_) => {}
                            Err(err) => {
                                tracing::error!("Error occurred during effect handling: {:?}", err);
                            }
                        }
                    }
                }
            }
        }
    });
}

#[cfg(test)]
mod test {

    use super::*;

    // struct Model;
    // struct Capabilities;
    // #[derive(Debug)]
    // struct Event;
    // struct Effect;
    //
    // struct App;
    //
    // impl Eventide for App {
    //     type Model = Model;
    //     type Capabilities = Capabilities;
    //     type Event = Event;
    //     type Effect = Effect;
    // }

    // impl EventHandler<App> for Event {
    //     fn handle(self, _model: &mut Model) -> Option<Vec<Message<App>>> {
    //         None
    //     }
    // }

    // impl EffectHandler<App> for Effect {
    //     async fn handle(self, _caps: &Capabilities) -> Option<Vec<Message<App>>> {
    //         None
    //     }
    // }
    // #[tokio::test]
    // async fn eve_spawn_test() {
    //     let eve = App::run(Model, Capabilities);
    //     let handle = eve.spawn(|eve| async { dbg!(42) });
    //     tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    // }
}
