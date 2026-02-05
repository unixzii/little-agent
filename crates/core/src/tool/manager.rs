use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;

use little_agent_model::{ModelTool, ToolCallRequest};

use crate::Tool;
use crate::tool::object::{ToolObject, ToolObjectImpl};
use crate::tool::{Approval, ToolResult};

/// An object that manages toolset and handles requests from the model.
#[derive(Default)]
pub struct Manager {
    tools: HashMap<String, Arc<dyn ToolObject>>,
    on_request: Option<Box<dyn Fn(Approval) + Send + Sync>>,
}

impl Manager {
    pub fn add_tool<T: Tool + 'static>(&mut self, tool: T) {
        let name = tool.name().to_owned();
        self.tools.insert(name, Arc::new(ToolObjectImpl(tool)));
    }

    #[inline]
    pub fn on_request<F: Fn(Approval) + Send + Sync + 'static>(
        &mut self,
        on_request: F,
    ) {
        self.on_request = Some(Box::new(on_request));
    }

    #[inline]
    pub fn definitions(&self) -> Vec<ModelTool> {
        self.tools
            .values()
            .map(|tool| ModelTool {
                name: tool.name().to_owned(),
                description: tool.description().to_owned(),
                parameters: tool.parameter_schema().clone(),
            })
            .collect()
    }

    pub fn handle_requests<S>(&self, requests: Vec<ToolCallRequest>, spawner: S)
    where
        S: FnMut(String, Pin<Box<dyn Future<Output = ToolResult> + Send>>),
    {
        let mut spawner = spawner;

        let span = debug_span!("tool manager");
        let _enter = span.enter();

        for req in requests {
            let Some(tool) = self.tools.get(&req.name) else {
                warn!("tool not found: {}", req.name);
                continue;
            };

            let id = req.id;
            let arguments = req.arguments;
            trace!("spawning a tool ({id}) with args: {arguments:?}");
            spawner(id, Arc::clone(tool).execute(arguments, &self.on_request));
        }
    }
}

#[cfg(test)]
mod tests {
    use std::future::ready;

    use serde_json::{Value, json};

    use super::*;

    static EMPTY_SCHEMA: &Value = &Value::Null;

    struct TestTool;

    impl Tool for TestTool {
        type Input = serde_json::Value;

        fn name(&self) -> &str {
            "test_tool"
        }

        fn description(&self) -> &str {
            "A test tool"
        }

        fn parameter_schema(&self) -> &serde_json::Value {
            EMPTY_SCHEMA
        }

        fn make_approval(&self, _input: &Self::Input) -> Approval {
            Approval::new("", "")
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
        let mut manager = Manager::default();
        manager.add_tool(TestTool);

        let requests = vec![ToolCallRequest {
            id: "tool:1".to_owned(),
            name: "test_tool".to_owned(),
            arguments: json!({}),
        }];

        let mut spawned_ids: Vec<String> = vec![];
        manager.handle_requests(requests, |id, _future| {
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
        manager.handle_requests(requests, |id, _future| {
            spawned_ids.push(id);
        });

        assert!(spawned_ids.is_empty());
    }
}
