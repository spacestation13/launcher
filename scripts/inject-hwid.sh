#!/usr/bin/env bash

set -euo pipefail

CRATE_DIR="$(dirname "$0")/../crates/hwid"

cat > "$CRATE_DIR/Cargo.toml" << 'EOF'
[package]
name = "launcher-hwid"
version = "0.1.0"
edition = "2021"

[dependencies]
ss13-hwid = { git = "ssh://git@github.com/spacestation13/ss13hwid.git", branch = "main" }
EOF

cat > "$CRATE_DIR/src/lib.rs" << 'EOF'
pub use ss13_hwid::*;
EOF
