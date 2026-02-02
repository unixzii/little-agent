use std::future::poll_fn;
use std::pin::pin;

use little_agent_model::{
    ModelFinishReason, ModelProvider, ModelProviderError, ModelRequest,
    ModelResponse, ModelResponseEvent, OpaqueMessage, ToolCallRequest,
};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tracing::Instrument;

/// A wrapper around a model provider that maintains an execution
/// environment for the provider and provides a type-erased interface
/// for the other modules.
///
/// When dropped, the client cancels all its in-flight requests.
pub struct ModelClient {
    req_tx: mpsc::UnboundedSender<ModelClientRequest>,
    client_task: JoinHandle<()>,
}

impl ModelClient {
    #[inline]
    pub fn new<P: ModelProvider + 'static>(provider: P) -> Self {
        let (req_tx, req_rx) = mpsc::unbounded_channel();
        let client_task = tokio::spawn(async move {
            serve_client(provider, req_rx)
                .instrument(debug_span!("model client"))
                .await;
        });
        Self {
            req_tx,
            client_task,
        }
    }

    /// Sends a request and returns the response.
    ///
    /// # Cancel safety
    ///
    /// This method is cancel safe. The response stops streaming further
    /// events when this operation is cancelled.
    pub async fn send_request(
        &self,
        req: ModelRequest,
    ) -> Result<ModelClientResponse, Box<dyn ModelProviderError>> {
        let (resp_event_tx, mut resp_event_rx) = mpsc::unbounded_channel();
        let (error_tx, mut error_rx) = mpsc::unbounded_channel();
        let (opaque_msg_tx, mut opaque_msg_rx) = mpsc::unbounded_channel();

        let client_req = ModelClientRequest {
            model_request: req,
            resp_event_tx,
            error_tx,
            opaque_msg_tx,
        };
        self.req_tx
            .send(client_req)
            .expect("task has been dropped too early");

        // Try collecting the events first.
        let mut transcript = String::new();
        let mut tool_calls = Vec::new();
        let mut finish_reason = None;
        loop {
            let Some(resp_event) = resp_event_rx.recv().await else {
                break;
            };
            match resp_event {
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

        // We are now out of the event receiving loop, check if there're
        // any errors before concluding the request.
        if let Some(err) = error_rx.recv().await {
            return Err(err);
        }

        // The request has been handled gracefully without errors. Now
        // try receiving the opaque message.
        let opaque_msg = opaque_msg_rx.recv().await;

        Ok(ModelClientResponse {
            transcript,
            opaque_msg,
            tool_calls,
            finish_reason,
        })
    }
}

impl Drop for ModelClient {
    fn drop(&mut self) {
        self.client_task.abort();
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

struct ModelClientRequest {
    model_request: ModelRequest,
    resp_event_tx: mpsc::UnboundedSender<ModelResponseEvent>,
    error_tx: mpsc::UnboundedSender<Box<dyn ModelProviderError>>,
    opaque_msg_tx: mpsc::UnboundedSender<OpaqueMessage>,
}

#[inline]
async fn serve_client<P: ModelProvider + 'static>(
    provider: P,
    mut req_rx: mpsc::UnboundedReceiver<ModelClientRequest>,
) {
    // We don't want to handle parallel requests in one agent, so any new
    // requests will be enqueued and handled sequentially.
    while let Some(req) = req_rx.recv().await {
        handle_client_request(req, &provider)
            .instrument(trace_span!("request"))
            .await;
    }
    debug!("will terminate");
}

async fn handle_client_request<P: ModelProvider + 'static>(
    req: ModelClientRequest,
    provider: &P,
) {
    trace!("got a request: {:?}", req.model_request);
    let resp_or_err = provider.send_request(&req.model_request).await;
    let resp = match resp_or_err {
        Ok(resp) => resp,
        Err(err) => {
            error!("got an error: {err:?}");
            req.error_tx.send(Box::new(err)).ok();
            return;
        }
    };

    trace!("start receiving events");
    let mut pinned_resp = pin!(resp);
    loop {
        let event_or_err =
            poll_fn(|cx| pinned_resp.as_mut().poll_next_event(cx)).await;
        let event = match event_or_err {
            Ok(event) => event,
            Err(err) => {
                error!("got an error: {err:?}");
                req.error_tx.send(Box::new(err)).ok();
                break;
            }
        };
        let Some(event) = event else {
            // Try getting the opaque message for this response.
            let opaque_msg = pinned_resp.make_opaque_message();
            if let Some(opaque_msg) = opaque_msg {
                req.opaque_msg_tx.send(opaque_msg).ok();
            }
            break;
        };
        trace!("got an event: {event:?}");
        if req.resp_event_tx.send(event).is_err() {
            trace!("cancelled");
            break;
        }
    }
    trace!("finished a request");
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
