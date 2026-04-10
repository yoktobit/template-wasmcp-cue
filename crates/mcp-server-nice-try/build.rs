use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;

use serde_json::Value;

fn decode_ref_name(reference: &str) -> String {
    reference
        .rsplit('/')
        .next()
        .unwrap_or(reference)
        .replace("%23", "")
        .trim_start_matches('#')
        .to_string()
}

fn write_if_changed(destination: &Path, contents: &str) {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)
            .unwrap_or_else(|e| panic!("failed to create {}: {e}", parent.display()));
    }

    if fs::read_to_string(destination).ok().as_deref() != Some(contents) {
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
}

fn copy_if_changed(source: &str, destination: &str) {
    let source_path = Path::new(source);
    let destination_path = Path::new(destination);
    let contents = fs::read_to_string(source_path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", source_path.display()));

    write_if_changed(destination_path, &contents);
}

fn prepare_schema_for_typify(source: &Path, destination: &Path, title: &str) {
    let mut schema: Value = serde_json::from_str(
        &fs::read_to_string(source)
            .unwrap_or_else(|e| panic!("failed to read {}: {e}", source.display())),
    )
    .unwrap_or_else(|e| panic!("failed to parse {}: {e}", source.display()));

    if let Some(map) = schema.as_object_mut() {
        map.insert("title".to_string(), Value::String(title.to_string()));

        if map.get("properties").is_some()
            || map.get("type").and_then(Value::as_str) == Some("object")
        {
            map.remove("$ref");

            let remove_defs =
                if let Some(defs) = map.get_mut("$defs").and_then(Value::as_object_mut) {
                    defs.retain(|_, definition| {
                        !definition
                            .as_object()
                            .map(|object| object.len() == 1 && object.contains_key("const"))
                            .unwrap_or(false)
                    });
                    defs.is_empty()
                } else {
                    false
                };

            if remove_defs {
                map.remove("$defs");
            }
        }
    }

    let normalized = serde_json::to_string_pretty(&schema)
        .unwrap_or_else(|e| panic!("failed to serialize {}: {e}", source.display()));
    write_if_changed(destination, &(normalized + "\n"));
}

fn root_type_name(schema_path: &str) -> String {
    let contents = fs::read_to_string(schema_path)
        .unwrap_or_else(|e| panic!("failed to read {schema_path}: {e}"));
    let schema: Value = serde_json::from_str(&contents)
        .unwrap_or_else(|e| panic!("failed to parse {schema_path}: {e}"));

    schema
        .get("title")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .or_else(|| {
            schema
                .get("$ref")
                .and_then(Value::as_str)
                .map(decode_ref_name)
        })
        .unwrap_or_else(|| "Schema".to_string())
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

fn export_cue_json(workspace_dir: &Path, expr: &str, output_file: &str) {
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
            "export",
            "--force",
            "schema.cue",
            "-e",
            expr,
            "-o",
            &format!("json:{temp_output_file}"),
        ])
        .status()
        .unwrap_or_else(|e| panic!("failed to execute `cue export`: {e}"));

    assert!(
        status.success(),
        "`cue export` failed while generating {output_file}. Is `cue` installed and on PATH?"
    );

    let temp_output_path = workspace_dir.join(&temp_output_file);
    let contents = fs::read_to_string(&temp_output_path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", temp_output_path.display()));
    write_if_changed(&output_path, &contents);
    let _ = fs::remove_file(temp_output_path);
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

    let mut entries = tool_map.iter();
    let (tool_name, tool) = entries.next().unwrap_or_else(|| {
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
        .unwrap_or_else(|| panic!("tool `{tool_name}` is missing `inputSchemaName`"));
    let output_schema_name = tool
        .get("outputSchemaName")
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("tool `{tool_name}` is missing `outputSchemaName`"));

    (
        input_schema_name.to_string(),
        output_schema_name.to_string(),
    )
}

fn ensure_root_schemas(workspace_dir: &Path) {
    let mcp_tools_path = workspace_dir.join("_mcpTools.json");

    export_cue_json(workspace_dir, "McpTools", "_mcpTools.json");
    let (input_schema_name, output_schema_name) = schema_names_for_component(&mcp_tools_path);

    run_cue(
        workspace_dir,
        &format!("Schemas[\"{input_schema_name}\"]"),
        "_input.schema.json",
    );
    run_cue(
        workspace_dir,
        &format!("Schemas[\"{output_schema_name}\"]"),
        "_output.schema.json",
    );
}

fn generate_component_api(destination: &Path) {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("missing CARGO_MANIFEST_DIR");
    let generated_dir = destination.parent().unwrap_or_else(|| Path::new("."));
    let input_schema_path = Path::new(&manifest_dir).join("../../_input.schema.json");
    let output_schema_path = Path::new(&manifest_dir).join("../../_output.schema.json");
    let input_schema = generated_dir.join("input.typify.schema.json");
    let output_schema = generated_dir.join("output.typify.schema.json");

    prepare_schema_for_typify(&input_schema_path, &input_schema, "Input");
    prepare_schema_for_typify(&output_schema_path, &output_schema, "Output");

    let input_schema = input_schema.display().to_string();
    let output_schema = output_schema.display().to_string();
    let input_type = root_type_name(&input_schema);
    let output_type = root_type_name(&output_schema);

    let source = format!(
        "pub mod input {{\n    typify::import_types!(schema = \"{input_schema}\");\n}}\n\npub mod output {{\n    typify::import_types!(schema = \"{output_schema}\");\n}}\n\npub use input::{input_type} as Input;\npub use output::{output_type} as Output;\n\nuse crate::bindings::acme::app::api;\n\npub fn call(input: Input) -> Output {{\n    api::run(&input)\n}}\n"
    );

    write_if_changed(destination, &source);
}

fn main() {
    println!("cargo:rerun-if-changed=../../schema.cue");
    println!("cargo:rerun-if-changed=../component/wit/world.wit");
    println!("cargo:rerun-if-changed=../../_input.schema.json");
    println!("cargo:rerun-if-changed=../../_output.schema.json");
    println!("cargo:rerun-if-changed=../../_mcpTools.json");

    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("missing CARGO_MANIFEST_DIR");
    let workspace_dir = Path::new(&manifest_dir).join("../..");

    ensure_root_schemas(&workspace_dir);
    copy_if_changed("../component/wit/world.wit", "wit/deps/acme-app.wit");

    let out_dir = env::var("OUT_DIR").expect("missing OUT_DIR");
    generate_component_api(&Path::new(&out_dir).join("component_api.rs"));
}
