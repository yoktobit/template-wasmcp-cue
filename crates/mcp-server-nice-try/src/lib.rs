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
    "/../../_all_schemas.schema.json"
));

mod tool_dispatch {
    use super::bindings::wasmcp::mcp_v20251125::mcp::CallToolResult;
    use super::{component_api, serialize_output, success_result};

    include!(concat!(env!("OUT_DIR"), "/tool_dispatch.rs"));
}

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
        let result = match tool_dispatch::call_component_tool(&request.name, args) {
            Ok(result) => result,
            Err(error) => error_result(format!("Invalid input: {error}")),
        };

        Ok(Some(result))
    }
}

fn serialize_output<T: serde::Serialize>(output: T) -> String {
    serde_json::to_string(&output)
        .unwrap_or_else(|error| format!("{{\"error\":\"serialization failed: {error}\"}}"))
}

fn build_tools() -> Vec<Tool> {
    tool_constants::TOOL_SPECS
        .iter()
        .map(|tool| Tool {
            name: tool.name.to_string(),
            input_schema: lookup_schema(tool.input_schema_name)
                .unwrap_or_else(|| "{}".to_string()),
            options: Some(ToolOptions {
                meta: None,
                icons: None,
                annotations: None,
                description: Some(tool.description.to_string()),
                output_schema: Some(
                    lookup_schema(tool.output_schema_name).unwrap_or_else(|| "{}".to_string()),
                ),
                title: Some(tool.title.to_string()),
            }),
        })
        .collect()
}

fn lookup_schema(schema_name: &str) -> Option<String> {
    let root: serde_json::Value = serde_json::from_str(INPUT_SCHEMA).ok()?;
    root.get("properties")
        .and_then(serde_json::Value::as_object)
        .and_then(|properties| properties.get(schema_name))
        .and_then(|schema| serde_json::to_string(schema).ok())
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
