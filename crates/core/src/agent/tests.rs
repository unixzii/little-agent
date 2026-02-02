use std::future::ready;
use std::time::Duration;

use little_agent_model::{ModelTool, ToolCallRequest};
use little_agent_test_model::{PresetEvent, PresetResponse, TestModelProvider};
use serde_json::{Value, json};
use tokio::sync::watch;
use tokio::time::timeout;

use crate::AgentBuilder;
use crate::tool::{Error as ToolError, Tool, ToolResult};

#[tokio::test]
async fn test_simple_message() {
    let mut model_provider = TestModelProvider::default();
    model_provider.add_user_turn();
    model_provider.add_assistant_turn(PresetResponse {
        events: vec![
            PresetEvent::MessageDelta("Hi, ".to_owned()),
            PresetEvent::MessageDelta("what can I do for you?".to_owned()),
        ],
    });

    let (idle_tx, mut idle_rx) = watch::channel::<bool>(false);

    let agent = AgentBuilder::with_model_provider(model_provider)
        .on_idle(move || {
            idle_tx.send(true).unwrap();
        })
        .build();
    agent.enqueue_user_input("Hello");

    timeout(Duration::from_millis(500), idle_rx.wait_for(|v| *v))
        .await
        .unwrap()
        .unwrap();
}

struct ListTodosTool;

impl Tool for ListTodosTool {
    type Input = Value;

    fn name(&self) -> &str {
        "list_todos"
    }

    fn definition(&self) -> ModelTool {
        ModelTool {
            name: "list_todos".to_owned(),
            description: "Lists all todos".to_owned(),
            parameters: vec![],
        }
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

    fn definition(&self) -> ModelTool {
        ModelTool {
            name: "list_calendar_events".to_owned(),
            description: "Lists all calendar events".to_owned(),
            parameters: vec![],
        }
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
    model_provider.add_user_turn();
    model_provider.add_assistant_turn(PresetResponse {
        events: vec![
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
        ],
    });
    model_provider.add_user_turn();
    model_provider.add_user_turn();
    model_provider.add_assistant_turn(PresetResponse {
        events: vec![PresetEvent::MessageDelta(
            "Your todo is clean, good job!".to_owned(),
        )],
    });

    let (idle_tx, mut idle_rx) = watch::channel::<bool>(false);

    let agent = AgentBuilder::with_model_provider(model_provider)
        .with_tool(ListTodosTool)
        .with_tool(ListCalendarEventsTool)
        .on_idle(move || {
            idle_tx.send(true).unwrap();
        })
        .build();
    agent.enqueue_user_input("Hello");

    timeout(Duration::from_millis(500), idle_rx.wait_for(|v| *v))
        .await
        .unwrap()
        .unwrap();
}
