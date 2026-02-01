use std::sync::Weak;

use tokio::select;
use tokio::sync::{mpsc, watch};

use crate::mailbox::Mailbox;
use crate::{Actor, Message};

#[inline]
pub async fn run_actor<S: Send + Sync + 'static>(
    mailbox: Weak<Mailbox<S>>,
    mut state: S,
    mut msg_rx: mpsc::UnboundedReceiver<Box<dyn Message<S>>>,
    mut kill_rx: watch::Receiver<bool>,
) {
    debug!("started");
    loop {
        let msg = select! {
            biased;

            _ = kill_rx.changed() => {
                break;
            }
            msg = msg_rx.recv() => {
                let Some(msg) = msg else {
                    break;
                };
                msg
            }
        };
        trace!("received message: {msg:?}");

        {
            let Some(mailbox) = mailbox.upgrade() else {
                warn!("last mailbox has been dropped, discard the message");
                break;
            };

            let proc_span = trace_span!("proc msg");
            proc_span.in_scope(|| {
                msg.handle(&mut state, &Actor::from_mailbox(mailbox));
                trace!("finished");
            });
        }
    }
    debug!("will terminate");
}
