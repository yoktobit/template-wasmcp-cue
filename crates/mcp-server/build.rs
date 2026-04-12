use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;

use serde_json::Value;

#[derive(Debug, Clone)]
struct ToolBinding {
    tool_name: String,
    input_schema_name: String,
    output_schema_name: String,
}

fn write_if_changed(destination: &Path, contents: &str) {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)
            .unwrap_or_else(|e| panic!("failed to create {}: {e}", parent.display()));
    }

    if fs::read_to_string(destination).ok().as_deref() == Some(contents) {
        return;
    }

    let temp_path = destination.with_file_name(format!(
        ".{}.{}.tmp",
        std::process::id(),
        destination
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("generated")
    ));

    fs::write(&temp_path, contents)
        .unwrap_or_else(|e| panic!("failed to write {}: {e}", temp_path.display()));
    fs::rename(&temp_path, destination).unwrap_or_else(|e| {
        panic!(
            "failed to move {} to {}: {e}",
            temp_path.display(),
            destination.display()
        )
    });
}

fn copy_if_changed(source: &str, destination: &str) {
    let source_path = Path::new(source);
    let destination_path = Path::new(destination);
    let contents = fs::read_to_string(source_path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", source_path.display()));

    write_if_changed(destination_path, &contents);
}

fn run_cue(workspace_dir: &Path, command: &str, expr: &str, format: &str, output_file: &str) {
    let temp_output_file = format!(
        ".{}.{}",
        env::var("CARGO_PKG_NAME").unwrap_or_else(|_| "schema".to_string()),
        output_file
    );
    let output_spec = format!("{format}:{temp_output_file}");

    let status = Command::new("mise")
        .current_dir(workspace_dir)
        .args([
            "x",
            "--",
            "cue",
            command,
            "--force",
            "schema.cue",
            "-e",
            expr,
            "-o",
            &output_spec,
        ])
        .status()
        .unwrap_or_else(|e| panic!("failed to execute `cue {command}`: {e}"));

    assert!(
        status.success(),
        "`cue {command}` failed while generating {output_file}. Is `cue` installed and on PATH?"
    );

    let temp_output_path = workspace_dir.join(&temp_output_file);
    let contents = fs::read_to_string(&temp_output_path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", temp_output_path.display()));
    write_if_changed(&workspace_dir.join(output_file), &contents);
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

fn export_tool_constants(workspace_dir: &Path, destination: &Path) {
    let output_spec = format!("text:{}", destination.display());

    let status = Command::new("mise")
        .current_dir(workspace_dir)
        .args([
            "x",
            "--",
            "cue",
            "export",
            "--force",
            "schema.cue",
            "-e",
            "RustToolConstants",
            "-o",
            &output_spec,
        ])
        .status()
        .unwrap_or_else(|e| panic!("failed to execute `cue export`: {e}"));

    assert!(
        status.success(),
        "`cue export` failed while generating {}. Is `cue` installed and on PATH?",
        destination.display()
    );
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

fn ensure_root_schemas(workspace_dir: &Path) {
    run_cue(
        workspace_dir,
        "def",
        "Schemas",
        "jsonschema",
        "_all_schemas.schema.json",
    );
}

fn to_snake_case(name: &str) -> String {
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

fn to_pascal_case(name: &str) -> String {
    let mut out = String::new();
    let mut new_word = true;

    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            if new_word {
                out.push(ch.to_ascii_uppercase());
                new_word = false;
            } else {
                out.push(ch);
            }
        } else {
            new_word = true;
        }
    }

    out
}

fn generate_tool_dispatch(destination: &Path, tools: &[ToolBinding]) {
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

    write_if_changed(destination, &source);
}

fn main() {
    println!("cargo:rerun-if-changed=../../schema.cue");
    println!("cargo:rerun-if-changed=../component/wit/world.wit");

    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("missing CARGO_MANIFEST_DIR");
    let workspace_dir = Path::new(&manifest_dir).join("../..");
    let out_dir = env::var("OUT_DIR").expect("missing OUT_DIR");
    let tools = cue_export_json(&workspace_dir, "McpTools");
    let bindings = tool_bindings_for_component(&tools);

    ensure_root_schemas(&workspace_dir);
    copy_if_changed("../component/wit/world.wit", "wit/deps/acme-app.wit");
    copy_if_changed(
        "../component/wit/world.wit",
        "wit/component-client/deps/acme-app.wit",
    );
    export_tool_constants(
        &workspace_dir,
        &Path::new(&out_dir).join("tool_constants.rs"),
    );
    generate_tool_dispatch(&Path::new(&out_dir).join("tool_dispatch.rs"), &bindings);
}
