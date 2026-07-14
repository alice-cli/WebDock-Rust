#!/bin/bash
# Build WebRust.app and sign with a stable identity so TCC permissions survive rebuilds.
set -euo pipefail
cd "$(dirname "$0")"

APP="WebRust.app"
BIN_DIR="$APP/Contents/MacOS"
EXE="$BIN_DIR/WebRust"
RESOURCES="$APP/Contents/Resources"

KC_PATH="${WEBRUST_KEYCHAIN:-$HOME/Library/Keychains/WebRust.keychain-db}"
KC_PASS_FILE="${WEBRUST_KEYCHAIN_PASS_FILE:-$HOME/Library/Application Support/WebRust/.sign-pass}"
KC_PASS_DEFAULT="webrust-sign"
HASH_FILE="${WEBRUST_IDENTITY_HASH_FILE:-$HOME/Library/Application Support/WebRust/.sign-hash}"

echo "compiling WebRust (release)…"
cargo build -p webdock-server --release

# App icon — must ship with the repo (CI has no sibling WebDock tree).
ICON_SRC=""
for c in Assets/AppIcon.icns Assets/MacRemote.png MacRemote.png \
         ../WebDock/Assets/AppIcon.icns ../WebDock/MacRemote.png; do
  if [[ -f "$c" ]]; then ICON_SRC="$c"; break; fi
done
if [[ -z "$ICON_SRC" ]]; then
  echo "ERROR: no AppIcon (expected Assets/AppIcon.icns)" >&2
  exit 1
fi
echo "icon: $ICON_SRC"

rm -rf "${APP}"
mkdir -p "${BIN_DIR}" "${RESOURCES}"

# Binary name must match CFBundleExecutable (TCC / codesign identity).
cp "target/release/WebRust" "${EXE}"
chmod +x "${EXE}"

# Bundle WebUI next to binary for offline/embedded-free runs
if [[ -d webui ]]; then
  mkdir -p "${RESOURCES}/webui"
  cp -R webui/* "${RESOURCES}/webui/" 2>/dev/null || true
fi

if [[ "$ICON_SRC" == *.icns ]]; then
  cp "$ICON_SRC" "${RESOURCES}/AppIcon.icns"
else
  ICONSET=$(mktemp -d)/AppIcon.iconset
  mkdir -p "$ICONSET"
  for s in 16 32 128 256 512; do
    sips -z "$s" "$s" "$ICON_SRC" --out "${ICONSET}/icon_${s}x${s}.png" >/dev/null
    s2=$((s * 2))
    sips -z "$s2" "$s2" "$ICON_SRC" --out "${ICONSET}/icon_${s}x${s}@2x.png" >/dev/null
  done
  iconutil -c icns "$ICONSET" -o "${RESOURCES}/AppIcon.icns"
  rm -rf "$(dirname "$ICONSET")"
fi
test -f "${RESOURCES}/AppIcon.icns"
echo "AppIcon.icns $(wc -c < "${RESOURCES}/AppIcon.icns") bytes"

cat > "$APP/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key><string>WebRust</string>
    <key>CFBundleDisplayName</key><string>WebRust</string>
    <key>CFBundleIdentifier</key><string>com.poc.webrust</string>
    <key>CFBundleExecutable</key><string>WebRust</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>CFBundleIconFile</key><string>AppIcon</string>
    <key>CFBundleShortVersionString</key><string>0.1.0</string>
    <key>CFBundleVersion</key><string>1</string>
    <key>LSMinimumSystemVersion</key><string>14.0</string>
    <key>LSUIElement</key><false/>
    <key>LSBackgroundOnly</key><false/>
    <key>NSHighResolutionCapable</key><true/>
    <key>NSScreenCaptureUsageDescription</key>
    <string>WebRust captures windows and displays to stream them to your browser.</string>
    <key>NSAppleEventsUsageDescription</key>
    <string>WebRust raises and controls apps for remote desktop input.</string>
</dict>
</plist>
PLIST

# Wrapper so double-click launches the server with bundled webui and stays open via Console if needed.
# The actual binary is WebRust; we inject env via a thin launcher only if desired.
# Prefer running the rust binary directly as CFBundleExecutable.

resolve_pass() {
  if [[ -f "$KC_PASS_FILE" ]]; then
    cat "$KC_PASS_FILE"
  else
    echo "$KC_PASS_DEFAULT"
  fi
}

resolve_identity() {
  local hash=""
  if [[ -f "$HASH_FILE" ]]; then
    hash=$(tr -d ' \n' < "$HASH_FILE")
  fi
  if [[ -z "$hash" && -f "$KC_PATH" ]]; then
    hash=$(security find-certificate -c "WebRust Dev" -Z "$KC_PATH" 2>/dev/null \
      | awk '/SHA-1 hash:/{print $3; exit}')
  fi
  if [[ -z "$hash" && -f "$KC_PATH" ]]; then
    hash=$(security find-identity -v -p codesigning "$KC_PATH" 2>/dev/null \
      | awk '/WebRust Dev/{print $2; exit}')
  fi
  echo "$hash"
}

sign_stable() {
  if [[ ! -f "$KC_PATH" ]]; then
    echo "no keychain at $KC_PATH"
    return 1
  fi
  local pass id
  pass=$(resolve_pass)
  security unlock-keychain -p "$pass" "$KC_PATH" 2>/dev/null || {
    echo "unlock failed"
    return 1
  }
  security set-key-partition-list -S apple-tool:,apple:,codesign: -s -k "$pass" "$KC_PATH" >/dev/null 2>&1 || true

  id=$(resolve_identity)
  if [[ -z "$id" ]]; then
    id="WebRust Dev"
  else
    mkdir -p "$(dirname "$HASH_FILE")"
    echo "$id" > "$HASH_FILE"
    chmod 600 "$HASH_FILE" 2>/dev/null || true
  fi

  codesign --force --sign "$id" --keychain "$KC_PATH" --timestamp=none \
    --options runtime \
    --entitlements "$(dirname "$0")/WebRust.entitlements" \
    "$APP" 2>/dev/null \
  || codesign --force --sign "$id" --keychain "$KC_PATH" --timestamp=none "$APP"
}

if [[ "${SKIP_SIGN:-0}" == "1" ]]; then
  echo "signing skipped (SKIP_SIGN=1)"
else
  echo "signing…"
  if sign_stable; then
    if codesign -d -r- "$APP" 2>&1 | grep -q 'certificate root'; then
      echo "signed stable (certificate-based designated requirement) ✓"
    else
      echo "signed (check: codesign -dv $APP)"
    fi
  else
    echo "WARNING: stable keychain missing — run ./setup_dev_cert.sh"
    echo "  Falling back to ad-hoc (permissions may reset every build)."
    codesign --force --sign - "$APP"
  fi
  codesign -dv "$APP" 2>&1 | grep -E 'Authority|Signature|flags|Identifier' || true
fi
echo "built: $APP"
