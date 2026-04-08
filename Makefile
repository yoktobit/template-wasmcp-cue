.PHONY: all setup build build-component build-mcp clean

# Default target
all: build

# Install required tools
setup:
	@rustup target add wasm32-wasip2

# Build both workspace crates
build:
	@cargo build --workspace --target wasm32-wasip2 --release

# Build the reusable application component only
build-component:
	@cargo build -p acme-component --target wasm32-wasip2 --release

# Build the MCP adapter component only
build-mcp:
	@cargo build -p acme-mcp --target wasm32-wasip2 --release

# Clean build artifacts
clean:
	@cargo clean
