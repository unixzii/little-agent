//! Tool call supports.

mod error;
mod executor;

use std::error::Error as StdError;
use std::pin::Pin;

use little_agent_model::ModelTool;
use serde_json::Value;

pub use error::{Error, ErrorKind};
pub(crate) use executor::Executor;

/// The result of a tool call.
pub type ToolResult = Result<String, Error>;

/// A tool that can be called by the model.
///
/// Implementations of this trait should be stateless, and may not maintain any
/// internal state.
pub trait Tool: Send + Sync + 'static {
    /// The type of input that the tool accepts.
    type Input: TryFrom<Value>;

    /// Returns the name of the tool.
    fn name(&self) -> &str;

    /// Returns the tool definition for the model.
    fn definition(&self) -> ModelTool;

    /// Executes the tool with the given input.
    ///
    /// This method must return a future that is fully independent of `self`,
    /// and the future should be cancellation safe.
    fn execute(
        &self,
        input: Self::Input,
    ) -> impl Future<Output = ToolResult> + Send + 'static;
}

pub(crate) trait ToolObject: Send + Sync + 'static {
    fn name(&self) -> &str;

    fn definition(&self) -> ModelTool;

    fn execute(
        &self,
        arguments: Value,
    ) -> Pin<Box<dyn Future<Output = ToolResult> + Send>>;
}

pub(crate) struct AnyTool<T: Tool>(pub T);

impl<T: Tool> ToolObject for AnyTool<T>
where
    <T::Input as TryFrom<Value>>::Error: StdError,
{
    #[inline]
    fn name(&self) -> &str {
        self.0.name()
    }

    #[inline]
    fn definition(&self) -> ModelTool {
        self.0.definition()
    }

    #[inline]
    fn execute(
        &self,
        arguments: Value,
    ) -> Pin<Box<dyn Future<Output = ToolResult> + Send>> {
        let input: T::Input = match arguments.try_into() {
            Ok(input) => input,
            Err(err) => {
                let reason = format!("{err}");
                return Box::pin(std::future::ready(ToolResult::Err(
                    Error::invalid_input().with_reason(reason),
                )));
            }
        };
        Box::pin(self.0.execute(input))
    }
}
