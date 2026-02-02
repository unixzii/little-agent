use serde_json::Value;

use crate::OpaqueMessage;

/// A request to be sent to the model provider.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ModelRequest {
    /// The input messages.
    pub messages: Vec<ModelMessage>,
    /// Tools that are available to the model.
    pub tools: Vec<ModelTool>,
}

/// A complete message.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ModelMessage {
    /// The system instructions.
    System(String),
    /// A user input text.
    User(String),
    /// An assistant text.
    Assistant(String),
    /// A tool call result.
    Tool(ToolCallResult),
    /// An opaque message (usually the history message from the model)
    Opaque(OpaqueMessage),
}

/// The result of calling a tool.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ToolCallResult {
    /// The unique identifier for the tool call request.
    pub id: String,
    /// The result of the tool call.
    pub content: String,
}

/// Describes a tool that can be used by the model.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ModelTool {
    /// Name of the tool.
    pub name: String,
    /// Description of the tool.
    pub description: String,
    /// Parameters definition of the tool.
    ///
    /// For most model providers, the parameters should typically be
    /// defined by a [JSON schema](https://json-schema.org/).
    pub parameters: Value,
}
