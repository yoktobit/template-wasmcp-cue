use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::path::Path;

use serde_json::{Map, Value};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WitConfig {
    pub package: String,
    pub version: String,
    pub interface: String,
    pub world: String,
    pub function: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolFunction {
    pub name: String,
    pub input_schema_name: String,
    pub output_schema_name: String,
}

impl Default for WitConfig {
    fn default() -> Self {
        Self {
            package: "acme:app".to_string(),
            version: "0.1.0".to_string(),
            interface: "api".to_string(),
            world: "component".to_string(),
            function: "run".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error(String);

impl Error {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

pub fn generate_wit_from_files(
    input_schema_path: impl AsRef<Path>,
    output_schema_path: impl AsRef<Path>,
    config: &WitConfig,
) -> Result<String> {
    let input_schema = read_schema(input_schema_path.as_ref())?;
    let output_schema = read_schema(output_schema_path.as_ref())?;
    generate_wit(&input_schema, &output_schema, config)
}

pub fn generate_wit_for_tools_from_file(
    all_schemas_path: impl AsRef<Path>,
    tool_functions: &[ToolFunction],
    config: &WitConfig,
) -> Result<String> {
    let all_schemas = read_schema(all_schemas_path.as_ref())?;
    generate_wit_for_tools(&all_schemas, tool_functions, config)
}

pub fn generate_wit(
    input_schema: &Value,
    output_schema: &Value,
    config: &WitConfig,
) -> Result<String> {
    let mut emitted = BTreeMap::new();
    let mut order = Vec::new();

    let input_type = resolve_root_type(input_schema, "input", &mut emitted, &mut order)?;
    let output_type = resolve_root_type(output_schema, "output", &mut emitted, &mut order)?;
    let interface_name = escape_wit_ident(&to_kebab_case(&config.interface));
    let world_name = escape_wit_ident(&to_kebab_case(&config.world));
    let function_name = escape_wit_ident(&to_kebab_case(&config.function));

    let mut wit = format!(
        "package {}@{};\n\n/// Generated from JSON Schema. Do not edit manually.\ninterface {} {{\n",
        config.package, config.version, interface_name
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

    wit.push_str(&format!(
        "    {function_name}: func(input: {input_type}) -> {output_type};\n"
    ));
    wit.push_str(&format!(
        "}}\n\nworld {world_name} {{\n    export {interface_name};\n}}\n"
    ));

    Ok(wit)
}

pub fn generate_wit_for_tools(
    all_schemas: &Value,
    tool_functions: &[ToolFunction],
    config: &WitConfig,
) -> Result<String> {
    if tool_functions.is_empty() {
        return Err(Error::new("expected at least one tool function"));
    }

    let mut emitted = BTreeMap::new();
    let mut order = Vec::new();
    let interface_name = escape_wit_ident(&to_kebab_case(&config.interface));
    let world_name = escape_wit_ident(&to_kebab_case(&config.world));
    let schemas = all_schema_properties(all_schemas)?;
    let empty_defs = Map::new();
    let defs = all_schemas
        .get("$defs")
        .and_then(Value::as_object)
        .unwrap_or(&empty_defs);

    let mut function_lines = Vec::with_capacity(tool_functions.len());
    for tool in tool_functions {
        let function_name = escape_wit_ident(&to_kebab_case(&tool.name));
        let input_schema = find_def(schemas, &tool.input_schema_name)?;
        let output_schema = find_def(schemas, &tool.output_schema_name)?;
        let input_type = schema_to_wit_type(
            &tool.input_schema_name,
            input_schema,
            defs,
            &mut emitted,
            &mut order,
        )?;
        let output_type = schema_to_wit_type(
            &tool.output_schema_name,
            output_schema,
            defs,
            &mut emitted,
            &mut order,
        )?;

        function_lines.push(format!(
            "    {function_name}: func(input: {input_type}) -> {output_type};"
        ));
    }

    let mut wit = format!(
        "package {}@{};\n\n/// Generated from JSON Schema. Do not edit manually.\ninterface {} {{\n",
        config.package, config.version, interface_name
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

    wit.push_str(&function_lines.join("\n"));
    wit.push('\n');
    wit.push_str(&format!(
        "}}\n\nworld {world_name} {{\n    export {interface_name};\n}}\n"
    ));

    Ok(wit)
}

pub fn normalize_trivial_aliases(schema: &mut Value) {
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

    inline_trivial_refs(schema, &trivial_defs);

    if let Some(defs) = schema.get_mut("$defs").and_then(Value::as_object_mut) {
        defs.retain(|name, _| !trivial_defs.contains_key(&decode_ref_name(name)));
    }
}

pub fn normalize_json_keys_to_snake(value: &mut Value) {
    match value {
        Value::Object(map) => {
            let old = std::mem::take(map);
            for (key, mut nested) in old {
                normalize_json_keys_to_snake(&mut nested);
                map.insert(to_snake_case(&key), nested);
            }
        }
        Value::Array(items) => {
            for item in items {
                normalize_json_keys_to_snake(item);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

pub fn remap_json_keys_to_schema(value: &mut Value, schema: &Value) {
    match value {
        Value::Object(map) => {
            let properties = schema.get("properties").and_then(Value::as_object);

            if let Some(properties) = properties {
                let mut canonical_to_schema = BTreeMap::new();
                for schema_key in properties.keys() {
                    canonical_to_schema.insert(to_snake_case(schema_key), schema_key.clone());
                }

                let old = std::mem::take(map);
                for (key, mut nested) in old {
                    let canonical = to_snake_case(&key);
                    let remapped_key = canonical_to_schema
                        .get(&canonical)
                        .cloned()
                        .unwrap_or(key.clone());

                    if let Some(nested_schema) = properties.get(&remapped_key) {
                        remap_json_keys_to_schema(&mut nested, nested_schema);
                    } else {
                        remap_json_keys_to_schema(&mut nested, &Value::Null);
                    }

                    map.insert(remapped_key, nested);
                }
            } else {
                for nested in map.values_mut() {
                    remap_json_keys_to_schema(nested, &Value::Null);
                }
            }
        }
        Value::Array(items) => {
            let item_schema = schema.get("items").unwrap_or(&Value::Null);
            for item in items {
                remap_json_keys_to_schema(item, item_schema);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

pub fn to_snake_case(name: &str) -> String {
    to_separated_case(name, '_')
}

pub fn to_kebab_case(name: &str) -> String {
    to_separated_case(name, '-')
}

pub fn to_pascal_case(name: &str) -> String {
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

fn read_schema(path: &Path) -> Result<Value> {
    let contents = fs::read_to_string(path)
        .map_err(|error| Error::new(format!("failed to read {}: {error}", path.display())))?;

    serde_json::from_str(&contents)
        .map_err(|error| Error::new(format!("failed to parse {}: {error}", path.display())))
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

fn to_separated_case(name: &str, separator: char) -> String {
    let mut out = String::new();
    let mut last_was_separator = false;

    for (index, ch) in name.chars().enumerate() {
        if ch.is_ascii_alphanumeric() {
            if ch.is_ascii_uppercase() {
                if index > 0 && !last_was_separator && !out.ends_with(separator) {
                    out.push(separator);
                }
                out.push(ch.to_ascii_lowercase());
            } else {
                out.push(ch.to_ascii_lowercase());
            }
            last_was_separator = false;
        } else if !out.is_empty() && !last_was_separator {
            out.push(separator);
            last_was_separator = true;
        }
    }

    out.trim_matches(separator).to_string()
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

fn escape_wit_ident(name: &str) -> String {
    match name {
        "type" | "record" | "enum" | "variant" | "flags" | "world" | "interface" | "use"
        | "func" | "export" | "import" | "package" | "include" | "with" | "from" | "static"
        | "resource" | "string" | "option" | "result" | "list" => {
            format!("%{name}")
        }
        _ => name.to_string(),
    }
}

fn all_schema_properties(all_schemas: &Value) -> Result<&Map<String, Value>> {
    all_schemas
        .get("properties")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            Error::new(
                "expected all-schemas JSON Schema document to contain an object `properties` field",
            )
        })
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

fn find_def<'a>(defs: &'a Map<String, Value>, def_name: &str) -> Result<&'a Value> {
    defs.iter()
        .find_map(|(key, value)| {
            let cleaned = key.trim_start_matches('#');
            if cleaned == def_name || key == def_name {
                Some(value)
            } else {
                None
            }
        })
        .ok_or_else(|| Error::new(format!("missing schema definition for {def_name}")))
}

fn schema_to_wit_type(
    logical_name: &str,
    schema: &Value,
    defs: &Map<String, Value>,
    emitted: &mut BTreeMap<String, String>,
    order: &mut Vec<String>,
) -> Result<String> {
    if schema.get("properties").is_some()
        || schema.get("type").and_then(Value::as_str) == Some("object")
    {
        return generate_named_record(logical_name, schema, defs, emitted, order);
    }

    if schema.get("enum").is_some() {
        return Ok("string".to_string());
    }

    if let Some(one_of) = schema.get("oneOf").and_then(Value::as_array) {
        if let Some(first) = one_of.first() {
            return schema_to_wit_type(logical_name, first, defs, emitted, order);
        }
    }

    if let Some(reference) = schema.get("$ref").and_then(Value::as_str) {
        let def_name = decode_ref_name(reference);
        let def_schema = find_def(defs, &def_name)?;
        return schema_to_wit_type(&def_name, def_schema, defs, emitted, order);
    }

    match schema.get("type").and_then(Value::as_str) {
        Some("string") => Ok("string".to_string()),
        Some("integer") => Ok("s64".to_string()),
        Some("number") => Ok("f64".to_string()),
        Some("boolean") => Ok("bool".to_string()),
        Some("array") => {
            let item_type = match schema.get("items") {
                Some(items) => schema_to_wit_type(
                    &format!("{logical_name}-item"),
                    items,
                    defs,
                    emitted,
                    order,
                )?,
                None => "string".to_string(),
            };
            Ok(format!("list<{item_type}>"))
        }
        Some(other) => Err(Error::new(format!(
            "unsupported schema type `{other}` in {logical_name}"
        ))),
        None => Ok("string".to_string()),
    }
}

fn generate_named_record(
    logical_name: &str,
    schema: &Value,
    defs: &Map<String, Value>,
    emitted: &mut BTreeMap<String, String>,
    order: &mut Vec<String>,
) -> Result<String> {
    let record_name = escape_wit_ident(&to_kebab_case(logical_name));

    if emitted.contains_key(&record_name) {
        return Ok(record_name);
    }

    let properties = schema
        .get("properties")
        .and_then(Value::as_object)
        .ok_or_else(|| Error::new(format!("expected object schema for {logical_name}")))?;

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
        )?;

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
    Ok(record_name)
}

fn resolve_root_type(
    schema: &Value,
    fallback_name: &str,
    emitted: &mut BTreeMap<String, String>,
    order: &mut Vec<String>,
) -> Result<String> {
    let empty_defs = Map::new();
    let defs = schema
        .get("$defs")
        .and_then(Value::as_object)
        .unwrap_or(&empty_defs);

    let has_inline_shape = schema.get("properties").is_some()
        || schema.get("enum").is_some()
        || schema.get("oneOf").is_some()
        || schema.get("type").is_some();

    let logical_name = if has_inline_shape {
        fallback_name.to_string()
    } else {
        schema
            .get("$ref")
            .and_then(Value::as_str)
            .map(decode_ref_name)
            .unwrap_or_else(|| fallback_name.to_string())
    };

    if let Some(reference) = schema.get("$ref").and_then(Value::as_str) {
        if has_inline_shape {
            schema_to_wit_type(&logical_name, schema, defs, emitted, order)
        } else {
            if defs.is_empty() {
                return Err(Error::new("schema uses `$ref` but does not define `$defs`"));
            }

            let def_name = decode_ref_name(reference);
            let root_schema = find_def(defs, &def_name)?;
            schema_to_wit_type(&logical_name, root_schema, defs, emitted, order)
        }
    } else {
        schema_to_wit_type(&logical_name, schema, defs, emitted, order)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn generates_wit_for_input_and_output_records() {
        let input = json!({
            "$defs": {
                "Input": {
                    "type": "object",
                    "properties": {
                        "name": { "type": "string" },
                        "age": { "type": "integer" }
                    },
                    "required": ["name"]
                }
            },
            "$ref": "#/$defs/Input"
        });
        let output = json!({
            "$defs": {
                "Output": {
                    "type": "object",
                    "properties": {
                        "message": { "type": "string" }
                    },
                    "required": ["message"]
                }
            },
            "$ref": "#/$defs/Output"
        });

        let wit =
            generate_wit(&input, &output, &WitConfig::default()).expect("WIT should be generated");

        assert!(wit.contains("package acme:app@0.1.0;"));
        assert!(wit.contains("interface api"));
        assert!(wit.contains("record input"));
        assert!(wit.contains("name: string"));
        assert!(wit.contains("age: option<s64>"));
        assert!(wit.contains("record output"));
        assert!(wit.contains("run: func(input: input) -> output;"));
        assert!(wit.contains("world component"));
    }

    #[test]
    fn supports_nested_objects_and_arrays() {
        let input = json!({
            "$defs": {
                "Input": {
                    "type": "object",
                    "properties": {
                        "items": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "label": { "type": "string" }
                                },
                                "required": ["label"]
                            }
                        }
                    }
                }
            },
            "$ref": "#/$defs/Input"
        });
        let output = json!({
            "$defs": {
                "Output": {
                    "type": "object",
                    "properties": {
                        "ok": { "type": "boolean" }
                    },
                    "required": ["ok"]
                }
            },
            "$ref": "#/$defs/Output"
        });

        let wit =
            generate_wit(&input, &output, &WitConfig::default()).expect("WIT should be generated");

        assert!(wit.contains("record input-items-item"));
        assert!(wit.contains("items: option<list<input-items-item>>"));
        assert!(wit.contains("ok: bool"));
    }

    #[test]
    fn supports_root_objects_without_defs() {
        let input = json!({
            "type": "object",
            "properties": {
                "prompt": { "type": "string" }
            },
            "required": ["prompt"]
        });
        let output = json!({
            "type": "object",
            "properties": {
                "answer": { "type": "string" }
            },
            "required": ["answer"]
        });

        let wit =
            generate_wit(&input, &output, &WitConfig::default()).expect("WIT should be generated");

        assert!(wit.contains("record input"));
        assert!(wit.contains("prompt: string"));
        assert!(wit.contains("record output"));
        assert!(wit.contains("answer: string"));
    }

    #[test]
    fn prefers_inline_root_object_over_placeholder_ref() {
        let input = json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "$defs": {
                "#Schema": {
                    "const": {}
                }
            },
            "$ref": "#/$defs/%23Schema",
            "type": "object",
            "properties": {
                "name": { "type": "string" }
            },
            "required": ["name"]
        });
        let output = json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "$defs": {
                "#Schema": {
                    "const": {}
                }
            },
            "$ref": "#/$defs/%23Schema",
            "type": "object",
            "properties": {
                "message": { "type": "string" }
            },
            "required": ["message"]
        });

        let wit =
            generate_wit(&input, &output, &WitConfig::default()).expect("WIT should be generated");

        assert!(wit.contains("record input"));
        assert!(wit.contains("name: string"));
        assert!(wit.contains("record output"));
        assert!(wit.contains("message: string"));
        assert!(wit.contains("run: func(input: input) -> output;"));
    }

    #[test]
    fn generates_multiple_functions_from_all_schemas() {
        let all_schemas = json!({
            "type": "object",
            "properties": {
                "PersonalData": {
                    "type": "object",
                    "properties": {
                        "name": { "type": "string" }
                    },
                    "required": ["name"]
                },
                "Message": {
                    "type": "object",
                    "properties": {
                        "text": { "type": "string" }
                    },
                    "required": ["text"]
                },
                "SearchInput": {
                    "type": "object",
                    "properties": {
                        "query": { "type": "string" }
                    },
                    "required": ["query"]
                },
                "SearchOutput": {
                    "type": "object",
                    "properties": {
                        "hits": { "type": "integer" }
                    },
                    "required": ["hits"]
                }
            }
        });

        let tools = vec![
            ToolFunction {
                name: "greeter-tool".to_string(),
                input_schema_name: "PersonalData".to_string(),
                output_schema_name: "Message".to_string(),
            },
            ToolFunction {
                name: "search-tool".to_string(),
                input_schema_name: "SearchInput".to_string(),
                output_schema_name: "SearchOutput".to_string(),
            },
        ];

        let wit = generate_wit_for_tools(&all_schemas, &tools, &WitConfig::default())
            .expect("WIT should be generated");

        assert!(wit.contains("record personal-data"));
        assert!(wit.contains("record message"));
        assert!(wit.contains("record search-input"));
        assert!(wit.contains("record search-output"));
        assert!(wit.contains("greeter-tool: func(input: personal-data) -> message;"));
        assert!(wit.contains("search-tool: func(input: search-input) -> search-output;"));
    }

    #[test]
    fn normalizes_camel_case_input_keys_to_snake_case() {
        let mut value = json!({
            "petType": "cat",
            "ownerData": {
                "firstName": "Ada"
            }
        });

        normalize_json_keys_to_snake(&mut value);

        assert_eq!(
            value,
            json!({
                "pet_type": "cat",
                "owner_data": {
                    "first_name": "Ada"
                }
            })
        );
    }

    #[test]
    fn remaps_output_keys_to_schema_shape() {
        let mut value = json!({
            "pet_type": "cat",
            "owner_data": {
                "first_name": "Ada"
            }
        });
        let schema = json!({
            "type": "object",
            "properties": {
                "petType": { "type": "string" },
                "ownerData": {
                    "type": "object",
                    "properties": {
                        "firstName": { "type": "string" }
                    }
                }
            }
        });

        remap_json_keys_to_schema(&mut value, &schema);

        assert_eq!(
            value,
            json!({
                "petType": "cat",
                "ownerData": {
                    "firstName": "Ada"
                }
            })
        );
    }
}
