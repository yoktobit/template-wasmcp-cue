use std::env;
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
        source.push_str(&format!(
            "        \"{}\" => {{\n            let input: component_api::{} = parse_component_input(args)?;\n            let output = component_api::{}(&input);\n            let output_json = serialize_output_for_schema(output, \"{}\")?;\n            Ok(success_result(output_json))\n        }}\n",
            tool.tool_name, input_type, function_name, tool.output_schema_name
        ));
    }

    source.push_str(
        "        _ => Err(format!(\"tool `{tool_name}` has no generated component binding\")),\n    }\n}\n",
    );

    build_common::write_if_changed(destination, &source);
}

fn main() {
    println!("cargo:rerun-if-changed=../../schema.cue");
    println!("cargo:rerun-if-changed=../component/wit/world.wit");

    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("missing CARGO_MANIFEST_DIR");
    let workspace_dir = Path::new(&manifest_dir).join("../..");
    let out_dir = env::var("OUT_DIR").expect("missing OUT_DIR");
    let tools = build_common::cue_export_json(&workspace_dir, "McpTools");
    let bindings = build_common::tool_bindings_for_component(&tools);

    build_common::run_cue(
        &workspace_dir,
        "def",
        "Schemas",
        "jsonschema",
        "_all_schemas.schema.json",
    );
    build_common::copy_if_changed("../component/wit/world.wit", "wit/deps/acme-app.wit");
    build_common::copy_if_changed(
        "../component/wit/world.wit",
        "wit/component-client/deps/acme-app.wit",
    );
    build_common::export_tool_constants(
        &workspace_dir,
        &Path::new(&out_dir).join("tool_constants.rs"),
    );
    generate_tool_dispatch(&Path::new(&out_dir).join("tool_dispatch.rs"), &bindings);
}
