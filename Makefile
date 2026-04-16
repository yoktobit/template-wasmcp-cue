.PHONY: all setup build build-component build-component-pet generate-component-tool-schemas build-mcp clean

# Default target
all: build

# Install required tools
setup:
	@rustup target add wasm32-wasip2

# Build both workspace crates
build: build-component-pet
	@cargo build --workspace --target wasm32-wasip2 --release

# Build the reusable application component only
build-component:
	@cargo build -p acme-component --target wasm32-wasip2 --release

# Build the pet-health component only
build-component-pet:
	@cargo build -p acme-component-pet --target wasm32-wasip2 --release

# Generate MCP tool schemas from compiled component wasm exports
generate-component-tool-schemas: build-component-pet build-component
	@mkdir -p crates/mcp-server/src/generated
	@cargo run -p c2s -- \
		target/wasm32-wasip2/release/acme_component_pet.wasm \
		target/wasm32-wasip2/release/acme_component.wasm \
		crates/mcp-server/src/generated/component-tools.json

# Build the MCP adapter component only
build-mcp: generate-component-tool-schemas
	@cargo build -p acme-mcp --target wasm32-wasip2 --release

# Clean build artifacts
clean:
	@cargo clean
