#!/usr/bin/env bash
# Preflight checks for Hen Local Translator (no conda/Python).
# Conda checks removed — bootstrap now uses the bundled hen-local-init Rust binary.
set -euo pipefail

MODE="${1:-}"
APP_RESOURCES="${HEN_LOCAL_APP_RESOURCES:-}"
APP_BIN_PATH=""

QWEN_ASR_MODEL_DIR="${QWEN3_ASR_MODEL_PATH:-$HOME/.OminiX/models/qwen3-asr-1.7b}"
QWEN35_TRANSLATOR_MODEL_DIR="${QWEN35_TRANSLATOR_MODEL_PATH:-$HOME/.OminiX/models/Qwen3.5-2B-MLX-4bit}"
QWEN_ASR_REPO="${QWEN3_ASR_REPO:-mlx-community/Qwen3-ASR-1.7B-8bit}"
QWEN35_TRANSLATOR_REPO="${QWEN35_TRANSLATOR_REPO:-mlx-community/Qwen3.5-2B-MLX-4bit}"
MODEL_COMPLETION_MARKER=".moxin-model-complete.json"
BOOTSTRAP_VERSION=1

if [[ -z "$APP_RESOURCES" ]]; then
  APP_RESOURCES="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
fi

TRANSLATION_DATAFLOW_PATH=""
if [[ -f "$APP_RESOURCES/dataflow/translation_qwen35.yml" ]]; then
  TRANSLATION_DATAFLOW_PATH="$APP_RESOURCES/dataflow/translation_qwen35.yml"
elif [[ -f "$APP_RESOURCES/apps/hen-local-translator/dataflow/translation_qwen35.yml" ]]; then
  TRANSLATION_DATAFLOW_PATH="$APP_RESOURCES/apps/hen-local-translator/dataflow/translation_qwen35.yml"
fi

if [[ -f "$APP_RESOURCES/../MacOS/hen-local-translator-bin" ]]; then
  APP_BIN_PATH="$APP_RESOURCES/../MacOS/hen-local-translator-bin"
else
  APP_BIN_PATH="$APP_RESOURCES/target/debug/hen-local-translator"
  if [[ ! -f "$APP_BIN_PATH" ]]; then
    APP_BIN_PATH="$APP_RESOURCES/target/release/hen-local-translator"
  fi
fi

errors=()
warnings=()

check_file() {
  local path="$1"
  local label="$2"
  if [[ ! -f "$path" ]]; then
    errors+=("$label missing: $path")
  fi
}

check_cmd() {
  local cmd="$1"
  local label="$2"
  if ! command -v "$cmd" >/dev/null 2>&1; then
    errors+=("$label command not found: $cmd")
  fi
}

qwen35_translation_model_ready() {
  local model_dir="$1"
  [[ -f "$model_dir/config.json" ]] &&
  [[ -f "$model_dir/tokenizer.json" ]] &&
  [[ -f "$model_dir/tokenizer_config.json" ]] &&
  ([[ -f "$model_dir/model.safetensors" ]] || [[ -f "$model_dir/model.safetensors.index.json" ]])
}

asr_model_ready() {
  local model_dir="$1"
  [[ -f "$model_dir/config.json" ]]
}

model_completion_marker_valid() {
  local model_dir="$1"
  local repo_id="$2"
  local marker="$model_dir/$MODEL_COMPLETION_MARKER"
  [[ -s "$marker" ]] && grep -q "\"repo_id\"[[:space:]]*:[[:space:]]*\"$repo_id\"" "$marker"
}

write_model_completion_marker() {
  local model_dir="$1"
  local repo_id="$2"
  mkdir -p "$model_dir"
  cat > "$model_dir/$MODEL_COMPLETION_MARKER" <<EOF
{
  "repo_id": "$repo_id",
  "bootstrap_version": $BOOTSTRAP_VERSION
}
EOF
}

ensure_model_complete() {
  local model_dir="$1"
  local repo_id="$2"
  local checker="$3"

  if "$checker" "$model_dir"; then
    if ! model_completion_marker_valid "$model_dir" "$repo_id"; then
      write_model_completion_marker "$model_dir" "$repo_id"
    fi
    return 0
  fi
  return 1
}

# Locate hen-local-init binary
hen_local_init_resolved=0
resolve_hen_local_init() {
  if [[ -x "$APP_RESOURCES/../MacOS/hen-local-init" ]]; then
    hen_local_init_resolved=1; return
  fi
  if [[ -x "$APP_RESOURCES/target/debug/hen-local-init" ]]; then
    hen_local_init_resolved=1; return
  fi
  if [[ -x "$APP_RESOURCES/target/release/hen-local-init" ]]; then
    hen_local_init_resolved=1; return
  fi
}

