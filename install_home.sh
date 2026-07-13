#!/bin/bash
# Build with STABLE signature and install to ~/WebRust.app
# Always kill the old process first so you never run a stale binary.
set -euo pipefail
cd "$(dirname "$0")"

DEST="${WEBRUST_INSTALL_PATH:-$HOME/WebRust.app}"

echo "==> quit running WebRust"
pkill -x WebRust 2>/dev/null || true
# also stop ad-hoc cargo binaries we may have started
if [[ -f /tmp/webdock-run.pid ]]; then
  kill "$(cat /tmp/webdock-run.pid)" 2>/dev/null || true
  rm -f /tmp/webdock-run.pid
fi
sleep 0.4

echo "==> build + sign"
./build_app.sh

if codesign -dv WebRust.app 2>&1 | grep -q 'Signature=adhoc'; then
  echo "NOTE: ad-hoc signature (no local WebRust Dev cert)."
  echo "  Optional (recommended if you rebuild often):"
  echo "    ./setup_dev_cert.sh && ./install_home.sh"
  echo "  Apple Developer Program is NOT required."
elif ! codesign -d -r- WebRust.app 2>&1 | grep -q 'certificate root'; then
  echo "NOTE: signature has no certificate designated requirement."
  echo "  Optional: ./setup_dev_cert.sh && ./install_home.sh"
fi

echo "==> install $DEST"
# Same path keeps System Settings permission rows stable
rm -rf "$DEST"
ditto WebRust.app "$DEST"
chmod +x "$DEST/Contents/MacOS/WebRust"

echo "==> signature at install path"
codesign -dv "$DEST" 2>&1 | grep -E 'Authority|Signature|Identifier' || true
codesign -d -r- "$DEST" 2>&1 | grep designated || true

echo "==> launch"
# Point at bundled webui when present
open "$DEST"
echo "done."
echo ""
echo "Grant (once) in System Settings → Privacy & Security:"
echo "  • Screen Recording  → WebRust  (com.poc.webrust)"
echo "  • Accessibility     → WebRust"
echo "Do NOT confuse with WebDock (Swift). They are separate apps."
