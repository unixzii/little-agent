use std::time::Duration;

use little_agent_test_model::{PresetEvent, PresetResponse, TestModelProvider};
use tokio::sync::watch;
use tokio::time::timeout;

use crate::AgentBuilder;

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
