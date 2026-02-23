#!/bin/bash

# Build script for Flowsta Identity DNA v1.3

set -e

echo "Building Flowsta Identity DNA v1.3"

# Check if Holochain CLI is installed
if ! command -v hc &> /dev/null; then
    echo "Error: Holochain CLI (hc) not found"
    echo "Install with: cargo install holochain_cli --version 0.6.0"
    exit 1
fi

# Create workdir if it doesn't exist
mkdir -p workdir

# Build all zomes
echo "Building zomes..."
RUSTFLAGS='--cfg getrandom_backend="custom"' CARGO_TARGET_DIR=target cargo build --release --target wasm32-unknown-unknown

# Copy wasm files flat to workdir (manifest_version "0" expects flat paths)
echo "Copying WASM files..."
cp target/wasm32-unknown-unknown/release/users_integrity.wasm workdir/
cp target/wasm32-unknown-unknown/release/users_coordinator.wasm workdir/
cp target/wasm32-unknown-unknown/release/sites_integrity.wasm workdir/
cp target/wasm32-unknown-unknown/release/sites_coordinator.wasm workdir/
cp target/wasm32-unknown-unknown/release/agent_linking_integrity.wasm workdir/
cp target/wasm32-unknown-unknown/release/agent_linking_coordinator.wasm workdir/

# Pack the DNA
echo "Packing DNA..."
hc dna pack workdir

# Pack the hApp
echo "Packing hApp..."
hc app pack workdir

echo "Build complete!"
echo ""
echo "Outputs:"
echo "  - DNA: workdir/flowsta_identity_v1_3.dna"
echo "  - hApp: workdir/flowsta_identity_v1_3_happ.happ"
