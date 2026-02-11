use std::future::poll_fn;
use std::pin::{Pin, pin};
use std::sync::Arc;

use little_agent_model::{
    ModelFinishReason, ModelProvider, ModelProviderError, ModelRequest,
    ModelResponse, ModelResponseEvent, OpaqueMessage, ToolCallRequest,
};
use tracing::Instrument;

type SendRequestResult =
    Result<ModelClientResponse, Box<dyn ModelProviderError>>;
type BoxedSendRequestFuture =
    Pin<Box<dyn Future<Output = SendRequestResult> + Send>>;
#[rustfmt::skip]
type HandlerFn = Arc<
    dyn Fn(ModelRequest, Box<dyn Fn(String) + Send + 'static>)
        -> BoxedSendRequestFuture + Send + Sync
>;

/// A wrapper around a model provider that maintains an execution
/// environment for the provider and provides a type-erased interface
/// for the other modules.
#[derive(Clone)]
pub struct ModelClient {
    handler_fn: HandlerFn,
}

impl ModelClient {
    #[inline]
    pub fn new<P: ModelProvider + 'static>(provider: P) -> Self {
        // We have to erase the type `P`, since `ModelClient` doesn't have a
        // generic parameter and we don't want it either.
        let handler_fn: HandlerFn = Arc::new(move |req, on_transcript| {
            let fut = provider.send_request(&req);
            Box::pin(
                async move {
                    trace!("got a request: {:?}", req);
                    let resp_or_err = fut.await;
                    handle_response::<P>(resp_or_err, on_transcript).await
                }
                .instrument(trace_span!("model client req")),
            )
        });
        Self { handler_fn }
    }

    /// Sends a request and returns the response.
    ///
    /// # Cancel safety
    ///
    /// This method is cancel safe. The response stops streaming further
    /// events when this operation is cancelled.
    #[inline]
    pub async fn send_request(
        &self,
        req: ModelRequest,
        on_transcript: impl Fn(String) + Send + 'static,
    ) -> Result<ModelClientResponse, Box<dyn ModelProviderError>> {
        (self.handler_fn)(req, Box::new(on_transcript)).await
    }
}

/// A completely received response from the model client.
#[derive(Clone, Debug)]
pub struct ModelClientResponse {
    pub transcript: String,
    pub opaque_msg: Option<OpaqueMessage>,
    /// Tool calls requested by the model.
    pub tool_calls: Vec<ToolCallRequest>,
    /// The reason the model finished generating.
    pub finish_reason: Option<ModelFinishReason>,
}

async fn handle_response<P: ModelProvider + 'static>(
    resp_or_err: Result<P::Response, P::Error>,
    on_transcript: Box<dyn Fn(String) + Send + 'static>,
) -> SendRequestResult {
    let resp = match resp_or_err {
        Ok(resp) => resp,
        Err(err) => {
            error!("got an error: {err:?}");
            // req.error_tx.send(Box::new(err)).ok();
            return Err(Box::new(err));
        }
    };

    let mut transcript = String::new();
    let opaque_msg;
    let mut tool_calls = Vec::new();
    let mut finish_reason = None;

    trace!("start receiving events");

    let mut pinned_resp = pin!(resp);
    loop {
        let event_or_err =
            poll_fn(|cx| pinned_resp.as_mut().poll_next_event(cx)).await;
        let event = match event_or_err {
            Ok(event) => event,
            Err(err) => {
                error!("got an error: {err:?}");
                return Err(Box::new(err));
            }
        };

        let Some(event) = event else {
            // The request has been handled gracefully without errors,
            // now try getting the opaque message for this response.
            opaque_msg = pinned_resp.make_opaque_message();
            break;
        };
        trace!("got an event: {event:?}");

        match event {
            ModelResponseEvent::MessageDelta(msg) => {
                transcript.push_str(&msg);
                on_transcript(msg);
            }
            ModelResponseEvent::ToolCall(req) => {
                tool_calls.push(req);
            }
            ModelResponseEvent::Completed(reason) => {
                finish_reason = Some(reason);
            }
        }
    }

    trace!("finished a request");

    Ok(ModelClientResponse {
        transcript,
        opaque_msg,
        tool_calls,
        finish_reason,
    })
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};

    use little_agent_model::ModelMessage;
    use little_agent_test_model::{
        PresetEvent, PresetResponse, TestModelProvider,
    };

    use super::*;

    #[tokio::test]
    async fn test_send_request() {
        let mut model_provider = TestModelProvider::default();
        model_provider.add_user_input_step();
        model_provider.add_assistant_response_step(
            PresetResponse::with_events([
                PresetEvent::MessageDelta("How ".to_owned()),
                PresetEvent::MessageDelta("are ".to_owned()),
                PresetEvent::MessageDelta("you?".to_owned()),
            ]),
        );

        let model_client = ModelClient::new(model_provider);

        for _ in 0..3 {
            let on_transcript_called = Arc::new(AtomicBool::new(false));
            let resp = model_client
                .send_request(
                    ModelRequest {
                        messages: vec![ModelMessage::User("Hi".to_owned())],
                        tools: vec![],
                    },
                    {
                        let on_transcript_called =
                            Arc::clone(&on_transcript_called);
                        move |_| {
                            on_transcript_called.store(true, Ordering::Relaxed);
                        }
                    },
                )
                .await
                .unwrap();
            assert_eq!(resp.transcript, "How are you?");
            assert!(resp.opaque_msg.is_some());
            assert!(on_transcript_called.load(Ordering::Relaxed));
        }
    }

    #[tokio::test]
    async fn test_error_handling() {
        let model_provider = TestModelProvider::default();
        let model_client = ModelClient::new(model_provider);
        let resp_or_err = model_client
            .send_request(
                ModelRequest {
                    messages: vec![ModelMessage::User("Hi".to_owned())],
                    tools: vec![],
                },
                |_| {},
            )
            .await;
        assert!(matches!(resp_or_err, Err(_)));
    }
}
