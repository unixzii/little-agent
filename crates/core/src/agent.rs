mod builder;
mod state;
#[cfg(test)]
mod tests;

use std::collections::{HashMap, VecDeque};

use little_agent_actor::define_actor;
use little_agent_model::ModelMessage;
use tokio::task::JoinHandle;

use crate::agent::state::EnqueueUserInput;
use crate::conversation::{Conversation, Item as ConversationItem};
use crate::model_client::ModelClient;
use crate::tool::{Manager as ToolManager, ToolResult};
pub use builder::AgentBuilder;
use state::AgentStage;

/// Where the transcript comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TranscriptSource {
    /// User input message.
    User,
    /// Assistant message.
    Assistant,
}

impl TranscriptSource {
    /// Returns true if the transcript source is assistant.
    #[inline]
    pub fn is_assistant(&self) -> bool {
        matches!(self, TranscriptSource::Assistant)
    }
}

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
    #[allow(clippy::type_complexity)]
    pub struct AgentState {
        model_client: Option<ModelClient>,
        tool_manager: ToolManager,
        conversation: Conversation,
        current_stage: AgentStage,
        pending_inputs: VecDeque<String>,
        pending_tool_results: HashMap<String, Option<ToolResult>>,
        running_tasks: HashMap<u64, JoinHandle<()>>,
        next_task_id: u64,

        on_idle: Option<Box<dyn Fn() + Send + Sync>>,
        on_transcript: Option<Box<dyn Fn(&str, TranscriptSource) + Send + Sync>>,
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
            tool_manager,
            system_prompt,
            on_idle,
            on_transcript,
        } = builder;

        let mut conversation = Conversation::default();
        if let Some(system_prompt) = system_prompt {
            conversation.items.push(ConversationItem {
                msg: ModelMessage::System(system_prompt.clone()),
                transcript: system_prompt,
            });
        }

        let state = AgentState {
            model_client: Some(model_client),
            tool_manager,
            conversation,
            current_stage: Default::default(),
            pending_inputs: Default::default(),
            pending_tool_results: Default::default(),
            running_tasks: Default::default(),
            next_task_id: 1,
            on_idle,
            on_transcript,
        };
        Self::spawn(state, Some("agent"))
    }
}
