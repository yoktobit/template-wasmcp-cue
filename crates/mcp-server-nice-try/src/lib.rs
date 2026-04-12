mod bindings {
    wit_bindgen::generate!({
        path: "wit",
        world: "acmeapp",
        generate_all,
    });
}

mod component_api_bindings {
    wit_bindgen::generate!({
        path: "wit/component-client",
        world: "component-client",
        generate_all,
        additional_derives: [serde::Serialize, serde::Deserialize],
    });
}

mod tool_constants {
    include!(concat!(env!("OUT_DIR"), "/tool_constants.rs"));
}

use bindings::exports::wasmcp::mcp_v20251125::tools::Guest;
use bindings::wasmcp::mcp_v20251125::mcp::*;
use bindings::wasmcp::mcp_v20251125::server_handler::MessageContext;
use component_api_bindings::acme::app::api as component_api;

const INPUT_SCHEMA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../_input.schema.json"
));
const OUTPUT_SCHEMA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../_output.schema.json"
));

struct AcmeTools;

impl Guest for AcmeTools {
    fn list_tools(
        _ctx: MessageContext,
        _request: ListToolsRequest,
    ) -> Result<ListToolsResult, ErrorCode> {
        Ok(ListToolsResult {
            tools: build_tools(),
            next_cursor: None,
            meta: None,
        })
    }

    fn call_tool(
        _ctx: MessageContext,
        request: CallToolRequest,
    ) -> Result<Option<CallToolResult>, ErrorCode> {
        if !tool_constants::TOOL_SPECS
            .iter()
            .any(|tool| request.name == tool.name)
        {
            return Ok(None);
        }

        let args = request.arguments.as_deref().unwrap_or("{}");
        let result = match parse_input(args) {
            Ok(input) => {
                let output = component_api::run(&input);
                success_result(serialize_output(output))
            }
            Err(error) => error_result(format!("Invalid input: {error}")),
        };

        Ok(Some(result))
    }
}

fn parse_input(arguments: &str) -> Result<component_api::Input, String> {
    serde_json::from_str(arguments).map_err(|error| format!("invalid JSON arguments: {error}"))
}

fn serialize_output(output: component_api::Output) -> String {
    serde_json::to_string(&output)
        .unwrap_or_else(|error| format!("{{\"error\":\"serialization failed: {error}\"}}"))
}

fn build_tools() -> Vec<Tool> {
    tool_constants::TOOL_SPECS
        .iter()
        .map(|tool| Tool {
            name: tool.name.to_string(),
            input_schema: INPUT_SCHEMA.to_string(),
            options: Some(ToolOptions {
                meta: None,
                icons: None,
                annotations: None,
                description: Some(tool.description.to_string()),
                output_schema: Some(OUTPUT_SCHEMA.to_string()),
                title: Some(tool.title.to_string()),
            }),
        })
        .collect()
}

fn success_result(text: String) -> CallToolResult {
    CallToolResult {
        content: vec![ContentBlock::Text(TextContent {
            text: TextData::Text(text.clone()),
            options: None,
        })],
        is_error: None,
        meta: None,
        structured_content: Some(text),
    }
}

fn error_result(message: String) -> CallToolResult {
    CallToolResult {
        content: vec![ContentBlock::Text(TextContent {
            text: TextData::Text(message),
            options: None,
        })],
        is_error: Some(true),
        meta: None,
        structured_content: None,
    }
}

bindings::export!(AcmeTools with_types_in bindings);
