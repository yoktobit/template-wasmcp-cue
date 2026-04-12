use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use jsonschema_to_wit::{generate_wit_from_files, WitConfig};
use serde_json::Value;

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

fn is_trivial_alias(schema: &Value) -> bool {
    let Some(object) = schema.as_object() else {
        return false;
    };

    if object.len() != 1 {
        return false;
    }

    matches!(
        object.get("type").and_then(Value::as_str),
        Some("string" | "integer" | "number" | "boolean")
    )
}

fn inline_trivial_refs(value: &mut Value, trivial_defs: &BTreeMap<String, Value>) {
    match value {
        Value::Object(map) => {
            if let Some(reference) = map.get("$ref").and_then(Value::as_str) {
                let def_name = decode_ref_name(reference);
                if let Some(replacement) = trivial_defs.get(&def_name) {
                    *value = replacement.clone();
                    return;
                }
            }

            for nested in map.values_mut() {
                inline_trivial_refs(nested, trivial_defs);
            }
        }
        Value::Array(items) => {
            for item in items {
                inline_trivial_refs(item, trivial_defs);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

fn normalize_schema_aliases(schema_path: &Path) {
    let mut schema: Value = serde_json::from_str(
        &fs::read_to_string(schema_path)
            .unwrap_or_else(|e| panic!("failed to read {}: {e}", schema_path.display())),
    )
    .unwrap_or_else(|e| panic!("failed to parse {}: {e}", schema_path.display()));

    let trivial_defs = schema
        .get("$defs")
        .and_then(Value::as_object)
        .map(|defs| {
            defs.iter()
                .filter_map(|(name, definition)| {
                    if is_trivial_alias(definition) {
                        Some((decode_ref_name(name), definition.clone()))
                    } else {
                        None
                    }
                })
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default();

    if trivial_defs.is_empty() {
        return;
    }

    inline_trivial_refs(&mut schema, &trivial_defs);

    if let Some(defs) = schema.get_mut("$defs").and_then(Value::as_object_mut) {
        defs.retain(|name, _| !trivial_defs.contains_key(&decode_ref_name(name)));
    }

    let normalized = serde_json::to_string_pretty(&schema)
        .unwrap_or_else(|e| panic!("failed to serialize {}: {e}", schema_path.display()));
    write_if_changed(schema_path, &(normalized + "\n"));
}

fn decode_ref_name(reference: &str) -> String {
    reference
        .rsplit('/')
        .next()
        .unwrap_or(reference)
        .replace("%23", "")
        .trim_start_matches('#')
        .to_string()
}

fn schema_names_for_component(tools: &Value) -> (String, String) {
    let tool_map = tools.as_object().unwrap_or_else(|| {
        panic!(
            "expected `{}` to contain a JSON object",
            "cue export McpTools"
        )
    });

    let mut entries = tool_map.iter();
    let (tool_name, tool) = entries.next().unwrap_or_else(|| {
        panic!(
            "expected at least one tool definition in {}",
            "cue export McpTools"
        )
    });

    assert!(
        entries.next().is_none(),
        "expected exactly one tool in cue export McpTools for the single-component template"
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

fn generate_component_wit(
    manifest_dir: &Path,
    input_schema_path: &Path,
    output_schema_path: &Path,
) {
    let wit_path = manifest_dir.join("wit/world.wit");
    let version = env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "0.1.0".to_string());

    let wit = generate_wit_from_files(
        input_schema_path,
        output_schema_path,
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
    let input_schema_path = workspace_dir.join("_input.schema.json");
    let output_schema_path = workspace_dir.join("_output.schema.json");

    println!(
        "cargo:rerun-if-changed={}",
        workspace_dir.join("schema.cue").display()
    );

    let tools = cue_export_json(&workspace_dir, "McpTools");
    let (input_schema_name, output_schema_name) = schema_names_for_component(&tools);

    run_cue(
        &workspace_dir,
        &format!("Schemas[\"{input_schema_name}\"]"),
        "_input.schema.json",
    );
    run_cue(
        &workspace_dir,
        &format!("Schemas[\"{output_schema_name}\"]"),
        "_output.schema.json",
    );
    normalize_schema_aliases(&input_schema_path);
    normalize_schema_aliases(&output_schema_path);
    generate_component_wit(&manifest_dir, &input_schema_path, &output_schema_path);
}
