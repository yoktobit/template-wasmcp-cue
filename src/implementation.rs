use crate::{
    bindings::wasmcp::mcp_v20251125::{
        mcp::{CallToolRequest, CallToolResult},
        server_handler::ErrorCode,
    },
    mcp,
    tool_calls::Tools,
};

pub struct ToolImplementation;

impl Tools for ToolImplementation {
    fn greeter(request: CallToolRequest) -> Result<Option<CallToolResult>, ErrorCode> {
        match parse_args(&request.arguments) {
            Ok((a, b)) => {
                let result = a + b;
                Ok(Some(mcp::success_result(result.to_string())))
            }
            Err(msg) => Ok(Some(mcp::error_result(msg))),
        }
    }
}

pub fn execute_operation<F>(arguments: &Option<String>, op: F) -> CallToolResult
where
    F: FnOnce(f64, f64) -> f64,
{
    match parse_args(arguments) {
        Ok((a, b)) => {
            let result = op(a, b);
            mcp::success_result(result.to_string())
        }
        Err(msg) => mcp::error_result(msg),
    }
}

pub fn parse_args(arguments: &Option<String>) -> Result<(f64, f64), String> {
    let args_str = arguments
        .as_ref()
        .ok_or_else(|| "Missing arguments".to_string())?;

    let json: serde_json::Value =
        serde_json::from_str(args_str).map_err(|e| format!("Invalid JSON arguments: {}", e))?;

    let a = json
        .get("a")
        .and_then(|v| v.as_f64())
        .ok_or_else(|| "Missing or invalid parameter 'a'".to_string())?;

    let b = json
        .get("b")
        .and_then(|v| v.as_f64())
        .ok_or_else(|| "Missing or invalid parameter 'b'".to_string())?;

    Ok((a, b))
}
