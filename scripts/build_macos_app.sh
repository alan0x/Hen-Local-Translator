#!/usr/bin/env bash
set -euo pipefail

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "This script only supports macOS."
  exit 1
fi

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORKSPACE_VERSION="$(sed -n '/^\[workspace.package\]/,/^\[/{s/^version = "\(.*\)"/\1/p;}' "$ROOT_DIR/Cargo.toml" | head -n 1)"
if [[ -z "$WORKSPACE_VERSION" ]]; then
  echo "Failed to read workspace package version from $ROOT_DIR/Cargo.toml"
  exit 1
fi

APP_NAME="Hen Local Translator"
BUNDLE_ID="com.henlocal.translator"
PROFILE="release"
OUT_DIR="$ROOT_DIR/dist"
VERSION="$WORKSPACE_VERSION"
MARKETING_VERSION="${VERSION%%[-+]*}"
BUILD_VERSION="${HEN_LOCAL_BUILD_VERSION:-$MARKETING_VERSION}"
BUILD_TARGET_DIR="${HEN_LOCAL_CARGO_TARGET_DIR:-${TMPDIR:-/tmp}/hen-local-translator-cargo-target}"
TAURI_PRODUCT_NAME="Hen Local Translator"

ensure_translator_is_not_running() {
  if pgrep -x hen-local-translator >/dev/null 2>&1; then
    echo "Hen Local Translator is currently running." >&2
    echo "Quit every Hen Local Translator window before rebuilding the app bundle." >&2
    echo "Replacing a live macOS bundle can leave WKWebView attached to stale files and show a blank window." >&2
    exit 1
  fi
}

usage() {
  cat <<EOF
Usage:
  $(basename "$0") [options]

Options:
  --app-name <name>      App name shown in Finder (default: "$APP_NAME")
  --bundle-id <id>       CFBundleIdentifier (default: "$BUNDLE_ID")
  --profile <profile>    Cargo profile: release or dev (default: "$PROFILE")
  --out-dir <dir>        Output directory for .app (default: "$OUT_DIR")
  --version <version>    App version; must match Cargo.toml (default: "$VERSION")
  -h, --help             Show this help
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --app-name) APP_NAME="$2"; shift 2 ;;
    --bundle-id) BUNDLE_ID="$2"; shift 2 ;;
    --profile) PROFILE="$2"; shift 2 ;;
    --out-dir) OUT_DIR="$2"; shift 2 ;;
    --version) VERSION="$2"; shift 2 ;;
    -h|--help) usage; exit 0 ;;
    *) echo "Unknown option: $1"; usage; exit 1 ;;
  esac
done

if [[ "$VERSION" != "$WORKSPACE_VERSION" ]]; then
  echo "App version $VERSION does not match the workspace version $WORKSPACE_VERSION"
  echo "Set the version with: node scripts/version.mjs set <version>"
  exit 1
fi
if [[ "$PROFILE" != "release" && "$PROFILE" != "dev" ]]; then
  echo "Unsupported profile: $PROFILE (expected release or dev)"
  exit 1
fi
PROFILE_DIR="$PROFILE"
if [[ "$PROFILE" == "dev" ]]; then
  PROFILE_DIR="debug"
fi
if [[ "$BUILD_TARGET_DIR" == *" "* ]]; then
  echo "Cargo target directory cannot contain spaces: $BUILD_TARGET_DIR"
  exit 1
