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
mkdir -p workdir/zomes/users workdir/zomes/sites

cp target/wasm32-unknown-unknown/release/users_integrity.wasm workdir/zomes/users/integrity.wasm
cp target/wasm32-unknown-unknown/release/users_coordinator.wasm workdir/zomes/users/coordinator.wasm
cp target/wasm32-unknown-unknown/release/sites_integrity.wasm workdir/zomes/sites/integrity.wasm
cp target/wasm32-unknown-unknown/release/sites_coordinator.wasm workdir/zomes/sites/coordinator.wasm

# Copy DNA and hApp manifests to workdir
cp dna.yaml workdir/
cp happ.yaml workdir/

# Pack the DNA
echo "🎁 Packing DNA..."
hc dna pack workdir

# Pack the hApp
echo "📦 Packing hApp..."
hc app pack workdir

echo "✅ Build complete!"
echo ""
echo "Outputs:"
echo "  - DNA: workdir/flowsta_identity_v1_0.dna"
echo "  - hApp: workdir/flowsta_identity_v1_0_happ.happ"

