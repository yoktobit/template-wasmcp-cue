# template-wasmcp-cue-new

MCP tools capability component in Rust.

## Build

```bash
mise run liscaf-merge  # converts cue to jsonschema and generates rust files
wash build  # runs wasmCloud build
```

## Run

```bash
# HTTP
wash dev
```

## Adding Tools

To add new tools:

1. create input/output schema in config.cue
2. create tool in config.cue
3. link input/output schema in tool def
4. run mise liscaf-merge
5. implement the now missing functions in implementation.rs
6. new types are in the module json_bindings
