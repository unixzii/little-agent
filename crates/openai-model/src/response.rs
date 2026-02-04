use std::collections::VecDeque;
use std::pin::Pin;
use std::task::{Context, Poll, ready};

use little_agent_model::{
    ErrorKind, ModelFinishReason, ModelResponse, ModelResponseEvent,
    OpaqueMessage, ToolCallRequest,
};
use pin_project_lite::pin_project;
use serde_json::Value;

use crate::Error;
use crate::io::Sse;
use crate::proto::{ChatCompletionChunk, Message, ToolCall};

struct PartialState {
    sse: Sse,
    id: Option<String>,
    content: String,
    reasoning_content: Option<String>,
    tool_calls: Vec<ToolCall>,
    // This field records the index of the tool calls that are generated but not
    // yet sent to the model user. When calling `poll_next_event`, the response
    // will return the pending tool calls.
    pending_tool_call_idx: VecDeque<usize>,
    // This field will be cleared after the response returns the complete event.
    pending_finish_reason: Option<ModelFinishReason>,
}

impl PartialState {
    #[inline]
    fn finish(self) -> Option<(String, Message)> {
        Some((
            self.id?,
            Message::Assistant {
                content: Some(self.content),
                tool_calls: if self.tool_calls.is_empty() {
                    None
                } else {
                    Some(self.tool_calls)
                },
                reasoning_content: self.reasoning_content,
            },
        ))
    }
}

type PinnedFuture<T> = Pin<Box<dyn Future<Output = T> + Send>>;
type NextEvent = Result<(Option<ModelResponseEvent>, PartialState), Error>;

pin_project! {
    pub struct OpenAIResponse {
        next_event_fut: Option<PinnedFuture<NextEvent>>,
        full_msg: Option<(String, Message)>,
    }
}

impl OpenAIResponse {
    #[inline]
    pub fn from_sse(sse: Sse) -> Self {
        let partial_state = PartialState {
            sse,
            id: None,
            content: Default::default(),
            reasoning_content: Default::default(),
            tool_calls: Default::default(),
            pending_tool_call_idx: Default::default(),
            pending_finish_reason: Default::default(),
        };
        let next_event_fut = async move { next_event(partial_state).await };
        Self {
            next_event_fut: Some(Box::pin(next_event_fut)),
            full_msg: None,
        }
    }
}

impl ModelResponse for OpenAIResponse {
    type Error = crate::Error;

    fn poll_next_event(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Option<ModelResponseEvent>, Self::Error>> {
        let this = self.project();
        let Some(next_event_fut) = this.next_event_fut else {
            // The stream has been exhausted, actually this should be an error.
            return Poll::Ready(Ok(None));
        };
        let (event, partial_state) =
            match ready!(next_event_fut.as_mut().poll(cx)) {
                Ok((Some(event), partial_state)) => (event, partial_state),
                Ok((None, partial_state)) => {
                    *this.next_event_fut = None;
                    *this.full_msg = partial_state.finish();
                    return Poll::Ready(Ok(None));
                }
                Err(err) => {
                    *this.next_event_fut = None;
                    return Poll::Ready(Err(err));
                }
            };

        // The stream may still have more data to pull, create a new future for
        // the next event.
        let next_event_fut = async move { next_event(partial_state).await };
        *this.next_event_fut = Some(Box::pin(next_event_fut));

        Poll::Ready(Ok(Some(event)))
    }

    fn make_opaque_message(&self) -> Option<OpaqueMessage> {
        self.full_msg
            .as_ref()
            .map(|(id, msg)| OpaqueMessage::new(id, msg.clone()))
    }
}

async fn next_event(
    mut partial_state: PartialState,
) -> Result<(Option<ModelResponseEvent>, PartialState), Error> {
    let sse = &mut partial_state.sse;
    let mut message_delta = None;

    loop {
        let sse_event = match sse.next_event().await {
            Ok(Some(event)) => event,
            Ok(None) => break,
            Err(err) => {
                return Err(Error::new(format!("{err:?}"), ErrorKind::Other));
            }
        };
        trace!("got sse event: {sse_event}");
        if sse_event == "[DONE]" {
            break;
        }

        let mut chunk = serde_json::from_str::<ChatCompletionChunk>(&sse_event)
            .map_err(|err| Error::new(format!("{err}"), ErrorKind::Other))?;
        if partial_state.id.get_or_insert_with(|| chunk.id.clone()) != &chunk.id
        {
            return Err(Error::new("chunk id mismatch", ErrorKind::Other));
        };

        let Some(choice) = chunk.choices.pop() else {
            break;
        };

        if let Some(finish_reason) = choice.finish_reason {
            let finish_reason = if finish_reason == "tool_calls" {
                ModelFinishReason::ToolCalls
            } else {
                ModelFinishReason::Stop
            };
            partial_state.pending_finish_reason = Some(finish_reason);
            break;
        }

        if let Some(content) = choice.delta.content {
            partial_state.content.push_str(&content);
            message_delta = Some(content.to_owned());
        }
        if let Some(reasoning_content) = &choice.delta.reasoning_content {
            partial_state
                .reasoning_content
                .get_or_insert_default()
                .push_str(reasoning_content);
        }
        if let Some(tool_calls) = choice.delta.tool_calls {
            for tool_call in tool_calls {
                let Some(partial_tool_call) = partial_state
                    .tool_calls
                    .iter_mut()
                    .find(|t| t.index == tool_call.index)
                else {
                    partial_state
                        .pending_tool_call_idx
                        .push_back(partial_state.tool_calls.len());
                    partial_state.tool_calls.push(tool_call);
                    continue;
                };
                // Patch the partial tool call.
                if let Some(id) = tool_call.id {
                    partial_tool_call.id.get_or_insert_default().push_str(&id);
                }
                if let Some(ty) = tool_call.r#type {
                    partial_tool_call
                        .r#type
                        .get_or_insert_default()
                        .push_str(&ty);
                }
                if let Some(function) = tool_call.function {
                    match partial_tool_call.function {
                        Some(ref mut partial_func) => {
                            if let Some(name) = function.name {
                                partial_func
                                    .name
                                    .get_or_insert_default()
                                    .push_str(&name);
                            }
                            if let Some(parameters) = function.arguments {
                                partial_func
                                    .arguments
                                    .get_or_insert_default()
                                    .push_str(&parameters);
                            }
                        }
                        None => partial_tool_call.function = Some(function),
                    }
                }
            }
        }

        if message_delta.is_some() {
            break;
        }
    }

