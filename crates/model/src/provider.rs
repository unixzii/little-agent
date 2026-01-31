use std::error::Error;

use crate::error::ErrorKind;
use crate::request::ModelRequest;
use crate::response::ModelResponse;

/// The error type for a model provider.
pub trait ModelProviderError: Error + Send + Sync + 'static {
    /// Returns the kind of this error.
    fn kind(&self) -> ErrorKind;
}

/// A type that represents a model provider, which is an entry for getting
/// model information, sampling requests, etc.
///
/// Once the provider is created, it should behave like a stateless object.
/// It can still have internal state, but callers should not rely on it,
/// and the provider should be prepared for being dropped anytime.
pub trait ModelProvider: Send + Sync {
    /// The error type that may be returned by the provider.
    type Error: ModelProviderError;

    /// The response type for this provider.
    type Response: ModelResponse<Error = Self::Error>;

    /// Sends a request to the model.
    fn send_request(
        &self,
        req: &ModelRequest,
    ) -> impl Future<Output = Result<Self::Response, Self::Error>> + Send + 'static;
}
