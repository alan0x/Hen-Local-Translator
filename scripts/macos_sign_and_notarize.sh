#!/usr/bin/env bash
set -euo pipefail

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "This script only supports macOS." >&2
  exit 1
fi

APP_PATH=""
DMG_PATH=""

usage() {
  cat <<EOF
Usage:
  $(basename "$0") --app </path/to/Hen Local Translator.app>
  $(basename "$0") --dmg </path/to/Hen-Local-Translator-vX.Y.Z.dmg>

Required environment:
  APPLE_SIGNING_IDENTITY        Developer ID Application identity

Additionally required for --dmg:
  APPLE_ID                      Apple Developer account email
  APPLE_APP_SPECIFIC_PASSWORD  App-specific password for notarytool
  APPLE_TEAM_ID                 Apple Developer team identifier
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --app)
      APP_PATH="$2"
      shift 2
      ;;
    --dmg)
      DMG_PATH="$2"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown option: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

if [[ -n "$APP_PATH" && -n "$DMG_PATH" ]]; then
  echo "Sign the app and notarize the DMG in separate invocations." >&2
  exit 1
fi
if [[ -z "$APP_PATH" && -z "$DMG_PATH" ]]; then
  usage >&2
  exit 1
fi

require_environment() {
  local name
  for name in "$@"; do
    if [[ -z "${!name:-}" ]]; then
      echo "Required environment variable is missing: $name" >&2
      exit 1
    fi
  done
}

sign_app() {
  local app_path="$1"
  local code_file

  if [[ ! -d "$app_path" ]]; then
    echo "App bundle not found: $app_path" >&2
    exit 1
  fi

  while IFS= read -r -d '' code_file; do
    if file "$code_file" | grep -q 'Mach-O'; then
      codesign \
        --force \
        --options runtime \
        --timestamp \
        --sign "$APPLE_SIGNING_IDENTITY" \
        "$code_file"
    fi
  done < <(find "$app_path/Contents" -type f -print0)

  codesign \
    --force \
    --options runtime \
    --timestamp \
    --sign "$APPLE_SIGNING_IDENTITY" \
    "$app_path"

  codesign --verify --deep --strict --verbose=2 "$app_path"
  echo "Signed and verified app bundle: $app_path"
}

notarize_dmg() {
  local dmg_path="$1"
  local result_path submission_id status

  if [[ ! -f "$dmg_path" ]]; then
    echo "DMG not found: $dmg_path" >&2
    exit 1
  fi

  codesign \
    --force \
    --timestamp \
    --sign "$APPLE_SIGNING_IDENTITY" \
    "$dmg_path"
  codesign --verify --verbose=2 "$dmg_path"

  result_path="$(mktemp "${RUNNER_TEMP:-${TMPDIR:-/tmp}}/hen-notary-result.XXXXXX")"
  if ! xcrun notarytool submit "$dmg_path" \
    --apple-id "$APPLE_ID" \
    --password "$APPLE_APP_SPECIFIC_PASSWORD" \
    --team-id "$APPLE_TEAM_ID" \
    --wait \
    --timeout 45m \
    --output-format json \
    > "$result_path"; then
    echo "Apple notarization submission failed." >&2
    sed -n '1,200p' "$result_path" >&2
    exit 1
  fi

  status="$(plutil -extract status raw -o - "$result_path")"
  submission_id="$(plutil -extract id raw -o - "$result_path")"
  echo "Apple notarization submission $submission_id finished with status: $status"
  if [[ "$status" != "Accepted" ]]; then
    sed -n '1,200p' "$result_path" >&2
    exit 1
  fi

  xcrun stapler staple "$dmg_path"
  xcrun stapler validate "$dmg_path"
  spctl \
    --assess \
    --type open \
    --context context:primary-signature \
    --verbose=2 \
    "$dmg_path"
  echo "Signed, notarized, and stapled DMG: $dmg_path"
}

require_environment APPLE_SIGNING_IDENTITY
if [[ -n "$APP_PATH" ]]; then
  sign_app "$APP_PATH"
else
  require_environment APPLE_ID APPLE_APP_SPECIFIC_PASSWORD APPLE_TEAM_ID
  notarize_dmg "$DMG_PATH"
fi
