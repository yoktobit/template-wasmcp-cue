# acme-app workspace

This workspace builds multiple reusable WASI components and exposes them through one MCP server component.

## Current model (important)

1. `wit/world.wit` is source-of-truth for each application component crate.
2. `schema.cue` is source-of-truth for MCP tool metadata (tool names, descriptions, handler mapping, schema names).
3. `component2json` output from compiled component wasm files is used by `mcp-server` for tool input/output schemas.

In short:

- component interface contract: WIT files in each component crate
- tool catalog and routing: `schema.cue`
- runtime tool schema payloads shown by MCP: generated from compiled component wasm exports

## Workspace crates

| Crate | Purpose |
| --- | --- |
| `crates/component` | Greeter WASI component exporting `acme:greeter/api` |
| `crates/component-pet` | Pet health WASI component exporting `acme:pet/api` |
| `crates/c2s` | Converts one or more component wasm artifacts into merged tool schema JSON |
| `crates/mcp-server` | MCP adapter component (list/call tools, dispatches to component imports) |
| `crates/jsonschema-to-wit` | Shared helper crate used by build scripts/tooling |

## Prerequisites

```bash
make setup
```

Also required:

- `mise` + `cue` (used by build scripts)
- `wasmcp` and `wasmtime` (for compose/run)
- local `wassette` checkout because `crates/c2s` currently depends on local `component2json`

Current expected local path for that dependency:

- `/home/martin/dev/wassette/wassette`

## Build correctly

### Recommended: full build

```bash
make build
```

This builds workspace crates and keeps component-pet built first.

### Build MCP adapter with fresh component-derived schemas

```bash
make build-mcp
```

This runs:

1. build components (`acme-component-pet`, `acme-component`)
2. generate merged schemas from compiled wasm exports via `c2s`
3. build `acme-mcp`

Generated schema registry consumed by `mcp-server`:

- `crates/mcp-server/src/generated/component-tools.json`

## Produced artifacts

- `target/wasm32-wasip2/release/acme_component.wasm`
- `target/wasm32-wasip2/release/acme_component_pet.wasm`
- `target/wasm32-wasip2/release/acme_mcp.wasm`

## Compose and run

```bash
wasmcp compose server target/wasm32-wasip2/release/acme_mcp.wasm -o server.wasm
```

```bash
# HTTP
wasmtime serve -Scli server.wasm

# Stdio
wasmcp compose server target/wasm32-wasip2/release/acme_mcp.wasm -t stdio -o server.wasm
wasmtime run server.wasm
```

## How to add a new tool to an existing component

1. Add/update function in that component's `wit/world.wit`.
2. Implement the function in that component's `src/lib.rs` guest impl.
3. Add tool entry in `schema.cue` under `Tools`:
	- `name`
	- `description`
	- `handler` (must match WIT package/interface/version)
	- `inputSchema` and `outputSchema`
4. Add or update schemas under `Schemas` in `schema.cue`.
5. Run:

```bash
make build-mcp
```

6. Verify `list_tools` now includes the new tool.

## How to add a new component crate

1. Create crate under `crates/<new-component>` with:
	- `src/lib.rs` using `wit_bindgen::generate!` with `path: "wit"`
	- `wit/world.wit` defining the exported interface/world
	- a minimal `build.rs` that only tracks source files:

```rust
fn main() {
	 println!("cargo:rerun-if-changed=wit/world.wit");
	 println!("cargo:rerun-if-changed=src/lib.rs");
}
```

2. Add the crate to workspace members in root `Cargo.toml`.
3. Add tool entries in `schema.cue` with `handler` pointing to the new component interface.
4. Wire the new component WIT into `crates/mcp-server/build.rs` so deps are copied into:
	- `crates/mcp-server/wit/deps`
	- `crates/mcp-server/wit/component-client/deps`
5. Include the new component wasm artifact in `generate-component-tool-schemas` in `Makefile`.
6. Run:

```bash
make build-mcp
```

7. Verify tool appears in `list_tools` and can be called.

## Consistency checklist

When adding/changing tools, keep these in sync:

1. Component WIT signature (`wit/world.wit`)
2. Component Rust implementation (`src/lib.rs`)
3. MCP tool declaration and handler (`schema.cue`)
4. MCP dependency wiring (`crates/mcp-server/build.rs` for new interfaces)
5. Schema generation inputs (`Makefile` component wasm list)

## Troubleshooting

### Tool listed but schema looks wrong

Regenerate schema registry and rebuild server:

```bash
make generate-component-tool-schemas
cargo check -p acme-mcp --target wasm32-wasip2
```

### Build fails in `c2s` with missing `component2json`

Ensure local `wassette` checkout exists at the path expected by `crates/c2s/Cargo.toml`.

### Tool dispatch missing in `call_tool`

Ensure the tool is present in `schema.cue` and handler matches imported interface/package/version.
