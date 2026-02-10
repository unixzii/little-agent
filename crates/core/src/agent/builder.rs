use little_agent_model::{ModelProvider, ModelProviderError};

use super::{Agent, TranscriptSource};
use crate::Tool;
use crate::model_client::ModelClient;
use crate::tool::{Approval, Manager as ToolManager};

/// [`Agent`] builder.
#[allow(clippy::type_complexity)]
pub struct AgentBuilder {
    pub(crate) model_client: ModelClient,
    pub(crate) tool_manager: ToolManager,
    pub(crate) system_prompt: Option<String>,
    pub(crate) on_idle: Option<Box<dyn Fn() + Send + Sync>>,
    pub(crate) on_error:
        Option<Box<dyn Fn(Box<dyn ModelProviderError>) + Send + Sync>>,
    pub(crate) on_transcript:
        Option<Box<dyn Fn(&str, TranscriptSource) + Send + Sync>>,
}

impl AgentBuilder {
    /// Creates a new builder with the specified model provider.
    #[inline]
    pub fn with_model_provider<P: ModelProvider + 'static>(
        provider: P,
    ) -> Self {
        Self {
            model_client: ModelClient::new(provider),
            tool_manager: Default::default(),
            system_prompt: None,
            on_idle: None,
            on_error: None,
            on_transcript: None,
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

    /// Attaches a callback to be invoked when an error occurs.
    #[inline]
    pub fn on_error(
        mut self,
        on_error: impl Fn(Box<dyn ModelProviderError>) + Send + Sync + 'static,
    ) -> Self {
        self.on_error = Some(Box::new(on_error));
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
    ///
    /// The receiver can either approve or reject the request. If this callback
    /// is not provided, the request will be automatically approved.
    #[inline]
    pub fn on_tool_call_request(
        mut self,
        on_tool_call_request: impl Fn(Approval) + Send + Sync + 'static,
    ) -> Self {
        self.tool_manager.on_request(on_tool_call_request);
        self
    }

    /// Registers a tool.
    #[inline]
    pub fn with_tool<T: Tool>(mut self, tool: T) -> Self {
        self.tool_manager.add_tool(tool);
        self
    }

    /// Builds the agent.
    #[inline]
    pub fn build(self) -> Agent {
        Agent::spawn_from_builder(self)
    }
}
