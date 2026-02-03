use little_agent_core::{Agent, AgentBuilder, TranscriptSource};
use little_agent_model::ModelProvider;
use little_agent_test_model::TestModelProvider;
use tokio::task::JoinHandle;

use crate::tools::{ShellTool, ShellToolApproval};

/// A session builder.
///
/// See [`Session`].
pub struct SessionBuilder {
    agent_builder: AgentBuilder,
    on_shell_request: Option<Box<dyn Fn(ShellToolApproval) + Send + Sync>>,
}

impl SessionBuilder {
    /// Creates a session builder with a test model.
    #[inline]
    pub fn with_test_model(provider: TestModelProvider) -> Self {
        Self::with_model_provider(provider)
    }

    fn with_model_provider<M: ModelProvider + 'static>(provider: M) -> Self {
        let agent_builder = AgentBuilder::with_model_provider(provider);
        Self {
            agent_builder,
            on_shell_request: None,
        }
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

    /// Attaches a callback to be invoked when a shell request is received.
    #[inline]
    pub fn on_shell_request(
        mut self,
        on_shell_request: impl Fn(ShellToolApproval) + Send + Sync + 'static,
    ) -> Self {
        self.on_shell_request = Some(Box::new(on_shell_request));
        self
    }

    /// Builds a new session.
    pub fn build(self) -> Session {
        let (shell_tool, mut shell_tool_approval_rx) = ShellTool::new();
        let approval_dispatching_task = if let Some(on_shell_request) =
            self.on_shell_request
        {
            tokio::spawn(async move {
                while let Some(approval) = shell_tool_approval_rx.recv().await {
                    on_shell_request(approval);
                }
            })
        } else {
            tokio::spawn(async move {
                while let Some(approval) = shell_tool_approval_rx.recv().await {
                    info!("will run command line: `{}`", approval.cmdline());
                    approval.approve();
                }
            })
        };

        let agent = self.agent_builder.with_tool(shell_tool).build();

        Session {
            agent,
            approval_dispatching_task,
        }
    }
}

/// A chat session, like a window that displays messages and has a input box.
///
/// The session holds a fully configured agent that you can use directly, and it
/// is basically a wrapper around [`Agent`].
pub struct Session {
    agent: Agent,
    approval_dispatching_task: JoinHandle<()>,
}

impl Session {
    /// Sends a message to the session.
    #[inline]
    pub fn send_message(&self, message: &str) {
        self.agent.enqueue_user_input(message);
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.approval_dispatching_task.abort();
    }
}
