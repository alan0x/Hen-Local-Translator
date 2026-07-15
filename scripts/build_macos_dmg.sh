#!/usr/bin/env bash
set -euo pipefail

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "This script only supports macOS."
  exit 1
fi

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_NAME="Hen Local Translator"
APP_PATH="$ROOT_DIR/dist/${APP_NAME}.app"
OUT_DIR="$ROOT_DIR/dist"
VERSION=""
VOL_NAME=""
DMG_NAME=""

read_workspace_version() {
  sed -n '/^\[workspace.package\]/,/^\[/{s/^version = "\(.*\)"/\1/p;}' "$ROOT_DIR/Cargo.toml" | head -n 1
}

read_app_version() {
  local plist="$APP_PATH/Contents/Info.plist"
  if [[ -f "$plist" ]] && command -v /usr/libexec/PlistBuddy >/dev/null 2>&1; then
    /usr/libexec/PlistBuddy -c "Print :CFBundleShortVersionString" "$plist" 2>/dev/null || true
  fi
}

usage() {
  cat <<EOF
Usage:
  $(basename "$0") [options]

Options:
  --app-path <path>   Path to .app bundle (default: $APP_PATH)
  --out-dir <dir>     Output directory (default: $OUT_DIR)
  --version <version> Version used for default DMG naming (default: app bundle version)
  --vol-name <name>   DMG volume name (default: "$APP_NAME <version> Installer")
  --dmg-name <name>   DMG file name (default: Hen-Local-Translator-v<version>.dmg)
  -h, --help          Show this help
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --app-path)
      APP_PATH="$2"
      shift 2
      ;;
    --out-dir)
      OUT_DIR="$2"
      shift 2
      ;;
    --version)
      VERSION="$2"
      shift 2
      ;;
    --vol-name)
      VOL_NAME="$2"
      shift 2
      ;;
    --dmg-name)
      DMG_NAME="$2"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown option: $1"
      usage
      exit 1
      ;;
  esac
done

if [[ ! -d "$APP_PATH" ]]; then
  echo "App bundle not found: $APP_PATH"
  echo "Build it first with scripts/build_macos_app.sh"
  exit 1
fi

if [[ -z "$VERSION" ]]; then
  VERSION="$(read_app_version)"
fi
if [[ -z "$VERSION" ]]; then
  VERSION="$(read_workspace_version)"
fi
if [[ -z "$VERSION" ]]; then
  echo "Failed to determine version from app bundle or Cargo.toml"
  exit 1
fi
if [[ -z "$VOL_NAME" ]]; then
  VOL_NAME="$APP_NAME $VERSION Installer"
fi
if [[ -z "$DMG_NAME" ]]; then
  DMG_NAME="Hen-Local-Translator-v${VERSION}.dmg"
fi

mkdir -p "$OUT_DIR"
DMG_PATH="$OUT_DIR/$DMG_NAME"
STAGING_DIR="$(mktemp -d)"

cp -R "$APP_PATH" "$STAGING_DIR/"
ln -s /Applications "$STAGING_DIR/Applications"

rm -f "$DMG_PATH"
hdiutil create \
  -volname "$VOL_NAME" \
  -srcfolder "$STAGING_DIR" \
  -ov \
  -format UDZO \
  "$DMG_PATH"

rm -rf "$STAGING_DIR"

echo "DMG created:"
echo "  $DMG_PATH"
