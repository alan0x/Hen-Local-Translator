#!/usr/bin/env bash
# Initialize model files required for live translation development.
#
# Downloads:
#   - Qwen3-ASR-1.7B-8bit
#   - Qwen3.5-2B-MLX-4bit translator
#
# Usage:
#   bash scripts/init_qwen3_models.sh
#
# Environment overrides:
#   QWEN3_ASR_MODEL_PATH        - ASR model directory (default: ~/.OminiX/models/qwen3-asr-1.7b)
#   QWEN3_ASR_REPO              - HuggingFace repo for ASR model
#   QWEN35_TRANSLATOR_MODEL_PATH - Translator model directory (default: ~/.OminiX/models/Qwen3.5-2B-MLX-4bit)
#   QWEN35_TRANSLATOR_REPO      - HuggingFace repo for translator model
#   HF_HUB_ENABLE_HF_TRANSFER   - set to 1 to use faster hf_transfer (default: 1)
set -euo pipefail

QWEN_ASR_DIR="${QWEN3_ASR_MODEL_PATH:-$HOME/.OminiX/models/qwen3-asr-1.7b}"
QWEN_ASR_REPO="${QWEN3_ASR_REPO:-mlx-community/Qwen3-ASR-1.7B-8bit}"
QWEN35_TRANSLATOR_DIR="${QWEN35_TRANSLATOR_MODEL_PATH:-$HOME/.OminiX/models/Qwen3.5-2B-MLX-4bit}"
QWEN35_TRANSLATOR_REPO="${QWEN35_TRANSLATOR_REPO:-mlx-community/Qwen3.5-2B-MLX-4bit}"

echo "=== Hen Local Translator Model Initialization ==="
echo "ASR model dir:        $QWEN_ASR_DIR"
echo "ASR repo:             $QWEN_ASR_REPO"
echo "Translator model dir: $QWEN35_TRANSLATOR_DIR"
echo "Translator repo:      $QWEN35_TRANSLATOR_REPO"
echo ""

if ! command -v python3 >/dev/null 2>&1; then
  echo "ERROR: python3 not found. Install Python 3.8+ first."
  exit 1
fi

download_snapshot() {
  local repo="$1"
  local target="$2"
  local label="$3"

  if [[ -f "$target/config.json" ]]; then
    echo "$label already present at $target"
    return
  fi

  echo "Downloading $label to $target ..."
  mkdir -p "$target"
  HF_HUB_ENABLE_HF_TRANSFER="${HF_HUB_ENABLE_HF_TRANSFER:-1}" \
  python3 - "$repo" "$target" <<'PYEOF'
import sys

repo, target = sys.argv[1], sys.argv[2]
try:
    from huggingface_hub import snapshot_download
except ImportError:
    import subprocess
    subprocess.check_call([sys.executable, "-m", "pip", "install", "huggingface-hub"])
    from huggingface_hub import snapshot_download

snapshot_download(repo, local_dir=target, local_dir_use_symlinks=False, resume_download=True)
PYEOF
}

download_snapshot "$QWEN_ASR_REPO" "$QWEN_ASR_DIR" "Qwen3-ASR"
download_snapshot "$QWEN35_TRANSLATOR_REPO" "$QWEN35_TRANSLATOR_DIR" "Qwen3.5 translator"

echo ""
echo "Done."
echo "  ASR model:        $QWEN_ASR_DIR"
echo "  Translator model: $QWEN35_TRANSLATOR_DIR"
echo "You can now run: npm --prefix hen-local-translator-shell/ui run desktop:dev"
