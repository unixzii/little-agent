use std::fmt::{self, Debug};

use little_agent_actor::{Actor, Message};
use little_agent_model::{ModelMessage, ModelProviderError, ModelRequest};

use super::AgentState;
use crate::conversation::Item as ConversationItem;
use crate::model_client::{ModelClient, ModelClientResponse};

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum AgentStage {
    #[default]
    Idle,
    ModelThinking,
    RunningTools,
}

impl AgentState {
    #[inline]
    fn enqueue_user_input(&mut self, input: String, handle: &Actor<Self>) {
        if self.current_stage != AgentStage::Idle {
            // If we are not in idle stage, just enqueue the input and
            // do nothing else.
            self.pending_inputs.push_back(input);
            return;
        }
        self.process_input_checked(input, handle);
    }

    fn process_next_input(&mut self, handle: &Actor<Self>) {
        if self.current_stage != AgentStage::Idle {
            // Cannot process the next input now. We don't need to send
            // another message to do this again, since it will be sent
            // automatically when other model client requests end.
            return;
        }
        let input = self.pending_inputs.pop_front();
        if let Some(input) = input {
            self.process_input_checked(input, handle);
        } else {
            // Nothing to process, so we can invoke the idle callback.
            if let Some(on_idle) = &self.on_idle {
                on_idle();
            }
        }
    }

    /// Process the input string, assuming the stage is checked.
    fn process_input_checked(&mut self, input: String, handle: &Actor<Self>) {
        self.current_stage = AgentStage::ModelThinking;

        // Insert the message to the conversation.
        self.conversation.items.push(ConversationItem {
            msg: ModelMessage::User(input.clone()),
            transcript: input,
        });

        let request = self.build_model_request();
        let model_client = self
            .model_client
            .take()
            .expect("model client is already in use");
        let handle_clone = handle.clone();
        self.spawn_task(
            |_| async move {
                // TODO: Implement this.
                let resp_res = model_client.send_request(request).await;

                handle_clone
                    .send(ModelClientRequestFinishedMessage {
                        model_client,
                        response: resp_res,
                    })
                    .ok();
            },
            handle,
        );
    }

    fn build_model_request(&self) -> ModelRequest {
        ModelRequest {
            messages: self
                .conversation
                .items
                .iter()
                .map(|i| i.msg.clone())
                .collect(),
            tools: vec![],
        }
    }

    fn spawn_task<F, Fut>(&mut self, f: F, handle: &Actor<Self>)
    where
        F: FnOnce(u64) -> Fut,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let task_id = self.next_task_id;
        self.next_task_id += 1;

        let handle = handle.clone();
        let fut = f(task_id);
        let task = tokio::spawn(async move {
            fut.await;
            handle.send(TaskEndedMessage(task_id)).ok();
        });
        self.running_tasks.insert(task_id, task);
    }
}

#[derive(Debug)]
pub struct EnqueueUserInput(pub String);

impl Message<AgentState> for EnqueueUserInput {
    fn handle(self, state: &mut AgentState, handle: &Actor<AgentState>) {
        state.enqueue_user_input(self.0, handle);
    }
}

struct ModelClientRequestFinishedMessage {
    model_client: ModelClient,
    response: Result<ModelClientResponse, Box<dyn ModelProviderError>>,
}

impl Debug for ModelClientRequestFinishedMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ModelClientRequestFinishedMessage")
            .field("response", &self.response)
            .finish_non_exhaustive()
    }
}

impl Message<AgentState> for ModelClientRequestFinishedMessage {
    fn handle(self, state: &mut AgentState, handle: &Actor<AgentState>) {
        let resp = match self.response {
            Ok(resp) => resp,
            Err(_) => unimplemented!(),
        };

        // Insert the message to the conversation.
        let transcript = resp.transcript;
        let msg = if let Some(opaque_msg) = resp.opaque_msg {
            ModelMessage::Opaque(opaque_msg)
        } else {
            // Downgrade to a text-only message.
            ModelMessage::Assistant(transcript.clone())
        };
        let conversation_item = ConversationItem { msg, transcript };
        state.conversation.items.push(conversation_item);

        // TODO: Implement this.
        state.model_client = Some(self.model_client);
        state.current_stage = AgentStage::Idle;
        state.process_next_input(handle);
    }
}

#[derive(Debug)]
struct TaskEndedMessage(u64);

impl Message<AgentState> for TaskEndedMessage {
    #[inline]
    fn handle(self, state: &mut AgentState, _handle: &Actor<AgentState>) {
        state
            .running_tasks
            .remove(&self.0)
            .expect("internal state is inconsistent");
    }
}
