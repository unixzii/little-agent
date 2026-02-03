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

/// A wrapper around a model provider that maintains an execution
/// environment for the provider and provides a type-erased interface
/// for the other modules.
#[derive(Clone)]
pub struct ModelClient {
    handler_fn:
        Arc<dyn Fn(ModelRequest) -> BoxedSendRequestFuture + Send + Sync>,
}

impl ModelClient {
    #[inline]
    pub fn new<P: ModelProvider + 'static>(provider: P) -> Self {
        Self {
            handler_fn: Arc::new(move |req| {
                let fut = provider.send_request(&req);
                Box::pin(
                    async move {
                        trace!("got a request: {:?}", req);
                        let resp_or_err = fut.await;
                        handle_response::<P>(resp_or_err).await
                    }
                    .instrument(trace_span!("model client req")),
                )
            }),
        }
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
    ) -> Result<ModelClientResponse, Box<dyn ModelProviderError>> {
        (self.handler_fn)(req).await
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
    use little_agent_model::ModelMessage;
    use little_agent_test_model::{
        PresetEvent, PresetResponse, TestModelProvider,
    };

    use super::*;

    #[tokio::test]
    async fn test_send_request() {
        let mut model_provider = TestModelProvider::default();
        model_provider.add_user_turn();
        model_provider.add_assistant_turn(PresetResponse {
            events: vec![
                PresetEvent::MessageDelta("How ".to_owned()),
                PresetEvent::MessageDelta("are ".to_owned()),
                PresetEvent::MessageDelta("you?".to_owned()),
            ],
        });

        let model_client = ModelClient::new(model_provider);

        for _ in 0..3 {
            let resp = model_client
                .send_request(ModelRequest {
                    messages: vec![ModelMessage::User("Hi".to_owned())],
                    tools: vec![],
                })
                .await
                .unwrap();
            assert_eq!(resp.transcript, "How are you?");
            assert!(resp.opaque_msg.is_some());
        }
    }

    #[tokio::test]
    async fn test_error_handling() {
        let model_provider = TestModelProvider::default();
        let model_client = ModelClient::new(model_provider);
        let resp_or_err = model_client
            .send_request(ModelRequest {
                messages: vec![ModelMessage::User("Hi".to_owned())],
                tools: vec![],
            })
            .await;
        assert!(matches!(resp_or_err, Err(_)));
    }
}
