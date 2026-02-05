use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;

use little_agent_core::tool::{
    Approval as ToolApproval, Error as ToolError, Tool, ToolResult,
};
use schemars::{JsonSchema, schema_for};
use serde::Deserialize;
use serde_json::Value;
use tokio::task::spawn_blocking;

const MAX_LINES: usize = 50;

#[derive(Deserialize, JsonSchema)]
pub struct ReadFileItem {
    #[schemars(description = "Absolute path to the file.")]
    path: String,
    #[schemars(description = "1-based start line to read from, default to 1.")]
    start_line: Option<usize>,
}

#[derive(Deserialize, JsonSchema)]
pub struct ReadFileParameters {
    #[schemars(description = "Files to read.")]
    files: Vec<ReadFileItem>,
}

/// A tool for reading file content with line numbers.
pub struct ReadFileTool {
    parameter_schema: Value,
}

impl ReadFileTool {
    /// Creates a new read file tool.
    #[inline]
    pub fn new() -> Self {
        ReadFileTool {
            parameter_schema: schema_for!(ReadFileParameters).to_value(),
        }
    }
}

impl Default for ReadFileTool {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for ReadFileTool {
    type Input = ReadFileParameters;

    fn name(&self) -> &str {
        "read_file"
    }

    fn description(&self) -> &str {
        r#"
Reads files from absolute paths and returns their contents prefixed with line numbers.
Each file includes a path and a 1-based start line, and returns up to 50 lines."#
    }

    fn parameter_schema(&self) -> &Value {
        &self.parameter_schema
    }

    fn make_approval(&self, input: &ReadFileParameters) -> ToolApproval {
        let mut summary = String::new();
        for item in &input.files {
            if !summary.is_empty() {
                summary.push('\n');
            }
            let start_line = item.start_line.unwrap_or(1);
            let end_line = start_line + MAX_LINES - 1;
            summary.push_str(&format!(
                "{} L{}-{}",
                item.path, start_line, end_line
            ));
        }
        ToolApproval::new(&summary, "Agent wants to read these files")
    }

    #[allow(clippy::manual_async_fn)]
    fn execute(
        &self,
        input: ReadFileParameters,
    ) -> impl Future<Output = ToolResult> + Send + 'static {
        async move {
            let mut result = String::new();
            for file in input.files {
                if !Path::new(&file.path).is_absolute() {
                    return Err(ToolError::execution_error()
                        .with_reason("`path` must be absolute"));
                }
                let start_line = file.start_line.unwrap_or(1);
                if start_line == 0 {
                    return Err(ToolError::execution_error()
                        .with_reason("`start_line` must be 1-based"));
                }

                let section = spawn_blocking(move || {
                    read_file_section(&file.path, start_line)
                })
                .await
                .map_err(|_| {
                    ToolError::execution_error()
                        .with_reason("Failed to read file")
                })??;

                if !result.is_empty() {
                    result.push('\n');
                }
                result.push_str(&section);
            }
            Ok(result)
        }
    }
}

fn read_file_section(
    path: &str,
    start_line: usize,
) -> Result<String, ToolError> {
    let file = File::open(path).map_err(|err| {
        ToolError::execution_error().with_reason(err.to_string())
    })?;
    format_reader_section(path, file, start_line)
}

// TODO: AI wrote this function, but I think it's too inefficient. Need to
// rewrite this.
fn format_reader_section<R: Read>(
    path: &str,
    reader: R,
    start_line: usize,
) -> Result<String, ToolError> {
    let reader = BufReader::new(reader);
    let mut lines = Vec::new();
    for (index, line) in reader.lines().enumerate() {
        let line_no = index + 1;
        if line_no < start_line {
            continue;
        }
        let line = line.map_err(|err| {
            ToolError::execution_error().with_reason(err.to_string())
        })?;
        lines.push(line);
        if lines.len() >= MAX_LINES {
            break;
        }
    }

    let mut result = String::new();
    result.push_str(&format!("==> {path} <==\n"));

    if !lines.is_empty() {
        let last_line_no = start_line + lines.len() - 1;
        let width = last_line_no.to_string().len();
        for (offset, line) in lines.into_iter().enumerate() {
            let line_no = start_line + offset;
            result.push_str(&format!("{line_no:>width$}: {line}\n"));
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_read_file_section_formats_lines() {
        let input = b"first\nsecond\nthird\n";

        let output =
            format_reader_section("/fake/path", Cursor::new(input), 2).unwrap();
        let mut output_lines = output.lines();

        assert_eq!(output_lines.next().unwrap(), "==> /fake/path <==");
        assert_eq!(output_lines.next().unwrap(), "2: second");
        assert_eq!(output_lines.next().unwrap(), "3: third");
    }

    #[test]
    fn test_read_file_section_respects_limit() {
        let mut input = Vec::new();
        for _ in 0..(MAX_LINES + 10) {
            input.extend_from_slice(b"line\n");
        }

        let output =
            format_reader_section("/fake/path", Cursor::new(input), 1).unwrap();
        let lines = output.lines().collect::<Vec<_>>();
        assert_eq!(lines.len(), MAX_LINES + 1);
    }
}
