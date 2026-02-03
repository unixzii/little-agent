use little_agent_model::ModelProvider;
use little_agent_model::ToolCallRequest;

use super::{Agent, TranscriptSource};
use crate::model_client::ModelClient;
use crate::tool::{AnyTool, Tool, ToolObject, ToolResult};

/// [`Agent`] builder.
#[allow(clippy::type_complexity)]
pub struct AgentBuilder {
    pub(crate) model_client: ModelClient,
    pub(crate) system_prompt: Option<String>,
    pub(crate) on_idle: Option<Box<dyn Fn() + Send + Sync>>,
    pub(crate) on_transcript:
        Option<Box<dyn Fn(&str, TranscriptSource) + Send + Sync>>,
    pub(crate) on_tool_call_request:
        Option<Box<dyn Fn(&ToolCallRequest) + Send + Sync>>,
    pub(crate) on_tool_result:
        Option<Box<dyn Fn(&str, &ToolResult) + Send + Sync>>,
    pub(crate) tools: Vec<Box<dyn ToolObject>>,
}

impl AgentBuilder {
    /// Creates a new builder with the specified model provider.
    #[inline]
    pub fn with_model_provider<P: ModelProvider + 'static>(
        provider: P,
    ) -> Self {
        Self {
            model_client: ModelClient::new(provider),
            system_prompt: None,
            on_idle: None,
            on_transcript: None,
            on_tool_call_request: None,
            on_tool_result: None,
            tools: vec![],
        }
    }

    /// Sets the system prompt for the agent.
    #[inline]
    pub fn with_system_prompt<S: Into<String>>(mut self, prompt: S) -> Self {
        self.system_prompt = Some(prompt.into());
        self
    }

    /// Attaches a callback to be invoked when the agent is idle.
    #[inline]
    pub fn on_idle(
        mut self,
        on_idle: impl Fn() + Send + Sync + 'static,
    ) -> Self {
        self.on_idle = Some(Box::new(on_idle));
        self
    }

    /// Attaches a callback to be invoked when a transcript is generated.
    #[inline]
    pub fn on_transcript(
        mut self,
        on_transcript: impl Fn(&str, TranscriptSource) + Send + Sync + 'static,
    ) -> Self {
        self.on_transcript = Some(Box::new(on_transcript));
        self
    }

    /// Attaches a callback to be invoked when a tool call request is received.
    #[inline]
    pub fn on_tool_call_request(
        mut self,
        on_tool_call_request: impl Fn(&ToolCallRequest) + Send + Sync + 'static,
    ) -> Self {
        self.on_tool_call_request = Some(Box::new(on_tool_call_request));
        self
    }

    /// Attaches a callback to be invoked when a tool result is received.
    #[inline]
    pub fn on_tool_result(
        mut self,
        on_tool_result: impl Fn(&str, &ToolResult) + Send + Sync + 'static,
    ) -> Self {
        self.on_tool_result = Some(Box::new(on_tool_result));
        self
    }

    /// Registers a tool.
    #[inline]
    pub fn with_tool<T: Tool>(mut self, tool: T) -> Self {
        let tool = Box::new(AnyTool(tool));
        self.tools.push(tool);
        self
    }

    /// Builds the agent.
    #[inline]
    pub fn build(self) -> Agent {
        Agent::spawn_from_builder(self)
    }
}