fi
if [[ ! "$MARKETING_VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "macOS marketing version must contain three numeric components: $MARKETING_VERSION"
  exit 1
fi
if [[ ! "$BUILD_VERSION" =~ ^[0-9]+(\.[0-9]+){0,2}$ ]]; then
  echo "macOS build version must contain one to three numeric components: $BUILD_VERSION"
  exit 1
fi

# Never mutate an application bundle while macOS still has its executable and
# WebContent processes open. This check is repeated immediately before the
# replacement because the build itself can take several minutes.
ensure_translator_is_not_running

mkdir -p "$BUILD_TARGET_DIR" "$OUT_DIR"
export CARGO_TARGET_DIR="$BUILD_TARGET_DIR"
export MOXIN_DORA_TARGET_DIR="$BUILD_TARGET_DIR"

resolve_mlx_prebuilt_path() {
  local build_dir="$BUILD_TARGET_DIR/$PROFILE_DIR/build"
  if [[ -d "$build_dir" ]]; then
    find "$build_dir" -type d -path '*mlx-sys-*/out/mlx-prebuilt' 2>/dev/null | tail -n 1 || true
  fi
}

run_cargo_build() {
  local mlx_prebuilt_path=""
  mlx_prebuilt_path="$(resolve_mlx_prebuilt_path)"
  if [[ -n "$mlx_prebuilt_path" ]]; then
    MLX_PREBUILT_PATH="$mlx_prebuilt_path" cargo build --locked "$@"
  else
    cargo build --locked "$@"
  fi
}

CARGO_PROFILE_ARGS=(--release)
if [[ "$PROFILE" == "dev" ]]; then
  CARGO_PROFILE_ARGS=(--profile dev)
fi

echo "Building required local translation executables..."
run_cargo_build --manifest-path "$ROOT_DIR/Cargo.toml" "${CARGO_PROFILE_ARGS[@]}" \
  -p dora-qwen3-asr \
  -p dora-qwen35-translator \
  -p hen-local-init

if ! command -v dora >/dev/null 2>&1; then
  echo "dora CLI not found in PATH. Install dora-cli before packaging."
  exit 1
fi

TARGET_TRIPLE="aarch64-apple-darwin"
SIDECAR_DIR="$ROOT_DIR/hen-local-translator-shell/.tauri-sidecars"
rm -rf "$SIDECAR_DIR"
mkdir -p "$SIDECAR_DIR"
stage_sidecar() {
  local source="$1"
  local name="$2"
  if [[ ! -f "$source" ]]; then
    echo "Required sidecar not found: $source"
    exit 1
  fi
  cp "$source" "$SIDECAR_DIR/${name}-${TARGET_TRIPLE}"
  chmod +x "$SIDECAR_DIR/${name}-${TARGET_TRIPLE}"
}
stage_sidecar "$(command -v dora)" "dora"
stage_sidecar "$BUILD_TARGET_DIR/$PROFILE_DIR/dora-qwen3-asr" "dora-qwen3-asr"
stage_sidecar "$BUILD_TARGET_DIR/$PROFILE_DIR/dora-qwen35-translator" "dora-qwen35-translator"
stage_sidecar "$BUILD_TARGET_DIR/$PROFILE_DIR/hen-local-init" "hen-local-init"
stage_sidecar "$BUILD_TARGET_DIR/$PROFILE_DIR/mlx.metallib" "mlx.metallib"

if [[ -z "${TAURI_SIGNING_PRIVATE_KEY:-}" ]]; then
  LOCAL_UPDATER_KEY="$HOME/.tauri/hen-local-translator.key"
  if [[ ! -f "$LOCAL_UPDATER_KEY" ]]; then
    echo "Tauri updater signing key not found: $LOCAL_UPDATER_KEY"
    exit 1
  fi
  export TAURI_SIGNING_PRIVATE_KEY="$(cat "$LOCAL_UPDATER_KEY")"
fi
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD="${TAURI_SIGNING_PRIVATE_KEY_PASSWORD:-}"

# Use Tauri's official bundler for the application shell. The previous script
# assembled Info.plist and the launcher by hand, which produced WebViews that
# could open as empty white windows in the distributed DMG.
echo "Building the Tauri application bundle..."
(
  cd "$ROOT_DIR/hen-local-translator-shell"
  if [[ "$PROFILE" == "dev" ]]; then
    ./ui/node_modules/.bin/tauri build --bundles app --debug --config tauri.bundle.conf.json
  else
    ./ui/node_modules/.bin/tauri build --bundles app --config tauri.bundle.conf.json
  fi
)

TAURI_APP="$BUILD_TARGET_DIR/$PROFILE_DIR/bundle/macos/$TAURI_PRODUCT_NAME.app"
if [[ ! -d "$TAURI_APP" ]]; then
  echo "Tauri app bundle not found: $TAURI_APP"
  exit 1
fi

APP_DIR="$OUT_DIR/$APP_NAME.app"
ensure_translator_is_not_running
rm -rf "$APP_DIR"
cp -R "$TAURI_APP" "$APP_DIR"

PLIST_PATH="$APP_DIR/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Set :CFBundleShortVersionString $MARKETING_VERSION" "$PLIST_PATH"
/usr/libexec/PlistBuddy -c "Set :CFBundleVersion $BUILD_VERSION" "$PLIST_PATH"
codesign --force --deep --sign - "$APP_DIR"

UPDATER_ARCHIVE="$OUT_DIR/Hen-Local-Translator-v${VERSION}.app.tar.gz"
rm -f "$UPDATER_ARCHIVE" "$UPDATER_ARCHIVE.sig"
COPYFILE_DISABLE=1 tar -czf "$UPDATER_ARCHIVE" -C "$OUT_DIR" "$APP_NAME.app"
(
  cd "$ROOT_DIR"
  ./hen-local-translator-shell/ui/node_modules/.bin/tauri signer sign "$UPDATER_ARCHIVE"
)
test -s "$UPDATER_ARCHIVE.sig"
rm -rf "$SIDECAR_DIR"

echo "App bundle created with the official Tauri shell:"
echo "  $APP_DIR"
echo "Models are intentionally not bundled. Core models download after setup;"
echo "spoken translation uses built-in Apple voices and needs no model download."
