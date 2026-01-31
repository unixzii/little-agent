use std::collections::VecDeque;
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::future::ready;
use std::pin::Pin;
use std::task::{self, Poll, ready};
use std::time::Duration;

use little_agent_model::{
    ErrorKind, ModelMessage, ModelProvider, ModelProviderError, ModelRequest,
    ModelResponse, ModelResponseEvent,
};
use tokio::time::{Sleep, sleep};

#[derive(Debug)]
struct FakeModelProviderError(ErrorKind);

impl Display for FakeModelProviderError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl Error for FakeModelProviderError {}

impl ModelProviderError for FakeModelProviderError {
    fn kind(&self) -> ErrorKind {
        self.0
    }
}

#[derive(Debug)]
struct FakeModelResponse {
    fake_items: VecDeque<String>,
    sleep: Option<Pin<Box<Sleep>>>,
}

impl FakeModelResponse {
    fn new(input: &str) -> Self {
        let fake_items = format!("You said {}", input)
            .split(" ")
            .map(ToString::to_string)
            .collect();
        Self {
            fake_items,
            sleep: None,
        }
    }
}

impl ModelResponse for FakeModelResponse {
    type Error = FakeModelProviderError;

    fn poll_next_event(
        self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> Poll<Result<Option<ModelResponseEvent>, Self::Error>> {
        // SAFETY: This type does not require to be pinned.
        let this = unsafe { self.get_unchecked_mut() };
        if let Some(sleep) = &mut this.sleep {
            let sleep = sleep.as_mut();
            ready!(sleep.poll(cx));
            this.sleep = None;

            if let Some(mut this_item) = this.fake_items.pop_front() {
                let need_space = !this.fake_items.is_empty();
                if need_space {
                    this_item.push(' ');
                }
                return Poll::Ready(Ok(Some(
                    ModelResponseEvent::MessageDelta(this_item),
                )));
            }

            return Poll::Ready(Ok(None));
        }
        this.sleep = Some(Box::pin(sleep(Duration::from_millis(1))));
        Pin::new(this).poll_next_event(cx)
    }
}

struct FakeModelProvider;

impl ModelProvider for FakeModelProvider {
    type Error = FakeModelProviderError;
    type Response = FakeModelResponse;

    fn send_request(
        &self,
        req: &ModelRequest,
    ) -> impl Future<Output = Result<Self::Response, Self::Error>> + Send + 'static
    {
        let result = 'blk: {
            if req.messages.is_empty() {
                break 'blk Err(FakeModelProviderError(ErrorKind::Other));
            }

            let content = req.messages.first().map(|msg| match &msg {
                ModelMessage::User(text) => text.as_str(),
                _ => unreachable!("unexpected message: {msg:?}"),
            });

            Ok(FakeModelResponse::new(content.unwrap_or("")))
        };
        ready(result)
    }
}

mod tests {
    use std::future::poll_fn;

    use super::*;

    #[tokio::test]
    async fn test_completion() {
        let provider = FakeModelProvider;
        let req = ModelRequest {
            messages: vec![ModelMessage::User("Good morning".to_string())],
            tools: vec![],
        };
        let mut resp = provider.send_request(&req).await.unwrap();

        let mut resp_message = String::new();
        loop {
            let resp_fut =
                poll_fn(|cx| Pin::new(&mut resp).poll_next_event(cx));
            match resp_fut.await {
                Ok(Some(event)) => match event {
                    ModelResponseEvent::MessageDelta(delta) => {
                        resp_message.push_str(&delta);
                    }
                    ModelResponseEvent::Completed(_) => {
                        break;
                    }
                    _ => unreachable!("unexpected event: {event:?}"),
                },
                Ok(None) => break,
                Err(err) => unreachable!("unexpected error: {err:?}"),
            }
        }

        assert_eq!(resp_message, "You said Good morning");
    }

    #[tokio::test]
    async fn test_error() {
        let provider = FakeModelProvider;
        let req = ModelRequest {
            messages: vec![],
            tools: vec![],
        };
        let result = provider.send_request(&req).await;
        let err = result.unwrap_err();
        assert_eq!(err.kind(), ErrorKind::Other);
    }
}
