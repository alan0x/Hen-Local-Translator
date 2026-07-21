#!/usr/bin/env bash
set -euo pipefail

APP_PATH=""
ARCHIVE_PATH=""
MANIFEST_PATH=""
VERSION=""

usage() {
  echo "Usage: $(basename "$0") --app <app> --archive <app.tar.gz> --manifest <latest.json> --version <semver>"
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --app) APP_PATH="$2"; shift 2 ;;
    --archive) ARCHIVE_PATH="$2"; shift 2 ;;
    --manifest) MANIFEST_PATH="$2"; shift 2 ;;
    --version) VERSION="$2"; shift 2 ;;
    -h|--help) usage; exit 0 ;;
    *) echo "Unknown option: $1" >&2; usage >&2; exit 1 ;;
  esac
done

for value in "$APP_PATH" "$ARCHIVE_PATH" "$MANIFEST_PATH" "$VERSION"; do
  [[ -n "$value" ]] || { usage >&2; exit 1; }
done

[[ -d "$APP_PATH" ]] || { echo "App missing: $APP_PATH" >&2; exit 1; }
[[ -s "$ARCHIVE_PATH" ]] || { echo "Updater archive missing: $ARCHIVE_PATH" >&2; exit 1; }
[[ -s "$ARCHIVE_PATH.sig" ]] || { echo "Updater signature missing: $ARCHIVE_PATH.sig" >&2; exit 1; }
[[ -s "$MANIFEST_PATH" ]] || { echo "Updater manifest missing: $MANIFEST_PATH" >&2; exit 1; }

for executable in hen-local-translator dora dora-qwen3-asr dora-qwen35-translator qwen-tts-node hen-local-init mlx.metallib; do
  test -f "$APP_PATH/Contents/MacOS/$executable" || { echo "App is missing $executable" >&2; exit 1; }
  tar -tzf "$ARCHIVE_PATH" | grep -q "Contents/MacOS/$executable$" || { echo "Updater is missing $executable" >&2; exit 1; }
done

node - "$MANIFEST_PATH" "$ARCHIVE_PATH.sig" "$VERSION" <<'NODE'
const fs = require('node:fs');
const [manifestPath, signaturePath, version] = process.argv.slice(2);
const manifest = JSON.parse(fs.readFileSync(manifestPath, 'utf8'));
const platform = manifest.platforms?.['darwin-aarch64'];
if (manifest.version !== version) throw new Error(`Manifest version mismatch: ${manifest.version}`);
if (!platform?.url?.startsWith('https://github.com/Hen-Local/Hen-Local-Translator/releases/download/')) throw new Error('Unexpected updater URL');
if (platform.signature !== fs.readFileSync(signaturePath, 'utf8').trim()) throw new Error('Manifest signature does not match .sig file');
NODE

codesign --verify --deep --strict "$APP_PATH"
echo "Tauri updater smoke test passed for $VERSION"
