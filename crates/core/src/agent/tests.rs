use std::future::ready;
use std::sync::atomic::{self, AtomicBool};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use little_agent_model::ToolCallRequest;
use little_agent_test_model::{PresetEvent, PresetResponse, TestModelProvider};
use serde_json::{Value, json};
use tokio::sync::watch;
use tokio::time::timeout;

use crate::AgentBuilder;
use crate::tool::{Approval, Error as ToolError, Tool, ToolResult};

#[tokio::test]
async fn test_simple_message() {
    let mut model_provider = TestModelProvider::default();
    model_provider.add_user_input_step();
    model_provider.add_assistant_response_step(PresetResponse::with_events([
        PresetEvent::MessageDelta("Hi, ".to_owned()),
        PresetEvent::MessageDelta("what can I do for you?".to_owned()),
    ]));

    let transcripts = Arc::new(Mutex::new(vec![]));
    let (idle_tx, mut idle_rx) = watch::channel::<bool>(false);

    let agent = AgentBuilder::with_model_provider(model_provider)
        .on_transcript({
            let transcripts = Arc::clone(&transcripts);
            move |transcript, _| {
                transcripts.lock().unwrap().push(transcript.to_owned());
            }
        })
        .on_idle(move || {
            idle_tx.send(true).unwrap();
        })
        .build();
    agent.enqueue_user_input("Hello");

    timeout(Duration::from_millis(500), idle_rx.wait_for(|v| *v))
        .await
        .unwrap()
        .unwrap();

    let transcripts = transcripts.lock().unwrap();
    assert_eq!(transcripts.len(), 3);
    assert_eq!(transcripts[0], "Hello");
    assert_eq!(transcripts[1], "Hi, ");
    assert_eq!(transcripts[2], "what can I do for you?");
}

static EMPTY_SCHEMA: &Value = &Value::Null;

struct ListTodosTool;

impl Tool for ListTodosTool {
    type Input = Value;

    fn name(&self) -> &str {
        "list_todos"
    }

    fn description(&self) -> &str {
        "Lists all todos"
    }

    fn parameter_schema(&self) -> &Value {
        EMPTY_SCHEMA
    }

    fn make_approval(&self, _input: &Self::Input) -> Approval {
        Approval::new(self.description(), "")
    }

    fn execute(
        &self,
        _input: Self::Input,
    ) -> impl Future<Output = ToolResult> + Send + 'static {
        ready(Ok("Found 0 todos".to_owned()))
    }
}

struct ListCalendarEventsTool;

impl Tool for ListCalendarEventsTool {
    type Input = Value;

    fn name(&self) -> &str {
        "list_calendar_events"
    }

    fn description(&self) -> &str {
        "Lists all calendar events"
    }

    fn parameter_schema(&self) -> &Value {
        EMPTY_SCHEMA
    }

    fn make_approval(&self, _input: &Self::Input) -> Approval {
        Approval::new(self.description(), "")
    }

    fn execute(
        &self,
        _input: Self::Input,
    ) -> impl Future<Output = ToolResult> + Send + 'static {
        ready(Err(ToolError::execution_error()))
    }
}

#[tokio::test]
async fn test_tool_call() {
    let mut model_provider = TestModelProvider::default();
    model_provider.add_user_input_step();
    model_provider.add_assistant_response_step(PresetResponse::with_events([
        PresetEvent::MessageDelta("Hi, ".to_owned()),
        PresetEvent::MessageDelta("let me check your todo.".to_owned()),
        PresetEvent::ToolCall(ToolCallRequest {
            id: "tool:1".to_owned(),
            name: "list_todos".to_owned(),
            arguments: json!({}),
        }),
        PresetEvent::ToolCall(ToolCallRequest {
            id: "tool:2".to_owned(),
            name: "list_calendar_events".to_owned(),
            arguments: json!({}),
        }),
    ]));
    model_provider.add_user_input_step();
    model_provider.add_user_input_step();
    model_provider.add_assistant_response_step(PresetResponse::with_events([
        PresetEvent::MessageDelta("Your todo is clean, good job!".to_owned()),
    ]));

    let tool_call_requests = Arc::new(Mutex::new(vec![]));
    let (idle_tx, mut idle_rx) = watch::channel::<bool>(false);

    let agent = AgentBuilder::with_model_provider(model_provider)
        .with_tool(ListTodosTool)
        .with_tool(ListCalendarEventsTool)
        .on_tool_call_request({
            let tool_call_requests = Arc::clone(&tool_call_requests);
            move |request| {
                tool_call_requests
                    .lock()
                    .unwrap()
                    .push(request.what().to_owned());
                request.approve();
            }
        })
        .on_idle(move || {
            idle_tx.send(true).unwrap();
        })
        .build();
    agent.enqueue_user_input("Hello");

    timeout(Duration::from_millis(500), idle_rx.wait_for(|v| *v))
        .await
        .unwrap()
        .unwrap();

    let tool_call_requests = tool_call_requests.lock().unwrap();
    assert_eq!(tool_call_requests.len(), 2);
    assert_eq!(tool_call_requests[0], "Lists all todos");
    assert_eq!(tool_call_requests[1], "Lists all calendar events");
}

#[tokio::test(start_paused = true)]
async fn test_retry() {
    let mut model_provider = TestModelProvider::default();
    model_provider.add_user_input_step();
    model_provider.add_assistant_response_step(
        PresetResponse::with_events([PresetEvent::MessageDelta(
            "Hi".to_owned(),
        )])
        .with_failures(3),
    );

    let on_error_triggered = Arc::new(AtomicBool::new(false));
    let transcripts = Arc::new(Mutex::new(String::new()));
    let (idle_tx, mut idle_rx) = watch::channel::<bool>(false);

    let agent = AgentBuilder::with_model_provider(model_provider)
        .on_transcript({
            let transcripts = Arc::clone(&transcripts);
            move |transcript, source| {
                if source.is_assistant() {
                    transcripts.lock().unwrap().push_str(transcript);
                }
            }
        })
        .on_error({
            let on_error_triggered = Arc::clone(&on_error_triggered);
            move |_| {
                on_error_triggered.store(true, atomic::Ordering::Relaxed);
            }
        })
        .on_idle(move || {
            idle_tx.send(true).unwrap();
        })
        .build();
    agent.enqueue_user_input("Hello");

    idle_rx.wait_for(|v| *v).await.unwrap();

    let transcripts = transcripts.lock().unwrap();
    assert_eq!(*transcripts, "Hi");
    assert_eq!(on_error_triggered.load(atomic::Ordering::Relaxed), true);
}
