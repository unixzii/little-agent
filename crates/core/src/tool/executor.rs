use std::collections::HashMap;
use std::pin::Pin;

use little_agent_model::{ModelTool, ToolCallRequest};

use crate::tool::{ToolObject, ToolResult};

/// An executor that handles tool call requests from the model.
pub struct Executor {
    tools: HashMap<String, Box<dyn ToolObject>>,
}

impl Executor {
    pub fn with_tools(tools: Vec<Box<dyn ToolObject>>) -> Self {
        let mut tool_map = HashMap::with_capacity(tools.len());
        for tool in tools {
            let name = tool.name();
            tool_map.insert(name.to_owned(), tool);
        }
        let tools = tool_map;
        Self { tools }
    }

    #[inline]
    pub fn definitions(&self) -> Vec<ModelTool> {
        self.tools.values().map(|tool| tool.definition()).collect()
    }

    pub fn handle_requests<S>(&self, requests: Vec<ToolCallRequest>, spawner: S)
    where
        S: FnMut(String, Pin<Box<dyn Future<Output = ToolResult> + Send>>),
    {
        let mut spawner = spawner;

        let span = debug_span!("tool executor");
        let _enter = span.enter();
        for req in requests {
            let Some(tool) = self.tools.get(&req.name) else {
                warn!("tool not found: {}", req.name);
                continue;
            };
            let id = req.id;
            let arguments = req.arguments;
            trace!("spawning a tool ({id}) with args: {arguments:?}");
            spawner(id, tool.execute(arguments));
        }
    }
}

#[cfg(test)]
mod tests {
    use std::future::ready;

    use serde_json::json;

    use super::*;
    use crate::tool::{AnyTool, Tool};

    struct TestTool;

    impl Tool for TestTool {
        type Input = serde_json::Value;

        fn name(&self) -> &str {
            "test_tool"
        }

        fn definition(&self) -> ModelTool {
            ModelTool {
                name: "test_tool".to_owned(),
                description: "A test tool".to_owned(),
                parameters: json!({}),
            }
        }

        fn execute(
            &self,
            _input: Self::Input,
        ) -> impl Future<Output = ToolResult> + Send + 'static {
            ready(Ok("success".to_owned()))
        }
    }

    #[test]
    fn test_handle_requests() {
        let executor = Executor::with_tools(vec![Box::new(AnyTool(TestTool))]);

        let requests = vec![ToolCallRequest {
            id: "tool:1".to_owned(),
            name: "test_tool".to_owned(),
            arguments: json!({}),
        }];

        let mut spawned_ids: Vec<String> = vec![];
        executor.handle_requests(requests, |id, _future| {
            spawned_ids.push(id);
        });

        assert_eq!(spawned_ids.len(), 1);
        assert_eq!(spawned_ids[0], "tool:1");

        // Test with non-existent tool.
        let requests = vec![ToolCallRequest {
            id: "tool:1".to_owned(),
            name: "read_tool".to_owned(),
            arguments: json!({}),
        }];

        let mut spawned_ids: Vec<String> = vec![];
        executor.handle_requests(requests, |id, _future| {
            spawned_ids.push(id);
        });

        assert!(spawned_ids.is_empty());
    }
}
