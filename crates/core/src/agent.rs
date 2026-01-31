mod builder;
#[cfg(test)]
mod tests;

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Weak};

use little_agent_model::{ModelMessage, ModelRequest};
use tokio::sync::{Mutex, RwLock, mpsc};
use tokio::task::JoinHandle;
use tracing::Instrument;

use crate::model_client::ModelClient;
pub use builder::AgentBuilder;

/// An agent instance, which maintains a session, a model provider, and
/// internal state.
pub struct Agent {
    task: JoinHandle<()>,
    state: Arc<AgentState>,
}

impl Agent {
    /// Enqueues a user input for processing.
    pub fn enqueue_user_input<S: Into<String>>(&self, input: S) {
        self.state
            .action_tx
            .send(AgentAction::EnqueueUserInput(input.into()))
            .expect("agent task has been dropped too early");
    }
}

impl Agent {
    fn spawn(builder: AgentBuilder) -> Self {
        let AgentBuilder {
            model_client,
            on_idle,
        } = builder;

        let (action_tx, action_rx) = mpsc::unbounded_channel();
        let state = Arc::new_cyclic(|weak_self| AgentState {
            weak_self: weak_self.clone(),
            model_client,
            action_tx,
            current_stage: Default::default(),
            pending_inputs: Default::default(),
            running_tasks: Default::default(),
            next_task_id: AtomicU64::new(1),
            on_idle,
        });
        let task = tokio::spawn(
            serve_agent(state.clone(), action_rx)
                .instrument(debug_span!("agent")),
        );
        Agent { task, state }
    }
}

/// An action that can be dispatched to the agent.
///
/// The action should be handled immediately, no matter what the current
/// stage this agent is in. For example, if the agent is currently running
/// a tool, it should still process an `EnqueueUserInput` action. Instead
/// of calling the model, the agent should enqueue the user input, and
/// handle it later when it becomes idle.
///
/// There are also some actions that are only dispatched internally from
/// the agent itself, these actions should not be used externally.
#[derive(Debug)]
enum AgentAction {
    EnqueueUserInput(String),
    ProcessNextInput,
    Exit,
}

/// A stage of the agent.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum AgentStage {
    #[default]
    Idle,
    ModelThinking,
    RunningTools,
}

/// The shared state of an agent.
struct AgentState {
    weak_self: Weak<Self>,
    model_client: ModelClient,
    action_tx: mpsc::UnboundedSender<AgentAction>,
    current_stage: RwLock<AgentStage>,
    pending_inputs: Mutex<VecDeque<String>>,
    running_tasks: Mutex<HashMap<u64, JoinHandle<()>>>,
    next_task_id: AtomicU64,

    on_idle: Option<Box<dyn Fn() + Send + Sync>>,
}

impl AgentState {
    #[inline]
    async fn enqueue_user_input(&self, input: String) {
        let stage_lock = self.current_stage.write().await;
        if *stage_lock != AgentStage::Idle {
            // If we are not in idle stage, just enqueue the input and
            // do nothing else.
            self.pending_inputs.lock().await.push_back(input);
            return;
        }
        drop(stage_lock);
        self.process_input_checked(input).await;
    }

    async fn process_next_input(&self) {
        let stage_lock = self.current_stage.write().await;
        if *stage_lock != AgentStage::Idle {
            // Cannot process the next input now. We don't need to
            // dispatch another `ProcessNextInput` action, since it
            // will be dispatched automatically when other stages end.
            return;
        }
        let input = self.pending_inputs.lock().await.pop_front();
        if let Some(input) = input {
            drop(stage_lock);
            self.process_input_checked(input).await;
        } else {
            // Nothing to process, so we can invoke the idle callback.
            if let Some(on_idle) = &self.on_idle {
                on_idle();
            }
        }
    }

    /// Process the input string, assuming the stage is checked.
    async fn process_input_checked(&self, input: String) {
        let request = self.build_model_request(input);

        *self.current_stage.write().await = AgentStage::ModelThinking;

        let this = self.weak_self.upgrade().unwrap();
        self.spawn_task(|_| async move {
            // TODO: Implement this.
            let _resp = this.model_client.send_request(request).await;

            *this.current_stage.write().await = AgentStage::Idle;
            this.action_tx.send(AgentAction::ProcessNextInput).ok();
        })
        .await;
    }

    fn build_model_request(&self, input: String) -> ModelRequest {
        // TODO: Implement this.
        ModelRequest {
            messages: vec![ModelMessage::User(input)],
            tools: vec![],
        }
    }

    async fn spawn_task<F, Fut>(&self, f: F)
    where
        F: FnOnce(u64) -> Fut,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let task_id = self
            .next_task_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let this = self.weak_self.upgrade().unwrap();
        let fut = f(task_id);
        let task = tokio::spawn(async move {
            fut.await;
            this.running_tasks.lock().await.remove(&task_id);
        });
        self.running_tasks.lock().await.insert(task_id, task);
    }
}

async fn serve_agent(
    state: Arc<AgentState>,
    mut action_rx: mpsc::UnboundedReceiver<AgentAction>,
) {
    while let Some(action) = action_rx.recv().await {
        debug!("received action: {:?}", action);
        match action {
            AgentAction::EnqueueUserInput(input) => {
                state.enqueue_user_input(input).await;
            }
            AgentAction::ProcessNextInput => {
                state.process_next_input().await;
            }
            AgentAction::Exit => {
                todo!()
            }
        }
    }
    debug!("will terminate");
}
