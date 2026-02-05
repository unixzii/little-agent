use little_agent_core::tool::Approval as ToolApproval;
use little_agent_core::{Agent, AgentBuilder, TranscriptSource};
use little_agent_model::ModelProvider;

use crate::tools::*;

/// A session builder.
///
/// See [`Session`].
pub struct SessionBuilder {
    agent_builder: AgentBuilder,
}

impl SessionBuilder {
    /// Creates a session builder with a specified model provider.
    pub fn with_model_provider<M: ModelProvider + 'static>(
        provider: M,
    ) -> Self {
        let agent_builder = AgentBuilder::with_model_provider(provider);
        Self { agent_builder }
    }

    /// Sets the system prompt for the agent.
    #[inline]
    pub fn with_system_prompt<S: Into<String>>(mut self, prompt: S) -> Self {
        self.agent_builder = self.agent_builder.with_system_prompt(prompt);
        self
    }

    /// Attaches a callback to be invoked when the agent is idle.
    #[inline]
    pub fn on_idle(
        mut self,
        on_idle: impl Fn() + Send + Sync + 'static,
    ) -> Self {
        self.agent_builder = self.agent_builder.on_idle(on_idle);
        self
    }

    /// Attaches a callback to be invoked when a transcript is generated.
    #[inline]
    pub fn on_transcript(
        mut self,
        on_transcript: impl Fn(&str, TranscriptSource) + Send + Sync + 'static,
    ) -> Self {
        self.agent_builder = self.agent_builder.on_transcript(on_transcript);
        self
    }

    /// Attaches a callback to be invoked when a tool call request is received.
    #[inline]
    pub fn on_tool_call_request(
        mut self,
        on_tool_call_request: impl Fn(ToolApproval) + Send + Sync + 'static,
    ) -> Self {
        self.agent_builder = self
            .agent_builder
            .on_tool_call_request(on_tool_call_request);
        self
    }

    /// Builds a new session.
    pub fn build(self) -> Session {
        let agent = self
            .agent_builder
            .with_tool(ShellTool::new())
            .with_tool(GlobTool::new())
            .build();

        Session { agent }
    }
}

/// A chat session, like a window that displays messages and has a input box.
///
/// The session holds a fully configured agent that you can use directly, and it
/// is basically a wrapper around [`Agent`].
pub struct Session {
    agent: Agent,
}

impl Session {
    /// Sends a message to the session.
    #[inline]
    pub fn send_message(&self, message: &str) {
        self.agent.enqueue_user_input(message);
    }
}
