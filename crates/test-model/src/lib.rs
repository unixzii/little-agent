//! A local fake model for testing purpose.

mod preset;

use std::error::Error as StdError;
use std::fmt::{self, Debug, Display, Formatter};
use std::future::ready;
use std::pin::Pin;
use std::task::{Context, Poll, ready};
use std::time::Duration;

use little_agent_model::{
    ErrorKind, ModelFinishReason, ModelProvider, ModelProviderError,
    ModelRequest, ModelResponse, ModelResponseEvent, OpaqueMessage,
};
use tokio::time::{Sleep, sleep};

pub use preset::*;

#[derive(Debug)]
pub struct Error {
    #[allow(dead_code)]
    message: &'static str,
    kind: ErrorKind,
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Debug::fmt(self, f)
    }
}

impl StdError for Error {}

impl ModelProviderError for Error {
    #[inline]
    fn kind(&self) -> ErrorKind {
        self.kind
    }
}

pub struct TestModelResponse {
    provider: TestModelProvider,
    request: ModelRequest,
    event_idx: usize,
    sleep: Option<Pin<Box<Sleep>>>,
}

impl ModelResponse for TestModelResponse {
    type Error = crate::Error;

    fn poll_next_event(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Option<ModelResponseEvent>, Self::Error>> {
        let step_idx = self.request.messages.len();
        if step_idx >= self.provider.conversation_script.len() {
            return Poll::Ready(Err(Error {
                message: "no enough steps",
                kind: ErrorKind::RateLimitExceeded,
            }));
        }

        // SAFETY: This type does not require to be pinned.
        let this = unsafe { self.get_unchecked_mut() };

        let step = &this.provider.conversation_script[step_idx];
        let preset_events = match step {
            ConversationStep::UserInput => {
                return Poll::Ready(Err(Error {
                    message: "not an assistant response step",
                    kind: ErrorKind::Moderated,
                }));
            }
            ConversationStep::AssistantResponse(response) => &response.events,
        };

        if let Some(sleep) = &mut this.sleep {
            let sleep = sleep.as_mut();
            ready!(sleep.poll(cx));
            this.sleep = None;

            if this.event_idx < preset_events.len() {
                let event = match &preset_events[this.event_idx] {
                    PresetEvent::MessageDelta(msg) => {
                        ModelResponseEvent::MessageDelta(msg.clone())
                    }
                    PresetEvent::ToolCall(req) => {
                        ModelResponseEvent::ToolCall(req.clone())
                    }
                };
                this.event_idx += 1;
                return Poll::Ready(Ok(Some(event)));
            } else if this.event_idx == preset_events.len() {
                this.event_idx += 1;
                let has_tool_call = preset_events
                    .iter()
                    .any(|event| matches!(event, PresetEvent::ToolCall(_)));
                return Poll::Ready(Ok(Some(ModelResponseEvent::Completed(
                    if has_tool_call {
                        ModelFinishReason::ToolCalls
                    } else {
                        ModelFinishReason::Stop
                    },
                ))));
            } else {
                // In case this method is called after completion.
                return Poll::Ready(Ok(None));
            }
        }
        this.sleep = Some(Box::pin(sleep(
            this.provider.delay.unwrap_or(Duration::from_millis(1)),
        )));
        Pin::new(this).poll_next_event(cx)
    }

    fn make_opaque_message(&self) -> Option<OpaqueMessage> {
        let step_idx = self.request.messages.len();
        let id = format!("msg:{step_idx}");
        Some(OpaqueMessage::new(id.clone(), id))
    }
}

#[derive(Clone)]
enum ConversationStep {
    UserInput,
    AssistantResponse(PresetResponse),
}

/// A local fake model for testing purpose.
///
/// Before sending requests, you need to setup the conversation script, which
/// is how the model should respond to a request. The added steps will be
/// selected according to the history messages in your request. If there are no
/// enough steps in the script, an error will be returned.
///
/// # Note
///
/// This type is not optimized for production use, there are heavy memory
/// copies involved. You should only use it for testing.
#[derive(Clone, Default)]
pub struct TestModelProvider {
    conversation_script: Vec<ConversationStep>,
    delay: Option<Duration>,
}

impl TestModelProvider {
    #[inline]
    pub fn add_assistant_response_step(&mut self, preset: PresetResponse) {
        self.conversation_script
            .push(ConversationStep::AssistantResponse(preset));
    }

