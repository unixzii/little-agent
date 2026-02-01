//! A lightweight actor framework.

#![deny(missing_docs)]

#[macro_use]
extern crate tracing;

mod error;
mod handle;
mod macros;
mod mailbox;
mod scheduler;

pub use error::ActorDeadError;
pub use handle::Actor;
pub use mailbox::Message;

#[cfg(test)]
mod tests {
    use tokio::sync::oneshot;

    use super::*;

    define_actor! {
        /// This is a test actor.
        #[wrapper_type(TestActor)]
        #[derive(Default)]
        struct TestActorState {
            value: u32,
        }
    }

    #[derive(Debug)]
    struct AddMessage(u32);

    impl Message<TestActorState> for AddMessage {
        fn handle(
            self,
            state: &mut TestActorState,
            _handle: &Actor<TestActorState>,
        ) {
            state.value += self.0;
        }
    }

    #[derive(Debug)]
    struct GetMessage(oneshot::Sender<u32>);

    impl Message<TestActorState> for GetMessage {
        fn handle(
            self,
            state: &mut TestActorState,
            _handle: &Actor<TestActorState>,
        ) {
            self.0.send(state.value).unwrap();
        }
    }

    #[tokio::test]
    async fn test_send_message() {
        let actor = TestActor::spawn(TestActorState::default(), None);
        actor.handle().send(AddMessage(42)).unwrap();

        let (tx, rx) = oneshot::channel();
        actor.handle().send(GetMessage(tx)).unwrap();
        assert_eq!(rx.await.unwrap(), 42);
    }
}
