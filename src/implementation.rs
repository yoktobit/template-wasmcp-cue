use crate::{
    bindings::wasmcp::mcp_v20251125::{
        mcp::{CallToolRequest, CallToolResult},
        server_handler::ErrorCode,
    },
    json_bindings::TestInput,
    mcp,
    tool_calls::Tools,
};

pub struct ToolImplementation;

impl Tools for ToolImplementation {
    fn greeter(request: CallToolRequest) -> Result<Option<CallToolResult>, ErrorCode> {
        match parse_args(request.arguments) {
            Ok(test_input) => {
                let result = format!(
                    "Hello, {}! How is your {}?",
                    test_input.name, test_input.pet
                );
                Ok(Some(mcp::success_result(result.to_string())))
            }
            Err(msg) => Ok(Some(mcp::error_result(msg))),
        }
    }
}

fn parse_args(arguments: Option<String>) -> Result<TestInput, String> {
    let arguments = arguments.ok_or_else(|| "no arguments given".to_string())?;
    serde_json::from_str::<TestInput>(arguments.as_ref()).map_err(|e| e.to_string())
}
