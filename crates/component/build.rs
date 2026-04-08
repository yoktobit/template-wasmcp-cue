use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{Map, Value};

fn write_if_changed(path: &Path, contents: &str) {
    if fs::read_to_string(path).ok().as_deref() == Some(contents) {
        return;
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .unwrap_or_else(|e| panic!("failed to create {}: {e}", parent.display()));
    }

    fs::write(path, contents).unwrap_or_else(|e| panic!("failed to write {}: {e}", path.display()));
}

fn run_cue(manifest_dir: &Path, expr: &str, output_file: &str) {
    let status = Command::new("mise")
        .current_dir(manifest_dir)
        .args([
            "x",
            "--",
            "cue",
            "def",
            "-f",
            "schema.cue",
            "-e",
            expr,
            "-o",
            &format!("jsonschema:{output_file}"),
        ])
        .status()
        .unwrap_or_else(|e| panic!("failed to execute `cue`: {e}"));

    assert!(
        status.success(),
        "`cue def` failed while generating {output_file}. Is `cue` installed and on PATH?"
    );
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

fn to_kebab_case(name: &str) -> String {
    let mut out = String::new();
    let mut last_was_separator = false;

    for (index, ch) in name.chars().enumerate() {
        if ch.is_ascii_alphanumeric() {
            if ch.is_ascii_uppercase() {
                if index > 0 && !last_was_separator && !out.ends_with('-') {
                    out.push('-');
                }
                out.push(ch.to_ascii_lowercase());
            } else {
                out.push(ch.to_ascii_lowercase());
            }
            last_was_separator = false;
        } else if !out.is_empty() && !last_was_separator {
            out.push('-');
            last_was_separator = true;
        }
    }

    out.trim_matches('-').to_string()
}

fn escape_wit_ident(name: &str) -> String {
    match name {
        "type" | "record" | "enum" | "variant" | "flags" | "world" | "interface"
        | "use" | "func" | "export" | "import" | "package" | "include" | "with"
        | "from" | "static" | "resource" | "string" | "option" | "result" | "list" => {
            format!("%{name}")
        }
        _ => name.to_string(),
    }
}

fn required_properties(schema: &Value) -> BTreeSet<String> {
    schema
        .get("required")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(ToOwned::to_owned)
        .collect()
}

fn find_def<'a>(defs: &'a Map<String, Value>, def_name: &str) -> &'a Value {
    defs.iter()
        .find_map(|(key, value)| {
            let cleaned = key.trim_start_matches('#');
            if cleaned == def_name || key == def_name {
                Some(value)
            } else {
                None
            }
        })
        .unwrap_or_else(|| panic!("missing schema definition for {def_name}"))
}

fn schema_to_wit_type(
    logical_name: &str,
    schema: &Value,
    defs: &Map<String, Value>,
    emitted: &mut BTreeMap<String, String>,
    order: &mut Vec<String>,
) -> String {
    if let Some(reference) = schema.get("$ref").and_then(Value::as_str) {
        let def_name = decode_ref_name(reference);
        let def_schema = find_def(defs, &def_name);
        return schema_to_wit_type(&def_name, def_schema, defs, emitted, order);
    }

    if schema.get("properties").is_some() || schema.get("type").and_then(Value::as_str) == Some("object") {
        return generate_named_record(logical_name, schema, defs, emitted, order);
    }

    if schema.get("enum").is_some() {
        return "string".to_string();
    }

    if let Some(one_of) = schema.get("oneOf").and_then(Value::as_array) {
        if let Some(first) = one_of.first() {
            return schema_to_wit_type(logical_name, first, defs, emitted, order);
        }
    }

    match schema.get("type").and_then(Value::as_str) {
        Some("string") => "string".to_string(),
        Some("integer") => "s64".to_string(),
        Some("number") => "f64".to_string(),
        Some("boolean") => "bool".to_string(),
        Some("array") => {
            let item_type = schema
                .get("items")
                .map(|items| schema_to_wit_type(&format!("{logical_name}-item"), items, defs, emitted, order))
                .unwrap_or_else(|| "string".to_string());
            format!("list<{item_type}>")
        }
        Some(other) => panic!("unsupported schema type `{other}` in {logical_name}"),
        None => "string".to_string(),
    }
}

