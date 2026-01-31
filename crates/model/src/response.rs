use std::pin::Pin;
use std::task::{self, Poll};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::OpaqueMessage;
use crate::provider::ModelProviderError;

/// A response from the model provider.
pub trait ModelResponse: Sized + Send + 'static {
    /// The error type that may be returned by the provider.
    type Error: ModelProviderError;

    /// Attempts to pull out the next event from the response.
    ///
    /// # Return value
    ///
    /// There are several possible return values, each indicating a
    /// distinct response state:
    ///
    /// - `Poll::Pending` means that this response is still waiting for
    ///   the next event. Implementations will ensure that the current
    ///   task will be notified when the next event may be ready.
    /// - `Poll::Ready(Ok(Some(event)))` means the response has an event
    ///   to deliver, and may produce further events on subsequent
    ///   `poll_next_event` calls.
    /// - `Poll::Ready(Ok(None))` means the response has completed.
    /// - `Poll::Ready(Err(error))` means an error occurred while
    ///   processing the response.
    ///
    /// Calling this method after completion should always return `None`.
    fn poll_next_event(
        self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> Poll<Result<Option<ModelResponseEvent>, Self::Error>>;

    /// Makes an [`OpaqueMessage`] that represents the message in this
    /// response.
    ///
    /// You should call this method after polling all events from this
    /// response, and the implementations should always return the same
    /// message for one response.
    ///
    /// Calling this method when the response is still producing events
    /// should be avoided, since the message may be incomplete.
    fn make_opaque_message(&self) -> Option<OpaqueMessage> {
        None
    }
}

/// The reason why a model response has finished.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ModelFinishReason {
    /// The model needs to call a tool.
    ToolCalls,
    /// The model has finished generating text.
    Stop,
}

/// Describes a tool call request from the model.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ToolCallRequest {
    /// The unique identifier for the tool call request.
    pub id: String,
    /// The name of the tool to call.
    pub name: String,
    /// The argument pairs to pass to the function.
    pub arguments: Vec<(String, Value)>,
}

/// The event from a model response.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ModelResponseEvent {
    /// The response has been completed.
    Completed(ModelFinishReason),
    /// Received a message delta.
    MessageDelta(String),
    /// Received a tool call request.
    ToolCall(ToolCallRequest),
}
