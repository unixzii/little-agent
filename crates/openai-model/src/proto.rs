use little_agent_model::{ModelMessage, ModelRequest, ModelTool};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::OpenAIConfig;

// ------------------------------
// Types received from the server
// ------------------------------

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FunctionToolCall {
    pub name: Option<String>,
    pub arguments: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ToolCall {
    pub index: Option<u32>,
    pub id: Option<String>,
    pub r#type: Option<String>,
    pub function: Option<FunctionToolCall>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Deserialize)]
pub struct ChatCompletionChunk {
    pub id: String,
    pub choices: Vec<Choice>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Deserialize)]
pub struct Choice {
    pub delta: Delta,
    pub finish_reason: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Deserialize)]
pub struct Delta {
    pub content: Option<String>,
    pub tool_calls: Option<Vec<ToolCall>>,
    pub reasoning_content: Option<String>,
}

// ------------------------
// Types sent to the server
// ------------------------

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
struct FunctionTool {
    name: String,
    description: String,
    parameters: Value,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
struct Tool {
    r#type: &'static str,
    function: FunctionTool,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(tag = "role", rename_all = "lowercase")]
pub enum Message {
    System {
        content: String,
    },
    User {
        content: String,
    },
    Assistant {
        content: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        tool_calls: Option<Vec<ToolCall>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        reasoning_content: Option<String>,
    },
    Tool {
        tool_call_id: String,
        content: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
pub struct ChatCompletionRequest {
    model: String,
    messages: Vec<Message>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<Tool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream_options: Option<StreamOptions>,
    stream: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
struct StreamOptions {
    include_usage: bool,
}

// -----------
// Conversions
// -----------

#[inline]
pub fn create_request(
    req: &ModelRequest,
    config: &OpenAIConfig,
) -> ChatCompletionRequest {
    ChatCompletionRequest {
        model: config.model.clone(),
        messages: req.messages.iter().map(create_message).collect(),
        tools: req.tools.iter().map(create_tool).collect(),
        stream_options: Some(StreamOptions {
            include_usage: true,
        }),
        stream: true,
    }
}

#[inline]
fn create_message(msg: &ModelMessage) -> Message {
    match msg {
        ModelMessage::System(content) => Message::System {
            content: content.clone(),
        },
        ModelMessage::User(content) => Message::User {
            content: content.clone(),
        },
        ModelMessage::Assistant(content) => Message::Assistant {
            content: Some(content.clone()),
            tool_calls: None,
            reasoning_content: None,
        },
        ModelMessage::Tool(result) => Message::Tool {
            tool_call_id: result.id.clone(),
            content: result.content.clone(),
        },
        ModelMessage::Opaque(opaque_message) => {
            // Opaque messages from this provide always have `Message` type.
            let Some(msg) = opaque_message.to_raw::<Message>() else {
                return Message::Assistant {
                    content: None,
                    tool_calls: None,
                    reasoning_content: None,
                };
            };
            msg.clone()
        }
    }
}

#[inline]
fn create_tool(tool: &ModelTool) -> Tool {
    Tool {
        r#type: "function",
        function: FunctionTool {
            name: tool.name.clone(),
            description: tool.description.clone(),
            parameters: tool.parameters.clone(),
        },
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::OpenAIConfigBuilder;

    #[test]
    fn test_create_request() {
        let request = ModelRequest {
            messages: vec![
                ModelMessage::System("You are a helpful assistant.".to_owned()),
                ModelMessage::User("Hello".to_owned()),
            ],
            tools: vec![ModelTool {
                name: "shell".to_owned(),
                description: "Runs shell commands.".to_owned(),
                parameters: json!({
                    "type": "string",
                    "description": "The command line."
                }),
            }],
        };
        let config = OpenAIConfigBuilder::with_api_key("xxx")
            .with_model("custom")
            .build();
        let expected = ChatCompletionRequest {
            model: "custom".to_owned(),
            messages: vec![
                Message::System {
                    content: "You are a helpful assistant.".to_owned(),
                },
                Message::User {
                    content: "Hello".to_owned(),
                },
            ],
            tools: vec![Tool {
                r#type: "function",
                function: FunctionTool {
                    name: "shell".to_owned(),
                    description: "Runs shell commands.".to_owned(),
                    parameters: json!({
                        "type": "string",
                        "description": "The command line."
                    }),
                },
            }],
            stream_options: Some(StreamOptions {
                include_usage: true,
            }),
            stream: true,
        };
        assert_eq!(create_request(&request, &config), expected);
    }
}
