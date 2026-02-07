use std::env;
use std::io;

use little_agent_core::tool::{
    Approval as ToolApproval, Error as ToolError, Tool, ToolResult,
};
use schemars::{JsonSchema, schema_for};
use serde::Deserialize;
use serde_json::Value;
use tokio::process::Command;

#[derive(Deserialize, JsonSchema)]
pub struct ShellToolParameters {
    #[schemars(description = "The command line to run.")]
    cmdline: String,
}

/// A tool for running shell commands.
pub struct ShellTool {
    parameter_schema: Value,
}

impl ShellTool {
    /// Creates a new shell tool.
    #[inline]
    pub fn new() -> Self {
        ShellTool {
            parameter_schema: schema_for!(ShellToolParameters).to_value(),
        }
    }
}

impl Default for ShellTool {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for ShellTool {
    type Input = ShellToolParameters;

    fn name(&self) -> &str {
        "shell"
    }

    fn description(&self) -> &str {
        r#"
Runs arbitrary commands like using a terminal.
The command line should be single line if possible. Strings collected from stdout and stderr will be returned as the tool's output."#
    }

    fn parameter_schema(&self) -> &Value {
        &self.parameter_schema
    }

    fn make_approval(&self, input: &Self::Input) -> ToolApproval {
        ToolApproval::new(&input.cmdline, "Agent wants to run the command")
    }

    #[allow(clippy::manual_async_fn)]
    fn execute(
        &self,
        input: ShellToolParameters,
    ) -> impl Future<Output = ToolResult> + Send + 'static {
        async move {
            run_command_line(&input.cmdline).await.map_err(|err| {
                ToolError::execution_error().with_reason(format!("{err}"))
            })
        }
    }
}

#[inline]
fn create_command_with_inferred_shell() -> Command {
    let Some(shell) = env::var_os("SHELL") else {
        return Command::new("/bin/sh");
    };
    Command::new(shell)
}

#[inline]
async fn run_command_line(cmdline: &str) -> Result<String, io::Error> {
    let output = create_command_with_inferred_shell()
        .arg("-cli")
        .arg(cmdline)
        .output()
        .await?;

    let mut result = String::new();
    if !output.stdout.is_empty() {
        result.push_str("==> STDOUT <==\n");
        result.push_str(&String::from_utf8_lossy(&output.stdout));
    }
    if !output.stderr.is_empty() {
        result.push_str("\n==> STDERR <==\n");
        result.push_str(&String::from_utf8_lossy(&output.stderr));
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_run_command_line() {
        let result = run_command_line("echo 'Hello, World!'").await;
        assert_eq!(result.unwrap(), "==> STDOUT <==\nHello, World!\n");
    }
}
