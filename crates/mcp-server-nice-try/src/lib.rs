mod bindings {
    wit_bindgen::generate!({
        path: "wit",
        world: "acmeapp",
        generate_all,
    });
}

mod tool_constants {
    include!(concat!(env!("OUT_DIR"), "/tool_constants.rs"));
}

use bindings::acme::app::api;
use bindings::exports::wasmcp::mcp_v20251125::tools::Guest;
use bindings::wasmcp::mcp_v20251125::mcp::*;
use bindings::wasmcp::mcp_v20251125::server_handler::MessageContext;

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
        if !tool_constants::TOOL_NAMES
            .iter()
            .any(|name| request.name == *name)
        {
            return Ok(None);
        }

        let args = request.arguments.as_deref().unwrap_or("{}");
        let result = match parse_input(args) {
            Ok(input) => {
                let output = api::run(&input);
                success_result(serialize_output(output))
            }
            Err(error) => error_result(format!("Invalid input: {error}")),
        };

        Ok(Some(result))
    }
}

fn parse_input(arguments: &str) -> Result<api::Input, String> {
    let json: serde_json::Value = serde_json::from_str(arguments)
        .map_err(|error| format!("invalid JSON arguments: {error}"))?;

    Ok(api::Input {
        name: required_string(&json, "name")?,
        forename: required_string(&json, "forename")?,
        birthdate: required_string(&json, "birthdate")?,
        wish: required_string(&json, "wish")?,
    })
}

fn required_string(json: &serde_json::Value, field: &str) -> Result<String, String> {
    json.get(field)
        .and_then(serde_json::Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| format!("missing or invalid parameter `{field}`"))
}

fn serialize_output(output: api::Output) -> String {
    serde_json::json!({
        "NiceMessage": output.nice_message,
        "NotsoNiceMessage": output.notso_nice_message,
    })
    .to_string()
}

fn build_tools() -> Vec<Tool> {
    tool_constants::TOOL_NAMES
        .iter()
        .zip(tool_constants::TOOL_TITLES.iter())
        .zip(tool_constants::TOOL_DESCRIPTIONS.iter())
        .map(|((name, title), description)| Tool {
            name: (*name).to_string(),
            input_schema: INPUT_SCHEMA.to_string(),
            options: Some(ToolOptions {
                meta: None,
                icons: None,
                annotations: None,
                description: Some((*description).to_string()),
                output_schema: Some(OUTPUT_SCHEMA.to_string()),
                title: Some((*title).to_string()),
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
