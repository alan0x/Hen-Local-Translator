#!/usr/bin/env bash
# macos_bootstrap.sh — translator-only, no conda/Python required.
#
# Model download is handled by the bundled `hen-local-init` Rust binary.
# It defaults to automatic source selection: ModelScope first when reachable,
# with Hugging Face as a fallback when that path is also reachable.
#
# This script is intentionally minimal: locate hen-local-init, pass
# environment variables, and exec it. All progress reporting is done
# by hen-local-init itself (writes bootstrap_state.txt directly).
set -euo pipefail

APP_RESOURCES="${HEN_LOCAL_APP_RESOURCES:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
STATE_PATH="${HEN_LOCAL_BOOTSTRAP_STATE_PATH:-$HOME/Library/Logs/HenLocalTranslator/bootstrap_state.txt}"

QWEN_ASR_DIR="${QWEN3_ASR_MODEL_PATH:-$HOME/.OminiX/models/qwen3-asr-1.7b}"
QWEN35_TRANSLATOR_DIR="${QWEN35_TRANSLATOR_MODEL_PATH:-$HOME/.OminiX/models/Qwen3.5-2B-MLX-4bit}"

QWEN_ASR_REPO="${QWEN3_ASR_REPO:-mlx-community/Qwen3-ASR-1.7B-8bit}"
QWEN35_TRANSLATOR_REPO="${QWEN35_TRANSLATOR_REPO:-mlx-community/Qwen3.5-2B-MLX-4bit}"

# Locate the hen-local-init binary: app bundle first, then dev build trees.
resolve_hen_local_init() {
  if [[ -x "$APP_RESOURCES/../MacOS/hen-local-init" ]]; then
    echo "$APP_RESOURCES/../MacOS/hen-local-init"; return 0
  fi
  for profile in release debug; do
    if [[ -x "$APP_RESOURCES/target/$profile/hen-local-init" ]]; then
      echo "$APP_RESOURCES/target/$profile/hen-local-init"; return 0
    fi
  done
  return 1
}

if ! MOXIN_INIT="$(resolve_hen_local_init)"; then
  echo "ERROR: hen-local-init binary not found." >&2
  echo "  Checked: $APP_RESOURCES/../MacOS/hen-local-init" >&2
  echo "  Checked: $APP_RESOURCES/target/{release,debug}/hen-local-init" >&2
  echo "  Run: cargo build -p hen-local-init --release" >&2
  exit 1
fi

echo "=== Hen Local Translator Bootstrap (hen-local-init) ==="
echo "hen-local-init: $MOXIN_INIT"
echo "ASR model dir: $QWEN_ASR_DIR"
echo "Qwen3.5 translator dir: $QWEN35_TRANSLATOR_DIR"
echo ""

MODEL_PROVIDER_VALUE="${MOXIN_MODEL_PROVIDER:-auto}"
MODELSCOPE_ENDPOINT_VALUE="${MOXIN_MODELSCOPE_ENDPOINT:-}"
HF_ENDPOINT_VALUE="${HF_ENDPOINT:-}"

echo "Model provider: $MODEL_PROVIDER_VALUE"
if [[ -n "$MODELSCOPE_ENDPOINT_VALUE" ]]; then
  echo "ModelScope endpoint: $MODELSCOPE_ENDPOINT_VALUE"
fi
if [[ -n "$HF_ENDPOINT_VALUE" ]]; then
  echo "Hugging Face endpoint: $HF_ENDPOINT_VALUE"
fi
echo ""

if [[ -n "$HF_ENDPOINT_VALUE" ]]; then
  exec env \
    HEN_LOCAL_BOOTSTRAP_STATE_PATH="$STATE_PATH" \
    MOXIN_MODEL_PROVIDER="$MODEL_PROVIDER_VALUE" \
    MOXIN_MODELSCOPE_ENDPOINT="$MODELSCOPE_ENDPOINT_VALUE" \
    QWEN3_ASR_MODEL_PATH="$QWEN_ASR_DIR" \
    QWEN3_ASR_REPO="$QWEN_ASR_REPO" \
    QWEN35_TRANSLATOR_MODEL_PATH="$QWEN35_TRANSLATOR_DIR" \
    QWEN35_TRANSLATOR_REPO="$QWEN35_TRANSLATOR_REPO" \
    HF_ENDPOINT="$HF_ENDPOINT_VALUE" \
    "$MOXIN_INIT"
else
  exec env \
    HEN_LOCAL_BOOTSTRAP_STATE_PATH="$STATE_PATH" \
    MOXIN_MODEL_PROVIDER="$MODEL_PROVIDER_VALUE" \
    MOXIN_MODELSCOPE_ENDPOINT="$MODELSCOPE_ENDPOINT_VALUE" \
    QWEN3_ASR_MODEL_PATH="$QWEN_ASR_DIR" \
    QWEN3_ASR_REPO="$QWEN_ASR_REPO" \
    QWEN35_TRANSLATOR_MODEL_PATH="$QWEN35_TRANSLATOR_DIR" \
    QWEN35_TRANSLATOR_REPO="$QWEN35_TRANSLATOR_REPO" \
    "$MOXIN_INIT"
fi
