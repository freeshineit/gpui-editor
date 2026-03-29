#!/usr/bin/env bash
set -euo pipefail

echo "==> Checking code (lint)..."
cargo clippy --all-targets --all-features -- -D warnings "$@"
echo "==> Lint passed."
