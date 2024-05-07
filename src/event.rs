use std::{future::Future, ops::Deref, sync::Arc};

use parking_lot::RwLock;
use uuid::Uuid;

use crate::eve::{DispatchError, Eve, Eventide};

pub struct EffectContext<A: Eventide> {
    pub id: Uuid,
    pub(crate) incoming_tx: tokio::sync::mpsc::UnboundedSender<EventMsg<A>>,
    pub(crate) outgoing_tx: tokio::sync::mpsc::UnboundedSender<EffectMsg<A>>,
    pub(crate) model: Arc<RwLock<A::Model>>,
    pub caps: Arc<A::Capabilities>,
}

impl<A: Eventide> EffectContext<A> {
    pub fn dispatch<T: Into<Message<A>>>(&self, msg: T) -> Result<(), DispatchError<A>> {
        let mut msg = msg.into();
        msg.set_parent_id(self.id);
        match msg {
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

    pub fn with_model<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&A::Model) -> R,
    {
        f(&*self.model.read())
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

impl<A> From<(&Eve<A>, Uuid)> for EffectContext<A>
where
    A: Eventide,
{
    fn from((eve, id): (&Eve<A>, Uuid)) -> Self {
        Self {
            id,
            incoming_tx: eve.incoming_tx.clone(),
            outgoing_tx: eve.outgoing_tx.clone(),
            model: Arc::clone(&eve.model),
            caps: Arc::clone(&eve.capabilities),
        }
    }
}

pub trait EventHandler<A>
where
    A: Eventide,
{
    fn handle(self, model: &mut A::Model) -> Option<Vec<Message<A>>>;
}

pub trait EffectHandler<A>
where
    A: Eventide,
{
    fn handle(self, ctx: EffectContext<A>) -> impl Future<Output = Option<Vec<Message<A>>>> + Send;
}

pub struct EventMsg<A: Eventide> {
    pub id: Uuid,
    pub parent: Option<Uuid>,
    pub src: Option<Uuid>,
    pub(crate) event: A::Event,
}

impl<A: Eventide> EventMsg<A> {
    pub fn id(&self) -> Uuid {
        self.id
    }
    pub fn parent_id(&self) -> Option<Uuid> {
        self.parent
    }
    pub fn src_id(&self) -> Option<Uuid> {
        self.src
    }
}

pub struct EffectMsg<A: Eventide> {
    pub id: Uuid,
    pub parent: Option<Uuid>,
    pub src: Option<Uuid>,
    pub(crate) effect: A::Effect,
}

impl<A: Eventide> EffectMsg<A> {
    pub fn id(&self) -> Uuid {
        self.id
    }
    pub fn parent_id(&self) -> Option<Uuid> {
        self.parent
    }
    pub fn src_id(&self) -> Option<Uuid> {
        self.src
    }
}

pub enum Message<A>
where
    A: Eventide,
{
    EventMsg(EventMsg<A>),
    EffectMsg(EffectMsg<A>),
}

impl<A: Eventide> Message<A> {
    pub fn event(event: A::Event) -> Self {
        Self::EventMsg(EventMsg {
            id: Uuid::now_v7(),
            parent: None,
            src: None,
            event,
        })
    }

    pub fn effect(effect: A::Effect) -> Self {
        Self::EffectMsg(EffectMsg {
            id: Uuid::now_v7(),
            parent: None,
            src: None,
            effect,
        })
    }

    pub(crate) fn set_parent_id(&mut self, parent: Uuid) {
        match self {
            Self::EventMsg(EventMsg { parent: p, .. })
            | Self::EffectMsg(EffectMsg { parent: p, .. }) => {
                *p = Some(parent);
            }
        };
    }

    pub(crate) fn set_src_id(&mut self, src: Uuid) {
        match self {
            Self::EventMsg(EventMsg { src: s, .. }) | Self::EffectMsg(EffectMsg { src: s, .. }) => {
                *s = Some(src);
            }
        };
    }

    pub fn id(&self) -> Uuid {
        match self {
            Self::EventMsg(EventMsg { id, .. }) | Self::EffectMsg(EffectMsg { id, .. }) => *id,
        }
    }

    pub fn parent_id(&self) -> Option<Uuid> {
        match self {
            Self::EventMsg(EventMsg { parent, .. }) | Self::EffectMsg(EffectMsg { parent, .. }) => {
                *parent
            }
        }
    }

    pub fn src_id(&self) -> Option<Uuid> {
        match self {
            Self::EventMsg(EventMsg { src, .. }) | Self::EffectMsg(EffectMsg { src, .. }) => *src,
        }
    }
}

impl<A: Eventide> IntoIterator for Message<A> {
    type Item = Message<A>;
    type IntoIter = std::iter::Once<Message<A>>;

    fn into_iter(self) -> Self::IntoIter {
        std::iter::once(self)
    }
}

impl<A: Eventide> From<Message<A>> for Vec<Message<A>> {
    fn from(msg: Message<A>) -> Self {
        vec![msg]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eve::Eventide;

    struct TestEventide;

    impl Eventide for TestEventide {
        type Model = ();
        type Capabilities = ();
        type Event = TestEvent;
        type Effect = TestEffect;
    }

    enum TestEvent {
        Event1,
        Event2,
    }

    impl From<TestEvent> for Message<TestEventide> {
        fn from(event: TestEvent) -> Self {
            Message::event(event)
        }
    }

    impl std::iter::IntoIterator for TestEvent {
        type Item = Message<TestEventide>;
        type IntoIter = std::iter::Once<Message<TestEventide>>;
        fn into_iter(self) -> Self::IntoIter {
            std::iter::once(Message::event(self))
        }
    }

    impl EventHandler<TestEventide> for TestEvent {
        fn handle(
            self,
            model: &mut <TestEventide as Eventide>::Model,
        ) -> Option<Vec<Message<TestEventide>>> {
            None
        }
    }

    enum TestEffect {
        Effect1,
        Effect2,
    }

    impl EffectHandler<TestEventide> for TestEffect {
        async fn handle(
            self,
            ctx: EffectContext<TestEventide>,
        ) -> Option<Vec<Message<TestEventide>>> {
            None
        }
    }

    #[test]
    fn test_event_msg_construction() {
        let event_msg: EventMsg<TestEventide> = EventMsg {
            id: Uuid::nil(),
            parent: None,
            src: None,
            event: TestEvent::Event1,
        };

        assert!(matches!(event_msg.event, TestEvent::Event1));
    }

    #[test]
    fn test_effect_msg_construction() {
        let effect_msg: EffectMsg<TestEventide> = EffectMsg {
            id: Uuid::nil(),
            parent: None,
            src: None,
            effect: TestEffect::Effect1,
        };

        assert!(matches!(effect_msg.effect, TestEffect::Effect1));
    }

    #[test]
    fn test_message_event_construction() {
        let message: Message<TestEventide> = Message::event(TestEvent::Event1);

        assert!(matches!(message, Message::EventMsg(_)));
        if let Message::EventMsg(event_msg) = message {
            assert!(matches!(event_msg.event, TestEvent::Event1));
        }
    }

    #[test]
    fn test_message_effect_construction() {
        let message: Message<TestEventide> = Message::effect(TestEffect::Effect1);

        assert!(matches!(message, Message::EffectMsg(_)));
        if let Message::EffectMsg(effect_msg) = message {
            assert!(matches!(effect_msg.effect, TestEffect::Effect1));
        }
    }
}
