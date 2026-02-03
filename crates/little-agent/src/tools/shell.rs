use std::io;

use little_agent_core::tool::{Error as ToolError, Tool, ToolResult};
use schemars::{JsonSchema, schema_for};
use serde::Deserialize;
use serde_json::Value;
use tokio::process::Command;
use tokio::sync::{mpsc, oneshot};

/// A pending approval for running a shell command.
pub struct ShellToolApproval {
    cmdline: String,
    approved_tx: oneshot::Sender<bool>,
}

impl ShellToolApproval {
    /// Returns the command line to run.
    #[inline]
    pub fn cmdline(&self) -> &str {
        &self.cmdline
    }

    /// Approves the request.
    #[inline]
    pub fn approve(self) -> bool {
        self.approved_tx.send(true).is_ok()
    }

    /// Rejects the request.
    #[inline]
    pub fn reject(self) -> bool {
        self.approved_tx.send(false).is_ok()
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct ShellToolParameters {
    #[schemars(description = "The command line to run.")]
    cmdline: String,
}

/// A tool for running shell commands.
pub struct ShellTool {
    parameter_schema: Value,
    approval_tx: mpsc::Sender<ShellToolApproval>,
}

impl ShellTool {
    /// Creates a new shell tool, returning the tool instance and an
    /// [`mpsc::Receiver`] for approvals.
    #[inline]
    pub fn new() -> (Self, mpsc::Receiver<ShellToolApproval>) {
        let (approval_tx, approval_rx) = mpsc::channel(1);
        (
            ShellTool {
                approval_tx,
                parameter_schema: schema_for!(ShellToolParameters).to_value(),
            },
            approval_rx,
        )
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
The command line should be single line if possible. Strings collected from stdout will be returned as the tool's output."#
    }

    fn parameter_schema(&self) -> &Value {
        &self.parameter_schema
    }

    fn execute(
        &self,
        input: ShellToolParameters,
    ) -> impl Future<Output = ToolResult> + Send + 'static {
        let approval_tx = self.approval_tx.clone();
        async move {
            let cmdline = input.cmdline;

            let (approved_tx, approved_rx) = oneshot::channel();
            let approval = ShellToolApproval {
                cmdline: cmdline.clone(),
                approved_tx,
            };
            if approval_tx.send(approval).await.is_err() {
                return Err(ToolError::permission_denied());
            }
            let Ok(approved) = approved_rx.await else {
                return Err(ToolError::permission_denied());
            };
            if !approved {
                return Err(ToolError::permission_denied());
            }

            run_command_line(&cmdline).await.map_err(|err| {
                ToolError::execution_error().with_reason(format!("{err}"))
            })
        }
    }
}

#[inline]
async fn run_command_line(cmdline: &str) -> Result<String, io::Error> {
    let cmd = Command::new("sh").arg("-c").arg(cmdline).output().await?;
    let stdout_str = String::from_utf8_lossy(&cmd.stdout).into_owned();
    Ok(stdout_str)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_run_command_line() {
        println!("{}", ShellTool::new().0.parameter_schema());

        let result = run_command_line("echo 'Hello, World!'").await;
        assert_eq!(result.unwrap(), "Hello, World!\n");
    }
}