    #[inline]
    pub fn add_user_input_step(&mut self) {
        self.conversation_script.push(ConversationStep::UserInput);
    }

    #[inline]
    pub fn set_delay(&mut self, duration: Duration) {
        self.delay = Some(duration);
    }
}

impl ModelProvider for TestModelProvider {
    type Error = crate::Error;
    type Response = TestModelResponse;

    fn send_request(
        &self,
        req: &ModelRequest,
    ) -> impl Future<Output = Result<Self::Response, Self::Error>> + Send + 'static
    {
        let resp = TestModelResponse {
            provider: self.clone(),
            request: req.clone(),
            event_idx: 0,
            sleep: None,
        };
        ready(Ok(resp))
    }
}

#[cfg(test)]
mod tests {
    use std::future::poll_fn;
    use std::pin::pin;

    use little_agent_model::{
        ModelMessage, ModelRequest, ModelTool, OpaqueMessage, ToolCallRequest,
    };
    use serde_json::json;

    use super::*;

    async fn collect_response(
        resp: TestModelResponse,
    ) -> (String, Option<ToolCallRequest>, OpaqueMessage) {
        let mut resp = pin!(resp);
        let mut msg = String::new();
        let mut tool_call = None;
        loop {
            let event = poll_fn(|cx| resp.as_mut().poll_next_event(cx))
                .await
                .unwrap()
                .unwrap();
            match event {
                ModelResponseEvent::Completed(_) => break,
                ModelResponseEvent::MessageDelta(delta) => {
                    msg.push_str(&delta);
                }
                ModelResponseEvent::ToolCall(req) => tool_call = Some(req),
            }
        }
        (msg, tool_call, resp.make_opaque_message().unwrap())
    }

    #[tokio::test]
    async fn test_send_request() {
        let mut provider = TestModelProvider::default();
        provider.add_user_input_step();
        provider.add_assistant_response_step(PresetResponse {
            events: vec![
                PresetEvent::MessageDelta("Hello, ".to_owned()),
                PresetEvent::MessageDelta("world!".to_owned()),
            ],
        });
        provider.add_user_input_step();
        provider.add_assistant_response_step(PresetResponse {
            events: vec![
                PresetEvent::MessageDelta("Sure, ".to_owned()),
                PresetEvent::MessageDelta("let me take a ".to_owned()),
                PresetEvent::MessageDelta("look.".to_owned()),
                PresetEvent::ToolCall(ToolCallRequest {
                    id: "tool:1".to_owned(),
                    name: "read_file".to_owned(),
                    arguments: json!({ "filename": "todo.txt" }),
                }),
            ],
        });

        let mut req = ModelRequest {
            messages: vec![ModelMessage::User("Hi".to_owned())],
            tools: vec![ModelTool {
                name: "read_file".to_owned(),
                description: "Reads a file".to_owned(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "filename": {
                            "type": "string",
                            "description": "The name of the file to read"
                        }
                    }
                }),
            }],
        };
        let resp = provider.send_request(&req).await.unwrap();
        let (msg, _, opaque_msg) = collect_response(resp).await;
        assert_eq!(msg, "Hello, world!");

        req.messages.push(ModelMessage::Opaque(opaque_msg));
        req.messages
            .push(ModelMessage::User("Check my todo".to_owned()));
        let resp = provider.send_request(&req).await.unwrap();
        let (msg, tool_call, _) = collect_response(resp).await;
        assert_eq!(msg, "Sure, let me take a look.");
        let tool_call = tool_call.unwrap();
        assert_eq!(tool_call.name, "read_file");
        assert_eq!(tool_call.arguments, json!({ "filename": "todo.txt" }));
    }
}
