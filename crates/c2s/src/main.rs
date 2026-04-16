use std::{env, path::Path};

use serde_json::{json, Value};

fn extract_tools_from_wasm(wasm_path: &str) -> Vec<Value> {
    let config = wasmtime::Config::new();
    let engine = wasmtime::Engine::new(&config).unwrap();
    let component = wasmtime::component::Component::from_file(&engine, Path::new(wasm_path)).unwrap();
    let wasm_bytes = std::fs::read(wasm_path).unwrap();
    let docs = component2json::extract_package_docs(&wasm_bytes)
        .or_else(|| Some(json!({})))
        .unwrap();
    let tools_json = component2json::component_exports_to_json_schema_with_docs(
        &component,
        &engine,
        true,
        &docs,
    );

    tools_json
        .get("tools")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let (wasm_paths, output_path): (Vec<&str>, Option<&str>) = match args.len() {
        1 => (
            vec!["../../target/wasm32-wasip2/release/acme_component_pet.wasm"],
            None,
        ),
        2 => (vec![args[1].as_str()], None),
        _ => {
            let out = args.last().map(String::as_str);
            let paths = args[1..args.len() - 1]
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>();
            (paths, out)
        }
    };

    let mut merged_tools: Vec<Value> = Vec::new();
    for wasm_path in &wasm_paths {
        merged_tools.extend(extract_tools_from_wasm(wasm_path));
    }

    let json = serde_json::json!({ "tools": merged_tools }).to_string();

    if let Some(path) = output_path {
        std::fs::write(path, json).unwrap();
    } else {
        println!("{}", json);
    }
}
