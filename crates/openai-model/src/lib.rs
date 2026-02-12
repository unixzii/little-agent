//! A model provider for OpenAI-compatible APIs.

#[macro_use]
extern crate tracing;

mod config;
mod io;
mod proto;
mod response;

use std::error::Error as StdError;
use std::fmt::{self, Display};
use std::sync::Arc;

use little_agent_model::{
    ErrorKind, ModelProvider, ModelProviderError, ModelRequest,
};
use mime::Mime;
use reqwest::{Client, Response, header};

pub use config::{OpenAIConfig, OpenAIConfigBuilder};
use io::{Chunks, Sse};
use response::OpenAIResponse;

/// Error type for [`OpenAIProvider`].
#[derive(Debug)]
pub struct Error {
    message: String,
    kind: ErrorKind,
}

impl Error {
    fn new(message: impl Into<String>, kind: ErrorKind) -> Self {
        Self {
            message: message.into(),
            kind,
        }
    }

    /// Returns the error message.
    #[inline]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl StdError for Error {}

impl ModelProviderError for Error {
    #[inline]
    fn kind(&self) -> ErrorKind {
        self.kind
    }
}

/// OpenAI-compatible model provider.
#[derive(Clone, Debug)]
pub struct OpenAIProvider {
    client: Client,
    config: Arc<OpenAIConfig>,
}

impl OpenAIProvider {
    /// Creates a new `OpenAIProvider` with the given configuration.
    #[inline]
    pub fn new(config: OpenAIConfig) -> Self {
        Self {
            client: Client::new(),
            config: Arc::new(config),
        }
    }
}

impl ModelProvider for OpenAIProvider {
    type Error = Error;
    type Response = OpenAIResponse;

    fn send_request(
        &self,
        req: &ModelRequest,
    ) -> impl Future<Output = Result<Self::Response, Self::Error>> + Send + 'static
    {
        let openai_req = proto::create_request(req, &self.config);
        let resp_fut = self
            .client
            .post(format!("{}{}", self.config.base_url, "/chat/completions"))
            .header(
                header::AUTHORIZATION,
                format!("Bearer {}", self.config.api_key),
            )
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::ACCEPT, "text/event-stream")
            .json(&openai_req)
            .send();

        async move {
            let resp = match resp_fut.await.and_then(Response::error_for_status)
            {
                Ok(resp) => resp,
                Err(err) => {
                    return Err(Error::new(format!("{err}"), ErrorKind::Other));
                }
            };

            let content_type = resp
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok());
            let is_valid_content_type = content_type
                .and_then(|v| v.parse().ok())
                .map(|m: Mime| m.subtype().as_str() == "text/event-stream")
                .unwrap_or(false);
            if is_valid_content_type {
                return Err(Error::new(
                    format!("Unexpected content type: {content_type:?}"),
                    ErrorKind::Other,
                ));
            }

            // Here we got a successful response.
            let chunks = Chunks::from_response(resp);
            let sse = Sse::new(chunks);
            Ok(OpenAIResponse::from_sse(sse))
        }
    }
}
