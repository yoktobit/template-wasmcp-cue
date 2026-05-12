use crate::bindings::wasmcp::mcp_v20251125::mcp::{Tool, ToolOptions};
use once_cell::sync::Lazy;

pub static TOOLS: Lazy<Vec<Tool>> = Lazy::new(|| vec![Tool {
    name: "greeter".to_string(),
    input_schema: r###"{
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "$defs": {
        "#Schema": {
            "const": {}
        }
    },
    "type": "object",
    "additionalProperties": true,
    "properties": {
        "name": {
            "type": "string"
        },
        "pet": {
            "enum": [
                "dog",
                "cat"
            ]
        }
    },
    "$ref": "#/$defs/%23Schema",
    "required": [
        "name",
        "pet"
    ]
}
"###.to_string(),
    options: Some(ToolOptions {
        meta: None,
        icons: None,
        annotations: None,
        description: Some("Greets somebody".to_string()),
        output_schema: None,
        title: Some("greeter".to_string()),
    }),
},]);