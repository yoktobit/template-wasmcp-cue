macro_rules! mcp_tool {
    (
        name: $name:literal,
        title: $title:literal,
        description: $desc:literal,
        input: $input:ty,
        handler: $handler:path $(,)?
    ) => {
            use $crate::bindings::exports::wasmcp::mcp_v20251125::tools::Guest;
            use $crate::bindings::wasmcp::mcp_v20251125::mcp::*;
            use $crate::bindings::wasmcp::mcp_v20251125::server_handler::MessageContext;

        mcp_tool! {
            name: $name,
            title: $title,
            description: $desc,
            input_schema: "_input.schema.json",
            input: $input,
            output_schema: "_output.schema.json",
            handler: $handler,
        }
    };
    (
        name: $name:literal,
        title: $title:literal,
        description: $desc:literal,
        input_schema: $input_schema:literal,
        input: $input:ty,
        output_schema: $output_schema:literal,
        handler: $handler:path $(,)?
    ) => {
        mod input {
            typify::import_types!(schema = $input_schema);
        }
        mod output {
            typify::import_types!(schema = $output_schema);
        }

        struct McpHandler;

        impl Guest for McpHandler {
            fn list_tools(
                _ctx: MessageContext,
                _request: ListToolsRequest,
            ) -> Result<ListToolsResult, ErrorCode> {
                Ok(ListToolsResult {
                    tools: vec![Tool {
                        name: $name.to_string(),
                        input_schema: include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/", $input_schema)).to_string(),
                        options: Some(ToolOptions {
                            meta: None,
                            icons: None,
                            annotations: None,
                            description: Some($desc.to_string()),
                            output_schema: Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/", $output_schema)).to_string()),
                            title: Some($title.to_string()),
                        }),
                    }],
                    next_cursor: None,
                    meta: None,
                })
            }

            fn call_tool(
                _ctx: MessageContext,
                request: CallToolRequest,
            ) -> Result<Option<CallToolResult>, ErrorCode> {
                Ok(if request.name == $name {
                    let args = request.arguments.as_deref().unwrap_or("{}");
                    Some(match serde_json::from_str::<$input>(args) {
                        Ok(input) => {
                            let output = $handler(input);
                            let text = serde_json::to_string(&output).unwrap_or_else(|e| format!("Serialization error: {}", e));
                            CallToolResult {
                                meta: None,
                                is_error: None,
                                structured_content: Some(text.clone()),
                                content: vec![ContentBlock::Text(TextContent {
                                    text: TextData::Text(text),
                                    options: None,
                                })],
                            }
                        },
                        Err(e) => CallToolResult {
                            meta: None,
                            is_error: Some(true),
                            structured_content: None,
                            content: vec![ContentBlock::Text(TextContent {
                                text: TextData::Text(format!("Invalid input: {}", e)),
                                options: None,
                            })],
                        },
                    })
                } else {
                    None
                })
            }
        }

        bindings::export!(McpHandler with_types_in bindings);
    };
}
