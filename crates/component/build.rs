use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use jsonschema_to_wit::{
    generate_wit_for_tools_from_file, normalize_trivial_aliases, ToolFunction, WitConfig,
};
use serde_json::Value;

#[derive(Debug, Clone)]
struct ToolBinding {
    tool_name: String,
    input_schema_name: String,
    output_schema_name: String,
}

fn write_if_changed(path: &Path, contents: &str) {
    if fs::read_to_string(path).ok().as_deref() == Some(contents) {
        return;
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .unwrap_or_else(|e| panic!("failed to create {}: {e}", parent.display()));
    }

    let temp_path = path.with_file_name(format!(
        ".{}.{}.tmp",
        std::process::id(),
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("generated")
    ));

    fs::write(&temp_path, contents)
        .unwrap_or_else(|e| panic!("failed to write {}: {e}", temp_path.display()));
    fs::rename(&temp_path, path).unwrap_or_else(|e| {
        panic!(
            "failed to move {} to {}: {e}",
            temp_path.display(),
            path.display()
        )
    });
}

fn run_cue(workspace_dir: &Path, expr: &str, output_file: &str) {
    let output_path = workspace_dir.join(output_file);
    let temp_output_file = format!(
        ".{}.{}",
        env::var("CARGO_PKG_NAME").unwrap_or_else(|_| "schema".to_string()),
        output_file
    );

    let status = Command::new("mise")
        .current_dir(workspace_dir)
        .args([
            "x",
            "--",
            "cue",
            "def",
            "--force",
            "schema.cue",
            "-e",
            expr,
            "-o",
            &format!("jsonschema:{temp_output_file}"),
        ])
        .status()
        .unwrap_or_else(|e| panic!("failed to execute `cue`: {e}"));

    assert!(
        status.success(),
        "`cue def` failed while generating {output_file}. Is `cue` installed and on PATH?"
    );

    let temp_output_path = workspace_dir.join(&temp_output_file);
    let contents = fs::read_to_string(&temp_output_path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", temp_output_path.display()));
    write_if_changed(&output_path, &contents);
    let _ = fs::remove_file(temp_output_path);
}

fn cue_export_json(workspace_dir: &Path, expr: &str) -> Value {
    let output = Command::new("mise")
        .current_dir(workspace_dir)
        .args(["x", "--", "cue", "export", "--force", "schema.cue", "-e", expr])
        .output()
        .unwrap_or_else(|e| panic!("failed to execute `cue export`: {e}"));

    assert!(
        output.status.success(),
        "`cue export` failed for expression `{expr}` while reading schema metadata. Is `cue` installed and on PATH?"
    );

    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|e| panic!("failed to parse cue export output as JSON: {e}"))
}

fn normalize_schema_aliases(schema_path: &Path) {
    let mut schema: Value = serde_json::from_str(
        &fs::read_to_string(schema_path)
            .unwrap_or_else(|e| panic!("failed to read {}: {e}", schema_path.display())),
    )
    .unwrap_or_else(|e| panic!("failed to parse {}: {e}", schema_path.display()));

    normalize_trivial_aliases(&mut schema);

    let normalized = serde_json::to_string_pretty(&schema)
        .unwrap_or_else(|e| panic!("failed to serialize {}: {e}", schema_path.display()));
    write_if_changed(schema_path, &(normalized + "\n"));
}

fn tool_bindings_for_component(tools: &Value) -> Vec<ToolBinding> {
    let tool_map = tools.as_object().unwrap_or_else(|| {
        panic!(
            "expected `{}` to contain a JSON object",
            "cue export McpTools"
        )
    });

    let mut bindings = Vec::with_capacity(tool_map.len());
    for (tool_name, tool) in tool_map {
        let input_schema_name = tool
            .get("inputSchemaName")
            .and_then(Value::as_str)
            .unwrap_or_else(|| panic!("tool `{tool_name}` is missing `inputSchemaName`"));
        let output_schema_name = tool
            .get("outputSchemaName")
            .and_then(Value::as_str)
            .unwrap_or_else(|| panic!("tool `{tool_name}` is missing `outputSchemaName`"));

        bindings.push(ToolBinding {
            tool_name: tool_name.to_string(),
            input_schema_name: input_schema_name.to_string(),
            output_schema_name: output_schema_name.to_string(),
        });
    }

    assert!(
        !bindings.is_empty(),
        "expected at least one tool definition in cue export McpTools"
    );

    bindings
}

fn generate_component_wit(
    manifest_dir: &Path,
    all_schemas_path: &Path,
    tool_bindings: &[ToolBinding],
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
            package: "acme:app".to_string(),
            version,
            interface: "api".to_string(),
            world: "component".to_string(),
            function: "run".to_string(),
        },
    )
    .unwrap_or_else(|e| panic!("failed to generate {}: {e}", wit_path.display()));

    write_if_changed(&wit_path, &wit);
}

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("missing manifest dir"));
    let workspace_dir = manifest_dir.join("../..");
    let all_schemas_path = workspace_dir.join("_all_schemas.schema.json");

    println!(
        "cargo:rerun-if-changed={}",
        workspace_dir.join("schema.cue").display()
    );

    let tools = cue_export_json(&workspace_dir, "McpTools");
    let tool_bindings = tool_bindings_for_component(&tools);

    run_cue(&workspace_dir, "Schemas", "_all_schemas.schema.json");
    normalize_schema_aliases(&all_schemas_path);
    generate_component_wit(&manifest_dir, &all_schemas_path, &tool_bindings);
}
