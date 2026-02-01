use little_agent_model::ModelProvider;

use super::Agent;
use crate::model_client::ModelClient;

/// [`Agent`] builder.
pub struct AgentBuilder {
    pub(crate) model_client: ModelClient,
    pub(crate) on_idle: Option<Box<dyn Fn() + Send + Sync>>,
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

    /// Builds the agent.
    #[inline]
    pub fn build(self) -> Agent {
        Agent::spawn_from_builder(self)
    }
}
