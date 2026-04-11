use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;

use serde_json::Value;

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

fn schema_names_for_component(mcp_tools_path: &Path) -> (String, String) {
    let tools: Value = serde_json::from_str(
        &fs::read_to_string(mcp_tools_path)
            .unwrap_or_else(|e| panic!("failed to read {}: {e}", mcp_tools_path.display())),
    )
    .unwrap_or_else(|e| panic!("failed to parse {}: {e}", mcp_tools_path.display()));

    let tool_map = tools.as_object().unwrap_or_else(|| {
        panic!(
            "expected `{}` to contain a JSON object",
            mcp_tools_path.display()
        )
    });

    let mut entries = tool_map.values();
    let tool = entries.next().unwrap_or_else(|| {
        panic!(
            "expected at least one tool definition in {}",
            mcp_tools_path.display()
        )
    });

    assert!(
        entries.next().is_none(),
        "expected exactly one tool in {} for the single-component template",
        mcp_tools_path.display()
    );

    let input_schema_name = tool
        .get("inputSchemaName")
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("tool definition is missing `inputSchemaName`"));
    let output_schema_name = tool
        .get("outputSchemaName")
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("tool definition is missing `outputSchemaName`"));

    (
        input_schema_name.to_string(),
        output_schema_name.to_string(),
    )
}

fn ensure_root_schemas(workspace_dir: &Path) {
    let mcp_tools_path = workspace_dir.join("_mcpTools.json");

    run_cue(
        workspace_dir,
        "export",
        "McpTools",
        "json",
        "_mcpTools.json",
    );
    let (input_schema_name, output_schema_name) = schema_names_for_component(&mcp_tools_path);

    run_cue(
        workspace_dir,
        "def",
        &format!("Schemas[\"{input_schema_name}\"]"),
        "jsonschema",
        "_input.schema.json",
    );
    run_cue(
        workspace_dir,
        "def",
        &format!("Schemas[\"{output_schema_name}\"]"),
        "jsonschema",
        "_output.schema.json",
    );
}

fn main() {
    println!("cargo:rerun-if-changed=../../schema.cue");
    println!("cargo:rerun-if-changed=../component/wit/world.wit");

    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("missing CARGO_MANIFEST_DIR");
    let workspace_dir = Path::new(&manifest_dir).join("../..");
    let out_dir = env::var("OUT_DIR").expect("missing OUT_DIR");

    ensure_root_schemas(&workspace_dir);
    copy_if_changed("../component/wit/world.wit", "wit/deps/acme-app.wit");
    export_tool_constants(
        &workspace_dir,
        &Path::new(&out_dir).join("tool_constants.rs"),
    );
}
