use std::{io, path::Path};

use serde_json::json;



fn main() {
    let config = wasmtime::Config::new();
    let engine = wasmtime::Engine::new(&config).unwrap();
    let component = wasmtime::component::Component::from_file(&engine, Path::new("../../target/wasm32-wasip2/release/acme_component.wasm")).unwrap();
    let wasm_bytes = std::fs::read("../../target/wasm32-wasip2/release/acme_component.wasm").unwrap();
    let docs = component2json::extract_package_docs(&wasm_bytes).or_else(|| Some(json!({}))).unwrap();
    let json = component2json::component_exports_to_json_schema_with_docs(&component, &engine, true, &docs).to_string();
    println!("{}", json);
}
