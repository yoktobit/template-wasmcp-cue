use std::env;
use std::collections::BTreeSet;
use std::path::Path;

use jsonschema_to_wit::{to_pascal_case, to_snake_case};

#[path = "../build-common.rs"]
#[allow(dead_code)]
mod build_common;

fn generate_tool_dispatch(destination: &Path, tools: &[build_common::ToolBinding]) {
    let mut source = String::from(
        "// Generated from `schema.cue` by `build.rs`. Do not edit manually.\n",
    );
    source.push_str(
        "pub fn call_component_tool(\n    tool_name: &str,\n    args: &str,\n) -> Result<CallToolResult, String> {\n    match tool_name {\n",
    );

    for tool in tools {
        let function_name = to_snake_case(&tool.tool_name);
        let input_type = to_pascal_case(&tool.input_schema_name);
        let handler = build_common::parse_tool_handler(&tool.handler);
        let handler_path = format!("component_api_bindings::{}", handler.rust_module_path());
        source.push_str(&format!(
            "        \"{}\" => {{\n            let input: {}::{} = parse_component_input(args)?;\n            let output = {}::{}(&input);\n            let output_json = serialize_output_for_schema(output, \"{}\")?;\n            Ok(success_result(output_json))\n        }}\n",
            tool.tool_name,
            handler_path,
            input_type,
            handler_path,
            function_name,
            tool.output_schema_name
        ));
    }

    source.push_str(
        "        _ => Err(format!(\"tool `{tool_name}` has no generated component binding\")),\n    }\n}\n",
    );

    build_common::write_if_changed(destination, &source);
}

fn generate_mcp_world(destination: &Path, handlers: &BTreeSet<build_common::ToolHandler>, version: &str) {
    let mut source = format!(
        "package wasmcp:acmeapp@{};\n\n/// MCP adapter that delegates tool execution to reusable application components.\nworld acmeapp {{\n",
        version
    );

    for handler in handlers {
        source.push_str(&format!("    import {};\n", handler.import_ref(version)));
    }

    source.push_str("    export wasmcp:mcp-v20251125/tools@0.1.1;\n}\n");
    build_common::write_if_changed(destination, &source);
}

fn generate_component_client_world(
    destination: &Path,
    handlers: &BTreeSet<build_common::ToolHandler>,
    version: &str,
) {
    let mut source = format!(
        "package wasmcp:acmeapp-client@{};\n\nworld component-client {{\n",
        version
    );

    for handler in handlers {
        source.push_str(&format!("    import {};\n", handler.import_ref(version)));
    }

    source.push_str("}\n");
    build_common::write_if_changed(destination, &source);
}

fn main() {
    println!("cargo:rerun-if-changed=../../schema.cue");
    println!("cargo:rerun-if-changed=../component/wit/world.wit");
    println!("cargo:rerun-if-changed=../component-pet/wit/world.wit");

    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("missing CARGO_MANIFEST_DIR");
    let workspace_dir = Path::new(&manifest_dir).join("../..");
    let out_dir = env::var("OUT_DIR").expect("missing OUT_DIR");
    let version = env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "0.1.0".to_string());
    let tools = build_common::cue_export_json(&workspace_dir, "McpTools");
    let bindings = build_common::tool_bindings_for_component(&tools);
    let handlers = bindings
        .iter()
        .map(|tool| build_common::parse_tool_handler(&tool.handler))
        .collect::<BTreeSet<_>>();

    build_common::run_cue(
        &workspace_dir,
        "def",
        "Schemas",
        "jsonschema",
        "_all_schemas.schema.json",
    );
    generate_mcp_world(Path::new("wit/world.wit"), &handlers, &version);
    generate_component_client_world(
        Path::new("wit/component-client/world.wit"),
        &handlers,
        &version,
    );
    for handler in &handlers {
        if handler.namespace == "acme" && handler.package == "greeter" && handler.interface == "api" {
            build_common::copy_if_changed("../component/wit/world.wit", "wit/deps/acme-greeter.wit");
            build_common::copy_if_changed(
                "../component/wit/world.wit",
                "wit/component-client/deps/acme-greeter.wit",
            );
        }

        if handler.namespace == "acme" && handler.package == "pet" && handler.interface == "api" {
            build_common::copy_if_changed("../component-pet/wit/world.wit", "wit/deps/acme-pet.wit");
            build_common::copy_if_changed(
                "../component-pet/wit/world.wit",
                "wit/component-client/deps/acme-pet.wit",
            );
        }
    }
    build_common::export_tool_constants(
        &workspace_dir,
        &Path::new(&out_dir).join("tool_constants.rs"),
    );
    generate_tool_dispatch(&Path::new(&out_dir).join("tool_dispatch.rs"), &bindings);
}
