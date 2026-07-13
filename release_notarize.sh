#!/bin/bash
# Optional: Developer ID sign + notarize WebRust.app (Apple Developer Program).
# Mirrors WebDock/release_notarize.sh pattern. For local CI-like releases.
#
# Required env:
#   APPLE_SIGN_IDENTITY   e.g. "Developer ID Application: Name (TEAMID)"
#   APPLE_API_KEY_ID
#   APPLE_API_ISSUER_ID
#   APPLE_API_KEY_P8_PATH path to AuthKey_XXX.p8
#
# Usage:
#   SKIP_SIGN=1 ./build_app.sh
#   ./release_notarize.sh
set -euo pipefail
cd "$(dirname "$0")"

APP="${1:-WebRust.app}"
if [[ ! -d "$APP" ]]; then
  echo "missing $APP — run ./build_app.sh first (SKIP_SIGN=1 recommended)"
  exit 1
fi

: "${APPLE_SIGN_IDENTITY:?set APPLE_SIGN_IDENTITY}"
: "${APPLE_API_KEY_ID:?set APPLE_API_KEY_ID}"
: "${APPLE_API_ISSUER_ID:?set APPLE_API_ISSUER_ID}"
: "${APPLE_API_KEY_P8_PATH:?set APPLE_API_KEY_P8_PATH}"

echo "codesign Developer ID…"
codesign --force --options runtime --timestamp \
  --sign "$APPLE_SIGN_IDENTITY" \
  --entitlements WebRust.entitlements \
  "$APP/Contents/MacOS/WebRust"
codesign --force --options runtime --timestamp \
  --sign "$APPLE_SIGN_IDENTITY" \
  --entitlements WebRust.entitlements \
  "$APP"
codesign --verify --deep --strict --verbose=2 "$APP"

ZIP="${TMPDIR:-/tmp}/WebRust-notarize-$$.zip"
rm -f "$ZIP"
ditto -c -k --keepParent "$APP" "$ZIP"
echo "notarytool submit…"
xcrun notarytool submit "$ZIP" \
  --key "$APPLE_API_KEY_P8_PATH" \
  --key-id "$APPLE_API_KEY_ID" \
  --issuer "$APPLE_API_ISSUER_ID" \
  --wait
xcrun stapler staple "$APP"
xcrun stapler validate "$APP"
rm -f "$ZIP"
echo "OK — notarized $APP"
