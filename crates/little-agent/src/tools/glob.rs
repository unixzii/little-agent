use std::path::Path;

use little_agent_core::tool::{
    Approval as ToolApproval, Error as ToolError, Tool, ToolResult,
};
use schemars::{JsonSchema, schema_for};
use serde::Deserialize;
use serde_json::Value;
use tokio::task::spawn_blocking;

#[derive(Deserialize, JsonSchema)]
pub struct GlobToolParameters {
    #[schemars(description = "The glob pattern, must be relative to `path`.")]
    pattern: String,
    #[schemars(description = "Absolute path to search in.")]
    path: String,
}

/// A tool for finding files using glob patterns.
pub struct GlobTool {
    parameter_schema: Value,
}

impl GlobTool {
    /// Creates a new glob tool.
    #[inline]
    pub fn new() -> Self {
        GlobTool {
            parameter_schema: schema_for!(GlobToolParameters).to_value(),
        }
    }
}

impl Default for GlobTool {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for GlobTool {
    type Input = GlobToolParameters;

    fn name(&self) -> &str {
        "glob"
    }

    fn description(&self) -> &str {
        r#"
Find files and directories using glob patterns.
This tool supports standard glob syntax like *, ?, and ** for recursive searches."#
    }

    fn parameter_schema(&self) -> &Value {
        &self.parameter_schema
    }

    fn make_approval(&self, input: &Self::Input) -> ToolApproval {
        ToolApproval::new(&input.pattern, "Agent wants to list files")
    }

    #[allow(clippy::manual_async_fn)]
    fn execute(
        &self,
        input: GlobToolParameters,
    ) -> impl Future<Output = ToolResult> + Send + 'static {
        async move {
            if Path::new(&input.pattern).is_absolute() {
                return Err(ToolError::execution_error()
                    .with_reason("`pattern` must be relative to `path`"));
            }
            if !Path::new(&input.path).is_absolute() {
                return Err(ToolError::execution_error()
                    .with_reason("`path` must be absolute"));
            }

            let mut pattern = input.path;
            if pattern.bytes().last() != Some(b'/') {
                pattern.push('/');
            }
            pattern.push_str(&input.pattern);
            let pattern = match glob::glob(&pattern) {
                Ok(pattern) => pattern,
                Err(err) => {
                    return Err(ToolError::execution_error()
                        .with_reason(err.to_string()));
                }
            };

            spawn_blocking(move || {
                let mut result = String::new();
                // FIXME: Ok, the limit here may look arbitrary. And we need a
                // mechanism to handle continuation.
                for item in pattern.take(50).flatten() {
                    result.push_str(&item.to_string_lossy());
                    result.push('\n');
                }
                result
            })
            .await
            .map_err(|_| {
                ToolError::execution_error()
                    .with_reason("Failed to execute glob")
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_input_validation() {
        let tool = GlobTool::new();

        let result = tool
            .execute(GlobToolParameters {
                pattern: "*.rs".to_owned(),
                path: "some/relative/path".to_owned(),
            })
            .await;
        assert!(result.is_err());

        let result = tool
            .execute(GlobToolParameters {
                pattern: "/*.*".to_owned(),
                path: "/some/relative/path".to_owned(),
            })
            .await;
        assert!(result.is_err());

        let result = tool
            .execute(GlobToolParameters {
                pattern: "*".to_owned(),
                path: "/".to_owned(),
            })
            .await;
        assert!(!result.unwrap().is_empty());
    }
}
