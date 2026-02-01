use std::sync::Arc;

use tracing::Instrument;

use crate::mailbox::{Mailbox, MailboxParts};
use crate::scheduler::run_actor;
use crate::{ActorDeadError, Message};

/// Handle to an actor.
pub struct Actor<S> {
    mailbox: Arc<Mailbox<S>>,
}

impl<S: Send + Sync + 'static> Actor<S> {
    /// Spawn a new actor with the specified state and an optional label.
    ///
    /// Typically, you should call use this method directly, but rather
    /// use [`crate::define_actor`] macro to define your actor type and
    /// then call `spawn` method on that type.
    pub fn spawn(state: S, label: Option<&str>) -> Self {
        let MailboxParts {
            mailbox,
            msg_rx,
            kill_rx,
        } = Mailbox::new();
        let mailbox = Arc::new(mailbox);
        tokio::spawn(
            run_actor(Arc::downgrade(&mailbox), state, msg_rx, kill_rx)
                .instrument(trace_span!("actor", label = label)),
        );
        Self { mailbox }
    }

    #[inline]
    pub(crate) fn from_mailbox(mailbox: Arc<Mailbox<S>>) -> Self {
        Self { mailbox }
    }

    /// Sends a message to the actor.
    #[inline]
    pub fn send<M: Message<S> + 'static>(
        &self,
        msg: M,
    ) -> Result<(), ActorDeadError> {
        self.mailbox.send(Box::new(msg))
    }

    /// Attempts to kill the actor.
    ///
    /// The actor is not guaranteed to be killed immediately, but it
    /// will stop handling further messages and quit soon.
    #[inline]
    pub fn try_kill(&self) {
        self.mailbox.try_kill();
    }
}

impl<S> Clone for Actor<S> {
    #[inline]
    fn clone(&self) -> Self {
        Self {
            mailbox: Arc::clone(&self.mailbox),
        }
    }
}
