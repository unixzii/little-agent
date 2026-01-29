//! A local fake model for testing purpose.

use std::error::Error as StdError;
use std::fmt::{self, Debug, Display, Formatter};
use std::future::ready;
use std::pin::Pin;
use std::task::{Context, Poll};

use little_agent_model::{
    ErrorKind, ModelProvider, ModelProviderError, ModelResponse,
    ModelResponseEvent,
};

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

pub struct TestModelResponse;

impl ModelResponse for TestModelResponse {
    type Error = crate::Error;

    fn poll_next_event(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Result<Option<ModelResponseEvent>, Self::Error>> {
        todo!()
    }
}

#[derive(Default)]
pub struct TestModelProvider;

impl ModelProvider for TestModelProvider {
    type Error = crate::Error;
    type Response = TestModelResponse;

    fn send_request(
        &self,
        _req: &little_agent_model::ModelRequest,
    ) -> impl Future<Output = Result<Self::Response, Self::Error>> + Send + 'static
    {
        let resp = TestModelResponse;
        ready(Ok(resp))
    }
}
