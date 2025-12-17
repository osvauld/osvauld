//! CEL Runtime - OCaml CEL Evaluator Integration
//!
//! Provides a Rust wrapper around the OCaml CEL evaluator executable.
//! Communication happens via stdin/stdout with JSON protocol.
//!
//! **Protocol:**
//! - Request: `{"cmd": "evaluate"|"interpolate", "expr"|"template": "...", "context": {...}}`
//! - Response: `{"success": true, "result": ...}` or `{"success": false, "error": "..."}`

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CelError {
    #[error("CEL evaluation failed: {0}")]
    Evaluation(String),

    #[error("CEL interpolation failed: {0}")]
    Interpolation(String),

    #[error("CEL dependency extraction failed: {0}")]
    DependencyExtraction(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("CEL executable not found at: {0}")]
    ExecutableNotFound(PathBuf),

    #[error("CEL process failed: {0}")]
    ProcessFailed(String),
}

#[derive(Serialize)]
struct EvalRequest<'a> {
    cmd: &'a str,
    expr: &'a str,
    context: &'a Value,
}

#[derive(Serialize)]
struct InterpolateRequest<'a> {
    cmd: &'a str,
    template: &'a str,
    context: &'a Value,
}

#[derive(Serialize)]
struct ExtractDepsRequest<'a> {
    cmd: &'a str,
    expr: &'a str,
}

#[derive(Serialize)]
struct ExtractTemplateDepsRequest<'a> {
    cmd: &'a str,
    template: &'a str,
}

#[derive(Deserialize)]
struct CelResponse {
    success: bool,
    result: Option<Value>,
    error: Option<String>,
}

#[derive(Deserialize)]
struct DepsResponse {
    success: bool,
    deps: Option<Vec<String>>,
    error: Option<String>,
}

/// CEL Evaluator that wraps the OCaml executable.
///
/// The evaluator spawns the OCaml process for each evaluation call.
/// This ensures clean state between evaluations and simplifies error handling.
#[derive(Debug, Clone)]
pub struct CelEvaluator {
    executable_path: PathBuf,
}

impl CelEvaluator {
    /// Create a new CEL evaluator with the given executable path.
    ///
    /// Returns error if the executable does not exist.
    pub fn new(executable_path: PathBuf) -> Result<Self, CelError> {
        if !executable_path.exists() {
            return Err(CelError::ExecutableNotFound(executable_path));
        }
        Ok(Self { executable_path })
    }

    /// Evaluate a CEL expression with the given context.
    ///
    /// **Expression format:** Can be wrapped in `${ }` or plain.
    /// **Context:** JSON object with variable bindings.
    ///
    /// # Examples
    /// ```ignore
    /// let result = evaluator.evaluate("x + y", &json!({"x": 5, "y": 3}))?;
    /// assert_eq!(result, json!(8));
    /// ```
    pub fn evaluate(&self, expr: &str, context: &Value) -> Result<Value, CelError> {
        let request = EvalRequest {
            cmd: "evaluate",
            expr,
            context,
        };
        let input = serde_json::to_string(&request)?;
        let output = self.run_process(&input)?;
        self.parse_response(&output, "evaluate")
    }

    /// Interpolate `{{ expr }}` patterns in a template string.
    ///
    /// Replaces all `{{ expression }}` patterns with evaluated results.
    ///
    /// # Examples
    /// ```ignore
    /// let result = evaluator.interpolate("Hello, {{name}}!", &json!({"name": "World"}))?;
    /// assert_eq!(result, "Hello, World!");
    /// ```
    pub fn interpolate(&self, template: &str, context: &Value) -> Result<String, CelError> {
        let request = InterpolateRequest {
            cmd: "interpolate",
            template,
            context,
        };
        let input = serde_json::to_string(&request)?;
        let output = self.run_process(&input)?;

        let value = self.parse_response(&output, "interpolate")?;
        value
            .as_str()
            .map(String::from)
            .ok_or_else(|| CelError::Interpolation("Expected string result".to_string()))
    }

