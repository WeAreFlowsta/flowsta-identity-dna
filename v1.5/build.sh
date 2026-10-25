#!/bin/bash

# Build script for Flowsta Identity DNA v1.5
#
# v1.5 is the Holochain 0.7 generation of v1.4 (new DNA hash, so a new
# network): users and sites unchanged, agent_linking from the
# flowsta-agent-linking crate (branch holochain-0.7: integrity 0.3.0,
# coordinator 0.4.0, with the 2026-09 validation fixes), and the new
# `devices` zome: EnrollmentKey, EnrollmentCancelled and DeviceRemoved
# records validated as planning/DEVICE_CERTIFICATES_0_7.md specifies.
#
# Prerequisites:
#   - hc CLI 0.7.1 (cargo install holochain_cli --version 0.7.1), reachable
#     as `hc-0.7` or named in $HC
#   - Rust 1.94 or newer with the wasm32 target (rust-toolchain.toml pins it)
#   - flowsta-agent-linking on its holochain-0.7 line, cloned at
#     ../../flowsta-agent-linking/

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
AGENT_LINKING_DIR="$SCRIPT_DIR/../../flowsta-agent-linking"

echo "Building Flowsta Identity DNA v1.5"

if ! command -v ${HC:-hc-0.7} &> /dev/null; then
    echo "Error: Holochain CLI (hc) not found"
    echo "Install with: cargo install holochain_cli --version 0.7.1"
    exit 1
fi
if [ ! -d "$AGENT_LINKING_DIR" ]; then
    echo "Error: flowsta-agent-linking not found at $AGENT_LINKING_DIR"
    exit 1
fi

mkdir -p workdir

echo "Building agent_linking zomes from flowsta-agent-linking..."
RUSTFLAGS='--cfg getrandom_backend="custom"' CARGO_TARGET_DIR="$AGENT_LINKING_DIR/target" \
    cargo build --release --target wasm32-unknown-unknown \
    --manifest-path "$AGENT_LINKING_DIR/Cargo.toml"

echo "Building users, sites and devices zomes..."
RUSTFLAGS='--cfg getrandom_backend="custom"' CARGO_TARGET_DIR=target \
    cargo build --release --target wasm32-unknown-unknown

echo "Copying WASM files and manifests..."
cp "$AGENT_LINKING_DIR/target/wasm32-unknown-unknown/release/flowsta_agent_linking_integrity.wasm" workdir/agent_linking_integrity.wasm
cp "$AGENT_LINKING_DIR/target/wasm32-unknown-unknown/release/flowsta_agent_linking_coordinator.wasm" workdir/agent_linking_coordinator.wasm
for z in users_integrity users_coordinator sites_integrity sites_coordinator devices_integrity devices_coordinator; do
    cp "target/wasm32-unknown-unknown/release/$z.wasm" workdir/
done
cp dna.yaml happ.yaml workdir/

echo "Packing DNA..."
${HC:-hc-0.7} dna pack workdir
echo "Packing hApp..."
${HC:-hc-0.7} app pack workdir

echo ""
echo "Build complete!"
echo "  - DNA: workdir/flowsta_identity_v1_5.dna"
echo "  - hApp: workdir/flowsta_identity_v1_5_happ.happ"
sha256sum workdir/flowsta_identity_v1_5_happ.happ workdir/flowsta_identity_v1_5.dna
