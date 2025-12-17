//! HUML Parser - Rust wrapper for native OCaml HUML parser
//!
//! Parses HUML template syntax directly (not YAML/JSON).
//! Calls the native OCaml executable via subprocess.
//!
//! **Usage:**
//! ```ignore
//! let parser = HumlParser::new("/path/to/huml_native.exe")?;
//! let template_json = parser.parse(huml_content)?;
//! ```

use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use thiserror::Error;
use tracing::{debug, instrument};

#[derive(Error, Debug)]
pub enum HumlError {
    #[error("Parser executable not found: {0}")]
    ExecutableNotFound(PathBuf),

    #[error("Failed to spawn parser process: {0}")]
    SpawnFailed(#[from] std::io::Error),

    #[error("Parser returned error: {0}")]
    ParseError(String),

    #[error("Failed to parse JSON response: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("Parser process failed with exit code: {0:?}")]
    ProcessFailed(Option<i32>),
}

/// Response from the native HUML parser
#[derive(Debug, Deserialize)]
struct ParserResponse {
    success: bool,
    result: Option<serde_json::Value>,
    error: Option<String>,
}

/// HUML Parser wrapper
///
/// Calls the native OCaml HUML parser executable.
pub struct HumlParser {
    executable_path: PathBuf,
}

impl HumlParser {
    /// Create a new HUML parser.
    ///
    /// **Arguments:**
    /// - `executable_path`: Path to the `huml_native.exe` binary
    pub fn new(executable_path: PathBuf) -> Result<Self, HumlError> {
        if !executable_path.exists() {
            return Err(HumlError::ExecutableNotFound(executable_path));
        }
        Ok(Self { executable_path })
    }

    /// Parse HUML content and return the result as JSON.
    ///
    /// **Arguments:**
    /// - `huml_content`: The HUML template string
    ///
    /// **Returns:**
    /// - The parsed template as a JSON Value
    #[instrument(skip(self, huml_content), fields(content_len = huml_content.len()))]
    pub fn parse(&self, huml_content: &str) -> Result<serde_json::Value, HumlError> {
        debug!("Parsing HUML content");

        let mut child = Command::new(&self.executable_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        // Write HUML content to stdin
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(huml_content.as_bytes())?;
        }

        // Wait for process to complete
        let output = child.wait_with_output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if !stderr.is_empty() {
                return Err(HumlError::ParseError(stderr.to_string()));
            }
            return Err(HumlError::ProcessFailed(output.status.code()));
        }

        // Parse JSON response
        let response: ParserResponse = serde_json::from_slice(&output.stdout)?;

        if response.success {
            response
                .result
                .ok_or_else(|| HumlError::ParseError("No result in response".to_string()))
        } else {
            Err(HumlError::ParseError(
                response.error.unwrap_or_else(|| "Unknown error".to_string()),
            ))
        }
    }

    /// Parse HUML content and deserialize into a specific type.
    ///
    /// **Arguments:**
    /// - `huml_content`: The HUML template string
    ///
    /// **Returns:**
    /// - The parsed and deserialized value
    pub fn parse_as<T: for<'de> Deserialize<'de>>(
        &self,
        huml_content: &str,
    ) -> Result<T, HumlError> {
        let json = self.parse(huml_content)?;
        serde_json::from_value(json).map_err(HumlError::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parser_not_found() {
        let result = HumlParser::new("/nonexistent/path".into());
        assert!(matches!(result, Err(HumlError::ExecutableNotFound(_))));
    }
}
