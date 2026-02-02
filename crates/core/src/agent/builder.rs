use std::error::Error;

use little_agent_model::ModelProvider;
use serde_json::Value;

use super::Agent;
use crate::model_client::ModelClient;
use crate::tool::{AnyTool, Tool, ToolObject};

/// [`Agent`] builder.
pub struct AgentBuilder {
    pub(crate) model_client: ModelClient,
    pub(crate) on_idle: Option<Box<dyn Fn() + Send + Sync>>,
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
            on_idle: None,
            tools: vec![],
        }
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

    /// Registers a tool.
    #[inline]
    pub fn with_tool<T: Tool>(mut self, tool: T) -> Self
    where
        <T::Input as TryFrom<Value>>::Error: Error,
    {
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
