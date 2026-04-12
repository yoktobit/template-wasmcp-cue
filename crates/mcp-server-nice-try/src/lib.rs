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
use serde::de::DeserializeOwned;

const INPUT_SCHEMA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../_all_schemas.schema.json"
));

mod tool_dispatch {
    use super::bindings::wasmcp::mcp_v20251125::mcp::CallToolResult;
    use super::{component_api, parse_component_input, serialize_output_for_schema, success_result};

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

fn parse_component_input<T: DeserializeOwned>(arguments: &str) -> Result<T, String> {
    let mut value: serde_json::Value =
        serde_json::from_str(arguments).map_err(|error| format!("invalid JSON arguments: {error}"))?;
    normalize_json_keys_for_wit(&mut value);
    serde_json::from_value(value)
        .map_err(|error| format!("arguments do not match component input type: {error}"))
}

fn serialize_output_for_schema<T: serde::Serialize>(output: T, schema_name: &str) -> Result<String, String> {
    let mut value = serde_json::to_value(output)
        .map_err(|error| format!("serialization failed: {error}"))?;

    if let Some(schema) = lookup_schema_value(schema_name) {
        remap_output_keys_to_schema(&mut value, &schema);
    }

    serde_json::to_string(&value).map_err(|error| format!("serialization failed: {error}"))
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

fn lookup_schema_value(schema_name: &str) -> Option<serde_json::Value> {
    let root: serde_json::Value = serde_json::from_str(INPUT_SCHEMA).ok()?;
    root.get("properties")
        .and_then(serde_json::Value::as_object)
        .and_then(|properties| properties.get(schema_name))
        .cloned()
}

fn to_snake_case_key(name: &str) -> String {
    let mut out = String::new();
    let mut last_was_separator = false;

    for (index, ch) in name.chars().enumerate() {
        if ch.is_ascii_alphanumeric() {
            if ch.is_ascii_uppercase() {
                if index > 0 && !last_was_separator && !out.ends_with('_') {
                    out.push('_');
                }
                out.push(ch.to_ascii_lowercase());
            } else {
                out.push(ch.to_ascii_lowercase());
            }
            last_was_separator = false;
        } else if !out.is_empty() && !last_was_separator {
            out.push('_');
            last_was_separator = true;
        }
    }

    out.trim_matches('_').to_string()
}

fn normalize_json_keys_for_wit(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            let old = std::mem::take(map);
            for (key, mut nested) in old {
                normalize_json_keys_for_wit(&mut nested);
                let normalized = to_snake_case_key(&key);
                map.insert(normalized, nested);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                normalize_json_keys_for_wit(item);
            }
        }
        serde_json::Value::Null
        | serde_json::Value::Bool(_)
        | serde_json::Value::Number(_)
        | serde_json::Value::String(_) => {}
    }
}

fn remap_output_keys_to_schema(value: &mut serde_json::Value, schema: &serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            let properties = schema
                .get("properties")
                .and_then(serde_json::Value::as_object);

            if let Some(properties) = properties {
                let mut canonical_to_schema = std::collections::BTreeMap::new();
                for schema_key in properties.keys() {
                    canonical_to_schema.insert(to_snake_case_key(schema_key), schema_key.clone());
                }

                let old = std::mem::take(map);
                for (key, mut nested) in old {
                    let canonical = to_snake_case_key(&key);
                    let remapped_key = canonical_to_schema
                        .get(&canonical)
                        .cloned()
                        .unwrap_or(key.clone());

                    if let Some(nested_schema) = properties.get(&remapped_key) {
                        remap_output_keys_to_schema(&mut nested, nested_schema);
                    } else {
                        remap_output_keys_to_schema(&mut nested, &serde_json::Value::Null);
                    }

                    map.insert(remapped_key, nested);
                }
            } else {
                for nested in map.values_mut() {
                    remap_output_keys_to_schema(nested, &serde_json::Value::Null);
                }
            }
        }
        serde_json::Value::Array(items) => {
            let item_schema = schema.get("items").unwrap_or(&serde_json::Value::Null);
            for item in items {
                remap_output_keys_to_schema(item, item_schema);
            }
        }
        serde_json::Value::Null
        | serde_json::Value::Bool(_)
        | serde_json::Value::Number(_)
        | serde_json::Value::String(_) => {}
    }
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