    // The order of events are important. Always emit message delta first, then
    // emit pending tool calls, and finally emit pending finish reason if any.

    if let Some(message_delta) = message_delta {
        return Ok((
            Some(ModelResponseEvent::MessageDelta(message_delta)),
            partial_state,
        ));
    }

    if let Some(idx) = partial_state.pending_tool_call_idx.pop_front() {
        let tool_call = &partial_state.tool_calls[idx];
        let id = tool_call.id.clone().unwrap_or_default();
        let name = tool_call
            .function
            .as_ref()
            .and_then(|f| f.name.clone())
            .unwrap_or_default();
        let arguments = tool_call
            .function
            .as_ref()
            .and_then(|f| f.arguments.as_deref())
            .and_then(|args| serde_json::from_str::<Value>(args).ok())
            .unwrap_or_default();
        return Ok((
            Some(ModelResponseEvent::ToolCall(ToolCallRequest {
                id,
                name,
                arguments,
            })),
            partial_state,
        ));
    }

    if let Some(finish_reason) = partial_state.pending_finish_reason.take() {
        return Ok((
            Some(ModelResponseEvent::Completed(finish_reason)),
            partial_state,
        ));
    }

    Ok((None, partial_state))
}

#[cfg(test)]
mod tests {
    use std::future::poll_fn;
    use std::pin::pin;

    use bytes::Bytes;

    use super::*;
    use crate::Chunks;

    #[tokio::test]
    async fn test_simple_events() {
        let chunks = Chunks::from_vec_deque(
            vec![Bytes::from_static(include_bytes!(
                "../fixtures/test_response.txt"
            ))]
            .into(),
        );
        let mut tool_call_count = 0;
        let sse = Sse::new(chunks);
        let mut resp = pin!(OpenAIResponse::from_sse(sse));
        loop {
            let Some(event) = poll_fn(|cx| resp.as_mut().poll_next_event(cx))
                .await
                .unwrap()
            else {
                break;
            };
            if let ModelResponseEvent::ToolCall(_) = event {
                tool_call_count += 1;
            }
            if let ModelResponseEvent::Completed(reason) = event {
                assert_eq!(tool_call_count, 2);
                assert_eq!(reason, ModelFinishReason::ToolCalls);
            }
        }
        let full_msg = resp.make_opaque_message().unwrap();
        let full_msg: &Message = full_msg.to_raw().unwrap();
        assert!(matches!(full_msg, Message::Assistant { .. }));
    }
}
