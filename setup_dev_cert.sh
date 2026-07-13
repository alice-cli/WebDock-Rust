#!/bin/bash
# Create a dedicated keychain + "WebRust Dev" code-signing cert so rebuilds
# keep the same identity (macOS Screen Recording / Accessibility stick).
# Separate from WebDock so TCC rows do not collide.
set -euo pipefail

KC_PATH="$HOME/Library/Keychains/WebRust.keychain-db"
KC_PASS="webrust-sign"
SUPPORT="$HOME/Library/Application Support/WebRust"
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

mkdir -p "$SUPPORT"
echo "$KC_PASS" > "$SUPPORT/.sign-pass"
chmod 600 "$SUPPORT/.sign-pass"

# Recreate keychain cleanly
security delete-keychain "$KC_PATH" 2>/dev/null || true
rm -f "$KC_PATH"
security create-keychain -p "$KC_PASS" "$KC_PATH"
security set-keychain-settings -lut 21600 "$KC_PATH"
security unlock-keychain -p "$KC_PASS" "$KC_PATH"

LOGIN="$HOME/Library/Keychains/login.keychain-db"
if [[ -f "$LOGIN" ]]; then
  security list-keychains -d user -s "$KC_PATH" "$LOGIN"
else
  security list-keychains -d user -s "$KC_PATH"
fi

openssl genrsa -out "$TMP/key.pem" 2048 2>/dev/null
cat > "$TMP/ext.cnf" <<'EOF'
[req]
distinguished_name = dn
x509_extensions = v3
prompt = no
[dn]
CN = WebRust Dev
O = WebRust Local
[v3]
basicConstraints = critical,CA:FALSE
keyUsage = critical,digitalSignature
extendedKeyUsage = critical,codeSigning
subjectKeyIdentifier = hash
EOF
openssl req -new -x509 -key "$TMP/key.pem" -out "$TMP/cert.pem" -days 3650 \
  -config "$TMP/ext.cnf" -extensions v3
if openssl pkcs12 -export -help 2>&1 | grep -q -- '-legacy'; then
  openssl pkcs12 -export -legacy -out "$TMP/c.p12" -inkey "$TMP/key.pem" -in "$TMP/cert.pem" \
    -passout pass:webrust -name "WebRust Dev"
else
  openssl pkcs12 -export -out "$TMP/c.p12" -inkey "$TMP/key.pem" -in "$TMP/cert.pem" \
    -passout pass:webrust -name "WebRust Dev" \
    -certpbe PBE-SHA1-3DES -keypbe PBE-SHA1-3DES -macalg sha1
fi

security delete-identity -c "WebRust Dev" "$HOME/Library/Keychains/login.keychain-db" 2>/dev/null || true
security delete-certificate -c "WebRust Dev" "$HOME/Library/Keychains/login.keychain-db" 2>/dev/null || true

security import "$TMP/c.p12" -k "$KC_PATH" -P webrust -A -T /usr/bin/codesign -T /usr/bin/security
security set-key-partition-list -S apple-tool:,apple:,codesign: -s -k "$KC_PASS" "$KC_PATH" >/dev/null

cp "$TMP/cert.pem" "$SUPPORT/WebRustDev.cer"
HASH=$(openssl x509 -in "$TMP/cert.pem" -fingerprint -sha1 -noout | sed 's/^.*=//;s/://g')
echo "$HASH" > "$SUPPORT/.sign-hash"
chmod 600 "$SUPPORT/.sign-hash"
echo "identity hash: $HASH"

# Smoke-test sign
mkdir -p "$TMP/T.app/Contents/MacOS"
echo '#!/bin/bash' > "$TMP/T.app/Contents/MacOS/T"
chmod +x "$TMP/T.app/Contents/MacOS/T"
printf '%s\n' '<?xml version="1.0"?><plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>com.poc.webrust.setup</string>
<key>CFBundleExecutable</key><string>T</string>
<key>CFBundlePackageType</key><string>APPL</string>
</dict></plist>' > "$TMP/T.app/Contents/Info.plist"
security unlock-keychain -p "$KC_PASS" "$KC_PATH"
codesign --force --sign "WebRust Dev" --keychain "$KC_PATH" --timestamp=none "$TMP/T.app"
codesign -dv "$TMP/T.app" 2>&1 | grep -E 'Authority|flags|Signature' || true

echo ""
echo "OK — WebRust Dev keychain ready:"
echo "  $KC_PATH"
echo "Next: ./build_app.sh && ./install_home.sh"
echo "Then grant Screen Recording + Accessibility ONCE for WebRust."
echo "Rebuilds keep permissions (same cert + same install path)."
