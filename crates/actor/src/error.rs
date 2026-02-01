use std::error::Error;
use std::fmt;

/// A type of error which can be returned whenever messages are sent to
/// an actor that has dead.
pub struct ActorDeadError;

impl fmt::Debug for ActorDeadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ActorDeadError").finish()
    }
}

impl fmt::Display for ActorDeadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        "the actor has dead".fmt(f)
    }
}

impl Error for ActorDeadError {}