fn generate_named_record(
    logical_name: &str,
    schema: &Value,
    defs: &Map<String, Value>,
    emitted: &mut BTreeMap<String, String>,
    order: &mut Vec<String>,
) -> String {
    let record_name = escape_wit_ident(&to_kebab_case(logical_name));

    if emitted.contains_key(&record_name) {
        return record_name;
    }

    let properties = schema
        .get("properties")
        .and_then(Value::as_object)
        .unwrap_or_else(|| panic!("expected object schema for {logical_name}"));

    let required = required_properties(schema);
    let mut fields = Vec::new();

    for (property_name, property_schema) in properties {
        let field_name = escape_wit_ident(&to_kebab_case(property_name));
        let mut field_type = schema_to_wit_type(
            &format!("{logical_name}-{property_name}"),
            property_schema,
            defs,
            emitted,
            order,
        );

        if !required.contains(property_name) {
            field_type = format!("option<{field_type}>");
        }

        fields.push(format!("    {field_name}: {field_type},"));
    }

    let mut record = format!("record {record_name} {{\n");
    if !fields.is_empty() {
        record.push_str(&fields.join("\n"));
        record.push('\n');
    }
    record.push('}');

    emitted.insert(record_name.clone(), record);
    order.push(record_name.clone());
    record_name
}

fn resolve_root_type(
    schema: &Value,
    fallback_name: &str,
    emitted: &mut BTreeMap<String, String>,
    order: &mut Vec<String>,
) -> String {
    let defs = schema
        .get("$defs")
        .and_then(Value::as_object)
        .unwrap_or_else(|| panic!("schema is missing `$defs`"));

    let logical_name = schema
        .get("$ref")
        .and_then(Value::as_str)
        .map(decode_ref_name)
        .unwrap_or_else(|| fallback_name.to_string());

    if let Some(reference) = schema.get("$ref").and_then(Value::as_str) {
        let def_name = decode_ref_name(reference);
        let root_schema = find_def(defs, &def_name);
        schema_to_wit_type(&logical_name, root_schema, defs, emitted, order)
    } else {
        schema_to_wit_type(&logical_name, schema, defs, emitted, order)
    }
}

fn generate_component_wit(manifest_dir: &Path) {
    let input_schema_path = manifest_dir.join("_input.schema.json");
    let output_schema_path = manifest_dir.join("_output.schema.json");
    let wit_path = manifest_dir.join("wit/world.wit");

    let input_schema: Value = serde_json::from_str(
        &fs::read_to_string(&input_schema_path)
            .unwrap_or_else(|e| panic!("failed to read {}: {e}", input_schema_path.display())),
    )
    .unwrap_or_else(|e| panic!("failed to parse {}: {e}", input_schema_path.display()));

    let output_schema: Value = serde_json::from_str(
        &fs::read_to_string(&output_schema_path)
            .unwrap_or_else(|e| panic!("failed to read {}: {e}", output_schema_path.display())),
    )
    .unwrap_or_else(|e| panic!("failed to parse {}: {e}", output_schema_path.display()));

    let mut emitted = BTreeMap::new();
    let mut order = Vec::new();

    let input_type = resolve_root_type(&input_schema, "input", &mut emitted, &mut order);
    let output_type = resolve_root_type(&output_schema, "output", &mut emitted, &mut order);
    let version = env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "0.1.0".to_string());

    let mut wit = format!(
        "package acme:app@{version};\n\n/// Generated from `schema.cue` by `build.rs`. Do not edit manually.\ninterface api {{\n"
    );

    for name in order {
        if let Some(definition) = emitted.get(&name) {
            for line in definition.lines() {
                wit.push_str("    ");
                wit.push_str(line);
                wit.push('\n');
            }
            wit.push('\n');
        }
    }

    wit.push_str(&format!("    run: func(input: {input_type}) -> {output_type};\n"));
    wit.push_str("}\n\nworld component {\n    export api;\n}\n");

    write_if_changed(&wit_path, &wit);
}

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("missing manifest dir"));

    println!("cargo:rerun-if-changed=schema.cue");

    run_cue(&manifest_dir, "Input", "_input.schema.json");
    run_cue(&manifest_dir, "Output", "_output.schema.json");
    normalize_schema_aliases(&manifest_dir.join("_input.schema.json"));
    normalize_schema_aliases(&manifest_dir.join("_output.schema.json"));
    generate_component_wit(&manifest_dir);
}
