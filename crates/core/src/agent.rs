mod builder;
mod state;
#[cfg(test)]
mod tests;

use std::collections::{HashMap, VecDeque};

use little_agent_actor::define_actor;
use tokio::task::JoinHandle;

use crate::agent::state::EnqueueUserInput;
use crate::conversation::Conversation;
use crate::model_client::ModelClient;
use crate::tool::{Executor as ToolExecutor, ToolResult};
pub use builder::AgentBuilder;
use state::AgentStage;

define_actor! {
    /// An agent instance, which maintains a session, a model provider, and
    /// internal state.
    ///
    /// Messages dispatched to the agent should be handled immediately, no
    /// matter what the current stage this agent is in. For example, if the
    /// agent is currently running a tool, it should still process an
    /// `enqueue_user_input` message. Instead of calling the model, the agent
    /// enqueues the user input, and handle it later when it becomes idle.
    #[wrapper_type(Agent)]
    pub struct AgentState {
        model_client: Option<ModelClient>,
        tool_executor: ToolExecutor,
        conversation: Conversation,
        current_stage: AgentStage,
        pending_inputs: VecDeque<String>,
        pending_tool_results: HashMap<String, Option<ToolResult>>,
        running_tasks: HashMap<u64, JoinHandle<()>>,
        next_task_id: u64,

        on_idle: Option<Box<dyn Fn() + Send + Sync>>,
    }
}

impl Agent {
    /// Enqueues a user input for processing.
    pub fn enqueue_user_input<S: Into<String>>(&self, input: S) {
        self.handle()
            .send(EnqueueUserInput(input.into()))
            .expect("agent task has been dropped too early");
    }
}

impl Agent {
    fn spawn_from_builder(builder: AgentBuilder) -> Self {
        let AgentBuilder {
            model_client,
            on_idle,
            tools,
        } = builder;

        let state = AgentState {
            model_client: Some(model_client),
            tool_executor: ToolExecutor::with_tools(tools),
            conversation: Default::default(),
            current_stage: Default::default(),
            pending_inputs: Default::default(),
            pending_tool_results: Default::default(),
            running_tasks: Default::default(),
            next_task_id: 1,
            on_idle,
        };
        Self::spawn(state, Some("agent"))
    }
}
