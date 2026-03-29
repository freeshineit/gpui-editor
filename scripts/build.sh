#!/usr/bin/env bash
set -euo pipefail

echo "==> Building gpui-editor..."
cargo build "$@"
echo "==> Build succeeded."
