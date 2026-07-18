#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MODE="${1:-dev}"
BUILD_TARGET_DIR="${HEN_LOCAL_CARGO_TARGET_DIR:-${TMPDIR:-/tmp}/hen-local-translator-cargo-target}"

if [[ "$BUILD_TARGET_DIR" == *" "* ]]; then
  echo "Hen Local Translator's Cargo target directory cannot contain spaces: $BUILD_TARGET_DIR"
  echo "Set HEN_LOCAL_CARGO_TARGET_DIR to a path without spaces."
  exit 1
fi

mkdir -p "$BUILD_TARGET_DIR"
export CARGO_TARGET_DIR="$BUILD_TARGET_DIR"
export MOXIN_DORA_TARGET_DIR="$BUILD_TARGET_DIR"

case "$MODE" in
  dev)
    echo "Building local translation nodes in $BUILD_TARGET_DIR..."
    cargo build \
      --manifest-path "$ROOT_DIR/Cargo.toml" \
      -p dora-qwen3-asr \
      -p dora-qwen35-translator
    cd "$ROOT_DIR/hen-local-translator-shell"
    exec ./ui/node_modules/.bin/tauri dev
    ;;
  build)
    echo "Building release translation nodes in $BUILD_TARGET_DIR..."
    cargo build \
      --manifest-path "$ROOT_DIR/Cargo.toml" \
      --release \
      -p dora-qwen3-asr \
      -p dora-qwen35-translator
    cd "$ROOT_DIR/hen-local-translator-shell"
    exec ./ui/node_modules/.bin/tauri build
    ;;
  *)
    echo "Usage: $(basename "$0") [dev|build]"
    exit 1
    ;;
esac
