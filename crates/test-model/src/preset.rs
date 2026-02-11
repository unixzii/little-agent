use little_agent_model::ToolCallRequest;
use serde::{Deserialize, Serialize};

/// The events in a preset response.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum PresetEvent {
    #[serde(rename = "message_delta")]
    MessageDelta(String),
    #[serde(rename = "tool_call")]
    ToolCall(ToolCallRequest),
}

/// The preset response for an assistant step.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PresetResponse {
    /// Events in this response.
    pub events: Vec<PresetEvent>,
    /// If set, the request will fail in the first `failure` attempts.
    /// `Some(0)` means the request will fail infinitely.
    pub failures: Option<u64>,
}

impl PresetResponse {
    /// Creates a `PresetResponse` with the specified events.
    #[inline]
    pub fn with_events(events: impl Into<Vec<PresetEvent>>) -> Self {
        Self {
            events: events.into(),
            failures: None,
        }
    }

    /// Sets failure times before a successful response. `0` means the
    /// response will always be a failure.
    #[inline]
    pub fn with_failures(mut self, failures: u64) -> Self {
        self.failures = Some(failures);
        self
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn test_serialize_deserialize() {
        let response = PresetResponse::with_events([
            PresetEvent::MessageDelta(
                "I have left a message for you.".to_string(),
            ),
            PresetEvent::ToolCall(ToolCallRequest {
                id: "1".to_string(),
                name: "write_file".to_string(),
                arguments: json!({
                    "filename": "message.txt",
                    "content": "Hello, world!"
                }),
            }),
        ]);

        let serialized = serde_json::to_string(&response).unwrap();
        let deserialized: PresetResponse =
            serde_json::from_str(&serialized).unwrap();

        assert_eq!(response, deserialized);
    }
}
