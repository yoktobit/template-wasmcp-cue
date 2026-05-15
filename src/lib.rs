//! template-wasmcp-cue-new Tools Capability Provider
//!
//! A tools capability that provides basic arithmetic operations.

mod bindings {
    wit_bindgen::generate!({
        world: "template-wasmcp-cue-new",
        generate_all,
    });
}

mod json_bindings {
    typify::import_types!(schema = "_allSchemas.schema.json", struct_builder = true);
}

use bindings::exports::wasmcp::mcp_v20251125::tools::Guest;
use bindings::wasmcp::mcp_v20251125::mcp::*;
use bindings::wasmcp::mcp_v20251125::server_handler::MessageContext;

mod implementation;
mod mcp;
mod tool_calls;
mod tools;
struct Calculator;

impl Guest for Calculator {
    fn list_tools(
        _ctx: MessageContext,
        _request: ListToolsRequest,
    ) -> Result<ListToolsResult, ErrorCode> {
        Ok(ListToolsResult {
            tools: tools::TOOLS.clone(),
            next_cursor: None,
            meta: None,
        })
    }

    fn call_tool(
        _ctx: MessageContext,
        request: CallToolRequest,
    ) -> Result<Option<CallToolResult>, ErrorCode> {
        tool_calls::tool_calls(request)
    }
}

bindings::export!(Calculator with_types_in bindings);
