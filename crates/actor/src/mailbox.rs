use std::fmt::Debug;

use tokio::sync::{mpsc, watch};

use crate::{Actor, ActorDeadError};

/// Helper trait for handling boxed messages.
pub trait BoxMessage<S>: Send + Debug + 'static {
    fn handle_box(self: Box<Self>, state: &mut S, handle: &Actor<S>);
}

/// The message that an actor can handle.
pub trait Message<S>: BoxMessage<S> {
    /// Handles the message with mutable access to the actor's state.
    fn handle(self, state: &mut S, handle: &Actor<S>);
}

impl<S, M: Message<S>> BoxMessage<S> for M {
    #[inline]
    fn handle_box(self: Box<Self>, state: &mut S, handle: &Actor<S>) {
        (*self).handle(state, handle)
    }
}

impl<S, M: Message<S> + ?Sized> Message<S> for Box<M> {
    #[inline]
    fn handle(self, state: &mut S, handle: &Actor<S>) {
        self.handle_box(state, handle)
    }
}

pub struct MailboxParts<S> {
    pub mailbox: Mailbox<S>,
    pub msg_rx: mpsc::UnboundedReceiver<Box<dyn Message<S>>>,
    pub kill_rx: watch::Receiver<bool>,
}

pub struct Mailbox<S> {
    msg_tx: mpsc::UnboundedSender<Box<dyn Message<S>>>,
    kill_tx: watch::Sender<bool>,
}

impl<S: Send + Sync + 'static> Mailbox<S> {
    #[inline]
    pub fn new() -> MailboxParts<S> {
        let (msg_tx, msg_rx) = mpsc::unbounded_channel();
        let (kill_tx, kill_rx) = watch::channel(false);
        MailboxParts {
            mailbox: Mailbox { msg_tx, kill_tx },
            msg_rx,
            kill_rx,
        }
    }

    #[inline]
    pub fn send(&self, msg: Box<dyn Message<S>>) -> Result<(), ActorDeadError> {
        self.msg_tx.send(msg).map_err(|_| ActorDeadError)
    }

    #[inline]
    pub fn try_kill(&self) {
        self.kill_tx.send(true).ok();
    }
}
