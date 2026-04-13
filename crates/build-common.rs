use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use jsonschema_to_wit::{
    generate_wit_for_tools_from_file, normalize_trivial_aliases, ToolFunction, WitConfig,
};
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct ToolBinding {
    pub tool_name: String,
    pub handler: String,
    pub input_schema_name: String,
    pub output_schema_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ToolHandler {
    pub namespace: String,
    pub package: String,
    pub interface: String,
    pub version: Option<String>,
}

impl ToolHandler {
    pub fn package_ref(&self) -> String {
        format!("{}:{}", self.namespace, self.package)
    }

    pub fn import_ref(&self, default_version: &str) -> String {
        let version = self
            .version
            .as_deref()
            .unwrap_or(default_version);
        format!("{}/{}@{}", self.package_ref(), self.interface, version)
    }

    pub fn rust_module_path(&self) -> String {
        format!(
            "{}::{}::{}",
            jsonschema_to_wit::to_snake_case(&self.namespace),
            jsonschema_to_wit::to_snake_case(&self.package),
            jsonschema_to_wit::to_snake_case(&self.interface)
        )
    }
}

pub fn parse_tool_handler(handler: &str) -> ToolHandler {
    let (package_ref, interface_ref) = handler
        .split_once('/')
        .unwrap_or_else(|| panic!("invalid handler `{handler}`: expected `namespace:package/interface`"));
    let (namespace, package) = package_ref
        .split_once(':')
        .unwrap_or_else(|| panic!("invalid handler `{handler}`: expected `namespace:package/interface`"));
    let (interface, version) = if let Some((interface, version)) = interface_ref.split_once('@') {
        (interface, Some(version.to_string()))
    } else {
        (interface_ref, None)
    };

    assert!(
        !namespace.is_empty() && !package.is_empty() && !interface.is_empty(),
        "invalid handler `{handler}`: expected non-empty `namespace`, `package`, and `interface`"
    );

    ToolHandler {
        namespace: namespace.to_string(),
        package: package.to_string(),
        interface: interface.to_string(),
        version,
    }
}

pub fn write_if_changed(destination: &Path, contents: &str) {
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

pub fn copy_if_changed(source: &str, destination: &str) {
    let source_path = Path::new(source);
    let destination_path = Path::new(destination);
    let contents = fs::read_to_string(source_path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", source_path.display()));

    write_if_changed(destination_path, &contents);
}

pub fn run_cue(
    workspace_dir: &Path,
    command: &str,
    expr: &str,
    format: &str,
    output_file: &str,
) {
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

pub fn cue_export_json(workspace_dir: &Path, expr: &str) -> Value {
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

pub fn export_tool_constants(workspace_dir: &Path, destination: &Path) {
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

pub fn tool_bindings_for_component(tools: &Value) -> Vec<ToolBinding> {
    let tool_map = tools.as_object().unwrap_or_else(|| {
        panic!(
            "expected `{}` to contain a JSON object",
            "cue export McpTools"
        )
    });

    let mut bindings = Vec::with_capacity(tool_map.len());
    for (tool_name, tool) in tool_map {
        let handler = tool
            .get("handler")
            .and_then(Value::as_str)
            .unwrap_or("acme:greeter/api@0.1.0");
        parse_tool_handler(handler);

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
            handler: handler.to_string(),
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

pub fn find_workspace_dir(manifest_dir: &Path) -> PathBuf {
    for candidate in manifest_dir.ancestors() {
        if candidate.join("schema.cue").exists() {
            return candidate.to_path_buf();
        }
    }

    panic!(
        "failed to find workspace root from {} (expected an ancestor containing schema.cue)",
        manifest_dir.display()
    );
}

pub fn select_tools_for_handler(
    all_tool_bindings: Vec<ToolBinding>,
    handler: &ToolHandler,
) -> Vec<ToolBinding> {
    all_tool_bindings
        .into_iter()
        .filter(|tool| {
            let tool_handler = parse_tool_handler(&tool.handler);
            tool_handler.namespace == handler.namespace
                && tool_handler.package == handler.package
                && tool_handler.interface == handler.interface
        })
        .collect::<Vec<_>>()
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

fn generate_component_wit(
    manifest_dir: &Path,
    all_schemas_path: &Path,
    tool_bindings: &[ToolBinding],
    handler: &ToolHandler,
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
            package: handler.package_ref(),
            version,
            interface: handler.interface.clone(),
            world: "component".to_string(),
            function: "run".to_string(),
        },
    )
    .unwrap_or_else(|e| panic!("failed to generate {}: {e}", wit_path.display()));

    write_if_changed(&wit_path, &wit);
}

pub fn run_component_codegen(manifest_dir: &Path, handler_env_var: &str, default_handler: &str) {
    let workspace_dir = find_workspace_dir(manifest_dir);
    let all_schemas_path = workspace_dir.join("_all_schemas.schema.json");

    println!(
        "cargo:rerun-if-changed={}",
        workspace_dir.join("schema.cue").display()
    );
    println!("cargo:rerun-if-env-changed={handler_env_var}");

    let tools: Value = cue_export_json(&workspace_dir, "McpTools");
    let all_tool_bindings = tool_bindings_for_component(&tools);
    let handler_ref = env::var(handler_env_var).unwrap_or_else(|_| default_handler.to_string());
    let handler = parse_tool_handler(&handler_ref);
    let tool_bindings = select_tools_for_handler(all_tool_bindings, &handler);

    assert!(
        !tool_bindings.is_empty(),
        "no tools matched {handler_env_var}={handler_ref}; update schema.cue or environment"
    );

    run_cue(
        &workspace_dir,
        "def",
        "Schemas",
        "jsonschema",
        "_all_schemas.schema.json",
    );
    normalize_schema_aliases(&all_schemas_path);
    generate_component_wit(manifest_dir, &all_schemas_path, &tool_bindings, &handler);
}
