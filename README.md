# acme-app workspace

This project is now split into **two crates** so the business component can be reused outside MCP:

| Crate | Purpose |
| --- | --- |
| `crates/component` | Reusable WASI component exporting the app-specific WIT interface |
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

- `crates/component/schema.cue` is the source of truth for the input/output schema
- `crates/component/wit/world.wit` is generated automatically from that schema during build
- `crates/mcp-server` contains only the MCP-facing adapter logic

This keeps the application contract reusable in non-MCP compositions while still producing a ready-to-run MCP server component.