    /// Extract variable dependencies from a CEL expression.
    ///
    /// Returns a list of variable names that the expression depends on.
    /// Lambda parameters are excluded (they're local bindings).
    ///
    /// # Examples
    /// ```ignore
    /// let deps = evaluator.extract_deps("${ count + x * y }")?;
    /// assert_eq!(deps, vec!["count", "x", "y"]);
    /// ```
    pub fn extract_deps(&self, expr: &str) -> Result<Vec<String>, CelError> {
        let request = ExtractDepsRequest {
            cmd: "extract_deps",
            expr,
        };
        let input = serde_json::to_string(&request)?;
        let output = self.run_process(&input)?;
        self.parse_deps_response(&output)
    }

    /// Extract variable dependencies from a template with `{{ expr }}` patterns.
    ///
    /// Returns a combined list of all variable names referenced in the template.
    ///
    /// # Examples
    /// ```ignore
    /// let deps = evaluator.extract_template_deps("Hello {{name}}, count: {{count}}")?;
    /// assert_eq!(deps, vec!["count", "name"]);
    /// ```
    pub fn extract_template_deps(&self, template: &str) -> Result<Vec<String>, CelError> {
        let request = ExtractTemplateDepsRequest {
            cmd: "extract_template_deps",
            template,
        };
        let input = serde_json::to_string(&request)?;
        let output = self.run_process(&input)?;
        self.parse_deps_response(&output)
    }

