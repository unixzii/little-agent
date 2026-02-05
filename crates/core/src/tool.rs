//! Tool call supports.

mod approval;
mod error;
mod manager;
mod object;

use serde::de::DeserializeOwned;
use serde_json::Value;

pub use approval::Approval;
pub use error::{Error, ErrorKind};
pub(crate) use manager::Manager;

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
    type Input: DeserializeOwned + Send;

    /// Returns the name of the tool.
    fn name(&self) -> &str;

    /// Returns the description of the tool.
    fn description(&self) -> &str;

    /// Returns the parameter schema of the tool.
    fn parameter_schema(&self) -> &Value;

    /// Makes an approval for calling this tool with the given input.
    fn make_approval(&self, input: &Self::Input) -> Approval;

    /// Executes the tool with the given input.
    ///
    /// This method must return a future that is fully independent of `self`,
    /// and the future should be cancellation safe.
    fn execute(
        &self,
        input: Self::Input,
    ) -> impl Future<Output = ToolResult> + Send + 'static;
}
