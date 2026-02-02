use little_agent_model::ToolCallRequest;
use serde::{Deserialize, Serialize};

/// The preset event in a response.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum PresetEvent {
    #[serde(rename = "message_delta")]
    MessageDelta(String),
    #[serde(rename = "tool_call")]
    ToolCall(ToolCallRequest),
}

/// The preset response in one turn.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PresetResponse {
    pub events: Vec<PresetEvent>,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn test_serialize_deserialize() {
        let response = PresetResponse {
            events: vec![
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
            ],
        };

        let serialized = serde_json::to_string(&response).unwrap();
        let deserialized: PresetResponse =
            serde_json::from_str(&serialized).unwrap();

        assert_eq!(response, deserialized);
    }
}
