#!/usr/bin/env bash
set -euo pipefail

echo "==> Running demo..."
cargo run --example demo "$@"
