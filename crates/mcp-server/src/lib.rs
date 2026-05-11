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
use jsonschema_to_wit::{normalize_json_keys_to_snake, remap_json_keys_to_schema};
use serde::de::DeserializeOwned;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

const INPUT_SCHEMA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../_all_schemas.schema.json"
));

mod tool_dispatch {
    use super::bindings::wasmcp::mcp_v20251125::mcp::CallToolResult;
    use super::{
        component_api, parse_component_input, serialize_output_for_schema, success_result,
    };

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
    let mut value: serde_json::Value = serde_json::from_str(arguments)
        .map_err(|error| format!("invalid JSON arguments: {error}"))?;
    normalize_json_keys_to_snake(&mut value);
    serde_json::from_value(value)
        .map_err(|error| format!("arguments do not match component input type: {error}"))
}

fn serialize_output_for_schema<T: serde::Serialize>(
    output: T,
    schema_name: &str,
) -> Result<String, String> {
    let mut value =
        serde_json::to_value(output).map_err(|error| format!("serialization failed: {error}"))?;

    if let Some(schema) = lookup_schema_value(schema_name) {
        remap_json_keys_to_schema(&mut value, &schema);
    }

    serde_json::to_string(&value).map_err(|error| format!("serialization failed: {error}"))
}

fn build_tools() -> Vec<Tool> {
    tool_constants::TOOL_SPECS
        .iter()
        .map(|tool| Tool {
            name: tool.name.to_string(),
            input_schema: lookup_schema(tool.input_schema_name).unwrap_or_else(|| "{}".to_string()),
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

fn all_schemas() -> &'static serde_json::Value {
    static ALL_SCHEMAS: OnceLock<serde_json::Value> = OnceLock::new();
    ALL_SCHEMAS.get_or_init(|| {
        serde_json::from_str(INPUT_SCHEMA).unwrap_or_else(|error| {
            panic!("failed to parse embedded _all_schemas.schema.json: {error}")
        })
    })
}

fn percent_decode(input: &str) -> String {
    fn hex(byte: u8) -> Option<u8> {
        match byte {
            b'0'..=b'9' => Some(byte - b'0'),
            b'a'..=b'f' => Some(byte - b'a' + 10),
            b'A'..=b'F' => Some(byte - b'A' + 10),
            _ => None,
        }
    }

    let bytes = input.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let (Some(hi), Some(lo)) = (hex(bytes[index + 1]), hex(bytes[index + 2])) {
                output.push((hi << 4) | lo);
                index += 3;
                continue;
            }
        }

        output.push(bytes[index]);
        index += 1;
    }

    String::from_utf8_lossy(&output).to_string()
}

fn decode_ref_name(reference: &str) -> String {
    let tail = reference.rsplit('/').next().unwrap_or(reference);
    percent_decode(tail).trim_start_matches('#').to_string()
}

fn decode_pointer_token(token: &str) -> String {
    percent_decode(token).replace("~1", "/").replace("~0", "~")
}

fn resolve_json_pointer<'a>(
    schema: &'a serde_json::Value,
    pointer: &str,
) -> Option<&'a serde_json::Value> {
    let mut current = schema;

    for token in pointer.split('/').skip(1) {
        let token = decode_pointer_token(token);
        current = match current {
            serde_json::Value::Object(map) => map.get(&token)?,
            serde_json::Value::Array(items) => {
                let index = token.parse::<usize>().ok()?;
                items.get(index)?
            }
            serde_json::Value::Null
            | serde_json::Value::Bool(_)
            | serde_json::Value::Number(_)
            | serde_json::Value::String(_) => return None,
        };
    }

    Some(current)
}

fn insert_named_schemas(
    target: &mut BTreeMap<String, serde_json::Value>,
    source: Option<&serde_json::Map<String, serde_json::Value>>,
) {
    let Some(source) = source else {
        return;
    };

    for (key, value) in source {
        let canonical = decode_ref_name(key);
        target.entry(canonical).or_insert_with(|| value.clone());
    }
}

fn named_schemas() -> &'static BTreeMap<String, serde_json::Value> {
    static NAMED_SCHEMAS: OnceLock<BTreeMap<String, serde_json::Value>> = OnceLock::new();
    NAMED_SCHEMAS.get_or_init(|| {
        let mut out = BTreeMap::new();
        let schema = all_schemas();
        insert_named_schemas(
            &mut out,
            schema
                .get("properties")
                .and_then(serde_json::Value::as_object),
        );
        insert_named_schemas(
            &mut out,
            schema.get("$defs").and_then(serde_json::Value::as_object),
        );
        insert_named_schemas(
            &mut out,
            schema
                .get("definitions")
                .and_then(serde_json::Value::as_object),
        );
        out
    })
}

fn resolve_reference_value(reference: &str) -> Option<serde_json::Value> {
    if let Some(pointer) = reference.strip_prefix('#') {
        if pointer.is_empty() {
            return Some(all_schemas().clone());
        }

        if pointer.starts_with('/') {
            if let Some(value) = resolve_json_pointer(all_schemas(), pointer) {
                return Some(value.clone());
            }
        }
    }

    let def_name = decode_ref_name(reference);
    named_schemas().get(&def_name).cloned()
}

fn lookup_schema_value(schema_name: &str) -> Option<serde_json::Value> {
    let lookup_name = decode_ref_name(schema_name);
    let mut schema = named_schemas().get(&lookup_name).cloned()?;
    let mut seen_refs = BTreeSet::new();

    loop {
        let Some(reference) = schema.get("$ref").and_then(serde_json::Value::as_str) else {
            break;
        };

        let is_direct_ref = schema
            .as_object()
            .map(|map| map.len() == 1)
            .unwrap_or(false);
        if !is_direct_ref {
            break;
        }

        if !seen_refs.insert(reference.to_string()) {
            break;
        }

        let resolved = resolve_reference_value(reference)?;
        schema = resolved;
    }

    Some(schema)
}

fn lookup_schema(schema_name: &str) -> Option<String> {
    lookup_schema_value(schema_name).and_then(|schema| serde_json::to_string(&schema).ok())
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
