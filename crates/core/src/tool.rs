//! Tool call supports.

mod error;
mod executor;

use std::pin::Pin;

use serde::de::DeserializeOwned;
use serde_json::Value;

pub use error::{Error, ErrorKind};
pub(crate) use executor::Executor;

/// The result of a tool call.
pub type ToolResult = Result<String, Error>;

/// A tool that can be called by the model.
///
/// Implementations of this trait should be stateless, and may not maintain any
/// internal state.
///
/// The tool can be context-aware, meaning it can access additional information
/// about the current execution context, such as the working directory or the
/// current user. To do this, make the context an immutable state of the tool,
/// which can be set during initialization, and copy it when executing.
pub trait Tool: Send + Sync + 'static {
    /// The type of input that the tool accepts.
    type Input: DeserializeOwned;

    /// Returns the name of the tool.
    fn name(&self) -> &str;

    /// Returns the description of the tool.
    fn description(&self) -> &str;

    /// Returns the parameter schema of the tool.
    fn parameter_schema(&self) -> &Value;

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

    fn description(&self) -> &str;

    fn parameter_schema(&self) -> &Value;

    fn execute(
        &self,
        arguments: Value,
    ) -> Pin<Box<dyn Future<Output = ToolResult> + Send>>;
}

pub(crate) struct AnyTool<T: Tool>(pub T);

impl<T: Tool> ToolObject for AnyTool<T> {
    #[inline]
    fn name(&self) -> &str {
        self.0.name()
    }

    #[inline]
    fn description(&self) -> &str {
        self.0.description()
    }

    #[inline]
    fn parameter_schema(&self) -> &Value {
        self.0.parameter_schema()
    }

    #[inline]
    fn execute(
        &self,
        arguments: Value,
    ) -> Pin<Box<dyn Future<Output = ToolResult> + Send>> {
        let input: T::Input = match serde_json::from_value(arguments) {
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
