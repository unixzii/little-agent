use std::fmt::{self, Debug};
use std::future::Future;

use little_agent_actor::{Actor, Message};
use little_agent_model::{
    ModelFinishReason, ModelMessage, ModelProviderError, ModelRequest,
    ToolCallRequest, ToolCallResult,
};

use super::AgentState;
use crate::conversation::Item as ConversationItem;
use crate::model_client::{ModelClient, ModelClientResponse};
use crate::tool::ToolResult;

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

    fn process_tool_call_requests(
        &mut self,
        requests: Vec<ToolCallRequest>,
        handle: &Actor<Self>,
    ) {
        if let Some(on_tool_call_request) = &self.on_tool_call_request {
            for request in &requests {
                on_tool_call_request(request);
            }
        }

        let mut tool_calls = vec![];
        self.tool_executor.handle_requests(requests, |id, fut| {
            tool_calls.push((id, fut));
        });
        for (id, fut) in tool_calls {
            self.pending_tool_results.insert(id.clone(), None);
            let handle_clone = handle.clone();
            self.spawn_task(
                |_| async move {
                    let result = fut.await;
                    handle_clone
                        .send(ToolCallFinishedMessage { id, result })
                        .ok();
                },
                handle,
            );
        }
    }

    /// Process the input string, assuming the stage is checked.
    fn process_input_checked(&mut self, input: String, handle: &Actor<Self>) {
        // Also invoke the transcript callback for user input, which can make
        // the messages in the conversation ordered correctly.
        if let Some(on_transcript) = &self.on_transcript {
            on_transcript(&input);
        }

        // Insert the message to the conversation.
        self.conversation.items.push(ConversationItem {
            msg: ModelMessage::User(input.clone()),
            transcript: input,
        });

        self.request_model_checked(handle);
    }

    /// Request the model with the current conversation, assuming the
    /// stage is checked.
    fn request_model_checked(&mut self, handle: &Actor<Self>) {
        self.current_stage = AgentStage::ModelThinking;

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
        let messages = self
            .conversation
            .items
            .iter()
            .map(|item| item.msg.clone())
            .collect();
        let tools = self.tool_executor.definitions();
        ModelRequest { messages, tools }
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
        if let Some(on_transcript) = &state.on_transcript {
            on_transcript(&transcript);
        }
        let msg = if let Some(opaque_msg) = resp.opaque_msg {
            ModelMessage::Opaque(opaque_msg)
        } else {
            // Downgrade to a text-only message.
            ModelMessage::Assistant(transcript.clone())
        };
        let conversation_item = ConversationItem { msg, transcript };
        state.conversation.items.push(conversation_item);

        // Release the model client.
        state.model_client = Some(self.model_client);

        // Check if we need to execute tools.
        let should_run_tools = resp.finish_reason
            == Some(ModelFinishReason::ToolCalls)
            && !resp.tool_calls.is_empty();
        if should_run_tools {
            state.current_stage = AgentStage::RunningTools;
            state.process_tool_call_requests(resp.tool_calls, handle);
        } else {
            // No tools to execute, continue to next input.
            state.current_stage = AgentStage::Idle;
            state.process_next_input(handle);
        }
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

#[derive(Debug)]
struct ToolCallFinishedMessage {
    id: String,
    result: ToolResult,
}

impl Message<AgentState> for ToolCallFinishedMessage {
    fn handle(self, state: &mut AgentState, handle: &Actor<AgentState>) {
        let Some(result) = state.pending_tool_results.get_mut(&self.id) else {
            debug_assert!(false, "internal state is inconsistent");
            return;
        };
        if let Some(on_tool_call_result) = &mut state.on_tool_result {
            on_tool_call_result(&self.id, &self.result);
        }
        *result = Some(self.result);

        let all_done = state.pending_tool_results.values().all(|r| r.is_some());
        if !all_done {
            return;
        }

        // Add the tool results to the conversation.
        for (id, result) in state.pending_tool_results.drain() {
            let result = result.unwrap();
            let transcript = if result.is_ok() {
                format!("Ran a tool")
            } else {
                format!("Failed to run tool")
            };
            let msg = ModelMessage::Tool(ToolCallResult {
                id,
                content: match result {
                    Ok(res) => res,
                    Err(err) => err.reason().into_owned(),
                },
            });
            let conversation_item = ConversationItem { msg, transcript };
            state.conversation.items.push(conversation_item);
        }

        // Now, proceed to the next turn directly.
        state.request_model_checked(handle);
    }
}
