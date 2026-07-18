#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BUILD_TARGET_DIR="${HEN_LOCAL_CARGO_TARGET_DIR:-${TMPDIR:-/tmp}/hen-local-translator-cargo-target}"

if [[ "$BUILD_TARGET_DIR" == *" "* ]]; then
  echo "Cargo target directory cannot contain spaces: $BUILD_TARGET_DIR"
  exit 1
fi

mkdir -p "$BUILD_TARGET_DIR"
export CARGO_TARGET_DIR="$BUILD_TARGET_DIR"

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
  cat <<'EOF'
Usage:
  ./scripts/test_qwen35_translator_prompt.sh [--mode normal|final-drain] [--tgt-lang LANG] TEXT
  ./scripts/test_qwen35_translator_prompt.sh [--mode normal|final-drain] [--tgt-lang LANG] --file /path/to/raw_tail.txt

Examples:
  ./scripts/test_qwen35_translator_prompt.sh "大家下午好我叫鲍月然后来自华为"
  ./scripts/test_qwen35_translator_prompt.sh --mode final-drain --file /tmp/raw_tail.txt
EOF
  exit 0
fi

cd "$ROOT_DIR"

exec cargo run -p dora-qwen35-translator --bin translator-prompt-harness -- "$@"
