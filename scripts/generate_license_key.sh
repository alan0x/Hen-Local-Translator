#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
KEY_DIR="${HEN_LOCAL_KEY_DIR:-$HOME/.hen-local/keys}"
PRIVATE_PEM="$KEY_DIR/license-v1-private.pem"
PRIVATE_BASE64="$KEY_DIR/license-v1-pkcs8.b64"
PUBLIC_BASE64="$ROOT_DIR/hen-local-translator-shell/license-public-key.b64"

mkdir -p "$KEY_DIR"
chmod 700 "$KEY_DIR"
if [[ -e "$PRIVATE_PEM" && "${1:-}" != "--force" ]]; then
  echo "Refusing to replace existing license key: $PRIVATE_PEM" >&2
  echo "Pass --force only when intentionally rotating every issued license." >&2
  exit 1
fi

umask 077
openssl genpkey -algorithm ED25519 -out "$PRIVATE_PEM"
openssl pkey -in "$PRIVATE_PEM" -outform DER | base64 | tr -d '\n' > "$PRIVATE_BASE64"
openssl pkey -in "$PRIVATE_PEM" -pubout -outform DER | tail -c 32 | base64 | tr -d '\n' > "$PUBLIC_BASE64"
printf '\n' >> "$PUBLIC_BASE64"

echo "Private deployment secret (never commit): $PRIVATE_BASE64"
echo "Desktop public key: $PUBLIC_BASE64"
