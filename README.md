# acme-app workspace

This project is now split into **two crates** so the business component can be reused outside MCP:

| Crate | Purpose |
| --- | --- |
| `crates/component` | Reusable WASI component exporting the app-specific WIT interface |
| `crates/jsonschema-to-wit` | Shared converter crate that turns JSON Schema into the component WIT contract |
| `crates/mcp-server` | Thin MCP adapter that exposes the component as a `tools` capability |

## Build

```bash
make setup
make build
```

Built artifacts:

- `target/wasm32-wasip2/release/acme_component.wasm`
- `target/wasm32-wasip2/release/acme_mcp.wasm`

## Compose the MCP server

```bash
wasmcp compose server target/wasm32-wasip2/release/acme_mcp.wasm -o server.wasm
```

## Run

```bash
# HTTP
wasmtime serve -Scli server.wasm

# Stdio
wasmcp compose server target/wasm32-wasip2/release/acme_mcp.wasm -t stdio -o server.wasm
wasmtime run server.wasm
```

## Layout

- the root `schema.cue` is the source of truth for MCP tools and schema definitions
- the root `schema.cue` generates `_all_schemas.schema.json` used for per-tool MCP input/output schemas
- `crates/jsonschema-to-wit` converts `_all_schemas.schema.json` plus tool bindings into `crates/component/wit/world.wit`
- `crates/mcp-server` uses the root schemas for MCP tool metadata and dispatches each tool call to the generated component interface function

This keeps the application contract reusable in non-MCP compositions while still producing a ready-to-run MCP server component.
