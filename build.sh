#!/bin/bash

# Build script for Flowsta Identity DNA

set -e

echo "🔨 Building Flowsta Identity DNA"

# Check if Holochain CLI is installed
if ! command -v hc &> /dev/null; then
    echo "❌ Error: Holochain CLI (hc) not found"
    echo "Install with: cargo install holochain_cli --version 0.5.6"
    exit 1
fi

# Create workdir if it doesn't exist
mkdir -p workdir/dnas

# Build all zomes
echo "📦 Building zomes..."
CARGO_TARGET_DIR=target cargo build --release --target wasm32-unknown-unknown

# Copy wasm files to workdir
echo "📋 Copying WASM files..."
mkdir -p zomes/users zomes/sites

cp target/wasm32-unknown-unknown/release/users_integrity.wasm zomes/users/integrity.wasm
cp target/wasm32-unknown-unknown/release/users_coordinator.wasm zomes/users/coordinator.wasm
cp target/wasm32-unknown-unknown/release/sites_integrity.wasm zomes/sites/integrity.wasm
cp target/wasm32-unknown-unknown/release/sites_coordinator.wasm zomes/sites/coordinator.wasm

# Pack the DNA
echo "🎁 Packing DNA..."
hc dna pack workdir

# Pack the hApp
echo "📦 Packing hApp..."
hc app pack workdir

echo "✅ Build complete!"
echo ""
echo "Outputs:"
echo "  - DNA: workdir/dnas/flowsta_identity.dna"
echo "  - hApp: workdir/flowsta_auth.happ"