check_cmd dora "Dora CLI"
check_file "$APP_BIN_PATH" "App runtime binary"
check_file "$TRANSLATION_DATAFLOW_PATH" "Translation dataflow file"

# Check dora-qwen3-asr binary
if [[ ! -x "${APP_RESOURCES}/../MacOS/dora-qwen3-asr" ]] && \
   [[ ! -x "${APP_RESOURCES}/target/debug/dora-qwen3-asr" ]] && \
   [[ ! -x "${APP_RESOURCES}/target/release/dora-qwen3-asr" ]]; then
  warnings+=("dora-qwen3-asr binary not found — live translation ASR unavailable (run: cargo build -p dora-qwen3-asr)")
fi

# Check dora-qwen35-translator binary
if [[ ! -x "${APP_RESOURCES}/../MacOS/dora-qwen35-translator" ]] && \
   [[ ! -x "${APP_RESOURCES}/target/debug/dora-qwen35-translator" ]] && \
   [[ ! -x "${APP_RESOURCES}/target/release/dora-qwen35-translator" ]]; then
  warnings+=("dora-qwen35-translator binary not found — Qwen3.5 translation backend unavailable (run: cargo build -p dora-qwen35-translator)")
fi

if [[ -n "$TRANSLATION_DATAFLOW_PATH" && -f "$TRANSLATION_DATAFLOW_PATH" ]]; then
  if grep -q 'TRANSLATION_MERGE_ENABLED' "$TRANSLATION_DATAFLOW_PATH"; then
    errors+=("translation_qwen35.yml still references removed TRANSLATION_MERGE_ENABLED placeholder: $TRANSLATION_DATAFLOW_PATH")
  fi
  if ! grep -q 'path: __ASR_BIN_PATH__' "$TRANSLATION_DATAFLOW_PATH"; then
    errors+=("translation_qwen35.yml missing __ASR_BIN_PATH__ placeholder: $TRANSLATION_DATAFLOW_PATH")
  fi
  if ! grep -q 'path: __TRANSLATOR_BIN_PATH__' "$TRANSLATION_DATAFLOW_PATH"; then
    errors+=("translation_qwen35.yml missing __TRANSLATOR_BIN_PATH__ placeholder: $TRANSLATION_DATAFLOW_PATH")
  fi
  if ! grep -q 'question_ended: moxin-mic-input/question_ended' "$TRANSLATION_DATAFLOW_PATH"; then
    warnings+=("translation_qwen35.yml no longer wires question_ended into ASR: $TRANSLATION_DATAFLOW_PATH")
  fi
fi

# Check hen-local-init binary (required for first-run bootstrap)
resolve_hen_local_init
if [[ "$hen_local_init_resolved" != "1" ]]; then
  if [[ -f "$APP_RESOURCES/../MacOS/hen-local-translator-bin" ]]; then
    errors+=("hen-local-init binary missing from app bundle. Run build_macos_app.sh.")
  else
    warnings+=("hen-local-init not found in dev tree (run: cargo build -p hen-local-init --release)")
  fi
fi

# Check Qwen3 ASR model (required)
if ! ensure_model_complete "$QWEN_ASR_MODEL_DIR" "$QWEN_ASR_REPO" asr_model_ready; then
  errors+=("Qwen3-ASR model not found: $QWEN_ASR_MODEL_DIR — run hen-local-init or launch the app")
fi

# Check Qwen3.5 translator model (required)
if ! ensure_model_complete "$QWEN35_TRANSLATOR_MODEL_DIR" "$QWEN35_TRANSLATOR_REPO" qwen35_translation_model_ready; then
  errors+=("Qwen3.5 translator model incomplete: $QWEN35_TRANSLATOR_MODEL_DIR — run hen-local-init or launch the app")
fi

if [[ "$MODE" != "--quick" ]]; then
  echo "=== Hen Local Translator Preflight ==="
  echo "Resources:  $APP_RESOURCES"
  echo "Dataflow:   $TRANSLATION_DATAFLOW_PATH"
  echo "ASR model:  $QWEN_ASR_MODEL_DIR"
  echo "Qwen3.5 translator model: $QWEN35_TRANSLATOR_MODEL_DIR"
  echo ""
fi

if ((${#warnings[@]} > 0)) && [[ "$MODE" != "--quiet" ]]; then
  echo "Warnings:"
  for w in "${warnings[@]}"; do
    echo "  - $w"
  done
  echo ""
fi

if ((${#errors[@]} > 0)); then
  if [[ "$MODE" != "--quiet" ]]; then
    echo "Preflight failed:"
    for e in "${errors[@]}"; do
      echo "  - $e"
    done
  fi
  exit 1
fi

if [[ "$MODE" != "--quiet" ]]; then
  echo "Preflight passed."
fi