    /// Run the CEL process with the given JSON input.
    fn run_process(&self, input: &str) -> Result<String, CelError> {
        let mut child = Command::new(&self.executable_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        // Write input to stdin
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(input.as_bytes())?;
        }

        // Wait for completion and capture output
        let output = child.wait_with_output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(CelError::ProcessFailed(format!(
                "Exit code: {:?}, stderr: {}",
                output.status.code(),
                stderr
            )));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Parse the JSON response from the CEL process.
    fn parse_response(&self, output: &str, operation: &str) -> Result<Value, CelError> {
        let response: CelResponse = serde_json::from_str(output)?;

        if response.success {
            response.result.ok_or_else(|| {
                if operation == "evaluate" {
                    CelError::Evaluation("No result in response".to_string())
                } else {
                    CelError::Interpolation("No result in response".to_string())
                }
            })
        } else {
            let error_msg = response.error.unwrap_or_else(|| "Unknown error".to_string());
            if operation == "evaluate" {
                Err(CelError::Evaluation(error_msg))
            } else {
                Err(CelError::Interpolation(error_msg))
            }
        }
    }

    /// Parse the JSON response for dependency extraction.
    fn parse_deps_response(&self, output: &str) -> Result<Vec<String>, CelError> {
        let response: DepsResponse = serde_json::from_str(output)?;

        if response.success {
            response
                .deps
                .ok_or_else(|| CelError::DependencyExtraction("No deps in response".to_string()))
        } else {
            let error_msg = response.error.unwrap_or_else(|| "Unknown error".to_string());
            Err(CelError::DependencyExtraction(error_msg))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn get_test_evaluator() -> Option<CelEvaluator> {
        // Try multiple paths - workspace root and template-transpiler directory
        let paths = [
            "sthalam/template-transpiler/_build/default/eval-bin/cel_wasi.exe",
            "_build/default/eval-bin/cel_wasi.exe",
        ];

        for path in paths {
            let path = PathBuf::from(path);
            if path.exists() {
                return Some(CelEvaluator::new(path).expect("CEL executable should be valid"));
            }
        }

        // Check via environment variable
        if let Ok(path) = std::env::var("CEL_EXECUTABLE") {
            let path = PathBuf::from(path);
            if path.exists() {
                return Some(CelEvaluator::new(path).expect("CEL executable should be valid"));
            }
        }

        None
    }

    macro_rules! skip_if_no_evaluator {
        ($eval:ident) => {
            let Some($eval) = get_test_evaluator() else {
                eprintln!("Skipping test: CEL executable not found. Set CEL_EXECUTABLE env var or build from template-transpiler.");
                return;
            };
        };
    }

    #[test]
    fn test_simple_arithmetic() {
        skip_if_no_evaluator!(eval);
        let result = eval.evaluate("1 + 1", &json!({})).unwrap();
        assert_eq!(result, json!(2));
    }

    #[test]
    fn test_expression_with_context() {
        skip_if_no_evaluator!(eval);
        let result = eval.evaluate("x + y", &json!({"x": 5, "y": 3})).unwrap();
        assert_eq!(result, json!(8));
    }

    #[test]
    fn test_wrapped_expression() {
        skip_if_no_evaluator!(eval);
        let result = eval.evaluate("${x * 2}", &json!({"x": 10})).unwrap();
        assert_eq!(result, json!(20));
    }

    #[test]
    fn test_size_function() {
        skip_if_no_evaluator!(eval);
        let result = eval
            .evaluate("size(items)", &json!({"items": [1, 2, 3]}))
            .unwrap();
        assert_eq!(result, json!(3));
    }

    #[test]
    fn test_map_access() {
        skip_if_no_evaluator!(eval);
        let result = eval
            .evaluate("user.name", &json!({"user": {"name": "Alice"}}))
            .unwrap();
        assert_eq!(result, json!("Alice"));
    }

    #[test]
    fn test_interpolation() {
        skip_if_no_evaluator!(eval);
        let result = eval
            .interpolate("Hello, {{name}}!", &json!({"name": "World"}))
            .unwrap();
        assert_eq!(result, "Hello, World!");
    }

    #[test]
    fn test_interpolation_multiple() {
        skip_if_no_evaluator!(eval);
        let result = eval
            .interpolate(
                "{{greeting}}, {{name}}! You have {{count}} messages.",
                &json!({"greeting": "Hi", "name": "Alice", "count": 5}),
            )
            .unwrap();
        assert_eq!(result, "Hi, Alice! You have 5 messages.");
    }

    #[test]
    fn test_boolean_expression() {
        skip_if_no_evaluator!(eval);
        let result = eval
            .evaluate("size(items) > 0", &json!({"items": [1, 2]}))
            .unwrap();
        assert_eq!(result, json!(true));
    }

    #[test]
    fn test_error_handling() {
        skip_if_no_evaluator!(eval);
        let result = eval.evaluate("undefined_var", &json!({}));
        assert!(result.is_err());
        match result {
            Err(CelError::Evaluation(msg)) => {
                assert!(msg.contains("Unknown identifier"));
            }
            _ => panic!("Expected evaluation error"),
        }
    }

    #[test]
    fn test_extract_deps_simple() {
        skip_if_no_evaluator!(eval);
        let deps = eval.extract_deps("${ count + x * y }").unwrap();
        assert_eq!(deps, vec!["count", "x", "y"]);
    }

    #[test]
    fn test_extract_deps_member_access() {
        skip_if_no_evaluator!(eval);
        let deps = eval.extract_deps("${ user.name + items.size() }").unwrap();
        assert_eq!(deps, vec!["items", "user"]);
    }

    #[test]
    fn test_extract_deps_lambda() {
        skip_if_no_evaluator!(eval);
        // Lambda parameter 'x' should not be in deps
        let deps = eval
            .extract_deps("${ items.filter(x => x.active) }")
            .unwrap();
        assert_eq!(deps, vec!["items"]);
    }

    #[test]
    fn test_extract_deps_ternary() {
        skip_if_no_evaluator!(eval);
        let deps = eval
            .extract_deps("${ isPositive ? messages : errors }")
            .unwrap();
        assert_eq!(deps, vec!["errors", "isPositive", "messages"]);
    }

    #[test]
    fn test_extract_template_deps() {
        skip_if_no_evaluator!(eval);
        let deps = eval
            .extract_template_deps("Hello {{name}}, you have {{count}} messages from {{sender}}")
            .unwrap();
        assert_eq!(deps, vec!["count", "name", "sender"]);
    }

    #[test]
    fn test_extract_template_deps_complex() {
        skip_if_no_evaluator!(eval);
        let deps = eval
            .extract_template_deps(r#"Count: {{count}}, Status: {{isActive ? "on" : "off"}}"#)
            .unwrap();
        assert_eq!(deps, vec!["count", "isActive"]);
    }
}
