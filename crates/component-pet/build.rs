use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[path = "../build-common.rs"]
#[allow(dead_code)]
mod build_common;

use jsonschema_to_wit::{
    generate_wit_for_tools_from_file, normalize_trivial_aliases, ToolFunction, WitConfig,
};
use serde_json::Value;

fn normalize_schema_aliases(schema_path: &Path) {
    let mut schema: Value = serde_json::from_str(
        &fs::read_to_string(schema_path)
            .unwrap_or_else(|e| panic!("failed to read {}: {e}", schema_path.display())),
    )
    .unwrap_or_else(|e| panic!("failed to parse {}: {e}", schema_path.display()));

    normalize_trivial_aliases(&mut schema);

    let normalized = serde_json::to_string_pretty(&schema)
        .unwrap_or_else(|e| panic!("failed to serialize {}: {e}", schema_path.display()));
    build_common::write_if_changed(schema_path, &(normalized + "\n"));
}

fn generate_component_wit(
    manifest_dir: &Path,
    all_schemas_path: &Path,
    tool_bindings: &[build_common::ToolBinding],
    handler: &build_common::ToolHandler,
) {
    let wit_path = manifest_dir.join("wit/world.wit");
    let version = env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "0.1.0".to_string());
    let tool_functions = tool_bindings
        .iter()
        .map(|tool| ToolFunction {
            name: tool.tool_name.clone(),
            input_schema_name: tool.input_schema_name.clone(),
            output_schema_name: tool.output_schema_name.clone(),
        })
        .collect::<Vec<_>>();

    let wit = generate_wit_for_tools_from_file(
        all_schemas_path,
        &tool_functions,
        &WitConfig {
            package: handler.package_ref(),
            version,
            interface: handler.interface.clone(),
            world: "component".to_string(),
            function: "run".to_string(),
        },
    )
    .unwrap_or_else(|e| panic!("failed to generate {}: {e}", wit_path.display()));

    build_common::write_if_changed(&wit_path, &wit);
}

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("missing manifest dir"));
    let workspace_dir = manifest_dir.join("../..");
    let all_schemas_path = workspace_dir.join("_all_schemas.schema.json");

    println!(
        "cargo:rerun-if-changed={}",
        workspace_dir.join("schema.cue").display()
    );
    println!("cargo:rerun-if-env-changed=ACME_COMPONENT_PET_HANDLER");

    let tools: Value = build_common::cue_export_json(&workspace_dir, "McpTools");
    let all_tool_bindings = build_common::tool_bindings_for_component(&tools);
    let handler_ref = env::var("ACME_COMPONENT_PET_HANDLER")
        .unwrap_or_else(|_| "acme:pet/api@0.1.0".to_string());
    let handler = build_common::parse_tool_handler(&handler_ref);
    let tool_bindings = all_tool_bindings
        .into_iter()
        .filter(|tool| {
            let tool_handler = build_common::parse_tool_handler(&tool.handler);
            tool_handler.namespace == handler.namespace
                && tool_handler.package == handler.package
                && tool_handler.interface == handler.interface
        })
        .collect::<Vec<_>>();

    assert!(
        !tool_bindings.is_empty(),
        "no tools matched ACME_COMPONENT_PET_HANDLER={handler_ref}; update schema.cue or environment"
    );

    build_common::run_cue(
        &workspace_dir,
        "def",
        "Schemas",
        "jsonschema",
        "_all_schemas.schema.json",
    );
    normalize_schema_aliases(&all_schemas_path);
    generate_component_wit(&manifest_dir, &all_schemas_path, &tool_bindings, &handler);
}
