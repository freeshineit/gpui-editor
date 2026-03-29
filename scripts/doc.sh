#!/usr/bin/env bash
set -euo pipefail

echo "==> Generating documentation..."
cargo doc --no-deps --open "$@"
echo "==> Docs generated."
