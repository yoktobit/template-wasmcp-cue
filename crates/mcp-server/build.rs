use std::env;
use std::fs;
use std::path::Path;

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
        fs::write(destination, contents)
            .unwrap_or_else(|e| panic!("failed to write {}: {e}", destination.display()));
    }
}

fn copy_if_changed(source: &str, destination: &str) {
    let source_path = Path::new(source);
    let destination_path = Path::new(destination);
    let contents = fs::read_to_string(source_path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", source_path.display()));

    write_if_changed(destination_path, &contents);
}

fn root_type_name(schema_path: &str) -> String {
    let contents = fs::read_to_string(schema_path)
        .unwrap_or_else(|e| panic!("failed to read {schema_path}: {e}"));
    let schema: Value = serde_json::from_str(&contents)
        .unwrap_or_else(|e| panic!("failed to parse {schema_path}: {e}"));

    schema
        .get("$ref")
        .and_then(Value::as_str)
        .map(decode_ref_name)
        .unwrap_or_else(|| panic!("schema {schema_path} is missing a root $ref"))
}

fn generate_component_api(destination: &Path) {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("missing CARGO_MANIFEST_DIR");
    let input_schema = format!("{manifest_dir}/../component/_input.schema.json");
    let output_schema = format!("{manifest_dir}/../component/_output.schema.json");
    let input_type = root_type_name(&input_schema);
    let output_type = root_type_name(&output_schema);

    let source = format!(
        "pub mod input {{\n    typify::import_types!(schema = \"{input_schema}\");\n}}\n\npub mod output {{\n    typify::import_types!(schema = \"{output_schema}\");\n}}\n\npub use input::{input_type} as Input;\npub use output::{output_type} as Output;\n\nuse crate::bindings::acme::app::api;\n\npub fn call(input: Input) -> Output {{\n    api::run(&input)\n}}\n"
    );

    write_if_changed(destination, &source);
}

fn main() {
    println!("cargo:rerun-if-changed=../component/schema.cue");
    println!("cargo:rerun-if-changed=../component/wit/world.wit");
    println!("cargo:rerun-if-changed=../component/_input.schema.json");
    println!("cargo:rerun-if-changed=../component/_output.schema.json");

    copy_if_changed("../component/wit/world.wit", "wit/deps/acme-app.wit");

    let out_dir = env::var("OUT_DIR").expect("missing OUT_DIR");
    generate_component_api(&Path::new(&out_dir).join("component_api.rs"));
}
