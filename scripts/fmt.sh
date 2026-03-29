#!/usr/bin/env bash
set -euo pipefail

echo "==> Formatting code..."
cargo fmt "$@"
echo "==> Format complete."
