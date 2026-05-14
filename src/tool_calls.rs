// this file is generated
// it calls the tool functions
use crate::{
    bindings::wasmcp::mcp_v20251125::{
        mcp::{CallToolRequest, CallToolResult},
        server_handler::ErrorCode,
    },
    implementation,
};

pub trait Tools {fn greeter(request: CallToolRequest) -> Result<Option<CallToolResult>, ErrorCode>;}

pub fn tool_calls(request: CallToolRequest) -> Result<Option<CallToolResult>, ErrorCode> {
    match request.name.as_str() {        "greeter" => implementation::ToolImplementation::greeter(request),        _ => Ok(None), // We don't handle this tool
    }
}