# macOS code signing (WebRust)

WebRust follows the **same local-signing model as WebDock**, with a **separate** keychain and identity so TCC (Screen Recording / Accessibility) does not collide.

## Local development (no Apple Developer Program)

| WebDock | WebRust |
|---------|---------|
| `WebDock.keychain-db` | `WebRust.keychain-db` |
| CN **WebDock Dev** | CN **WebRust Dev** |
| `~/Library/Application Support/WebDock/` | `~/Library/Application Support/WebRust/` |
| `com.poc.webdock` | `com.poc.webrust` |
| `~/WebDock.app` | `~/WebRust.app` |

```bash
./setup_dev_cert.sh   # once per machine
./build_app.sh        # signs with hash or "WebRust Dev"
./install_home.sh     # ditto → ~/WebRust.app + open
```

- Pass file: `~/Library/Application Support/WebRust/.sign-pass`
- Identity hash: `~/Library/Application Support/WebRust/.sign-hash`
- Entitlements: `WebRust.entitlements` (hardened runtime when possible)

Grant **Screen Recording** and **Accessibility** once for **WebRust**. Rebuilds to the same path keep permissions when the cert is stable.

### Skip sign

```bash
SKIP_SIGN=1 ./build_app.sh   # ad-hoc; TCC may reset every rebuild
```

## GitHub Release (Developer ID + notarization + `.pkg`)

Same secrets pattern as WebDock’s `.github/workflows/release.yml`.
Without `APPLE_CERTIFICATE_BASE64`, the release job **skips Developer ID** and
ships an ad-hoc zip only (Gatekeeper will block downloads).

| Secret | Purpose |
|--------|---------|
| `APPLE_CERTIFICATE_BASE64` | Developer ID Application `.p12` (**required** for signed zip) |
| `APPLE_CERTIFICATE_PASSWORD` | p12 password |
| `APPLE_INSTALLER_CERTIFICATE_BASE64` | Developer ID Installer `.p12` (**required** for `.pkg`) |
| `APPLE_INSTALLER_CERTIFICATE_PASSWORD` | Installer p12 password |
| `APPLE_SIGN_IDENTITY` | e.g. `Developer ID Application: Name (TEAMID)` (optional if unique) |
| `APPLE_INSTALLER_IDENTITY` | e.g. `Developer ID Installer: Name (TEAMID)` (optional if unique) |
| `APPLE_API_KEY_P8` | App Store Connect API key body (`.p8` PEM text) |
| `APPLE_API_KEY_ID` | Key ID |
| `APPLE_API_ISSUER_ID` | Issuer UUID |

### What the release job produces

| Artifact | Needs |
|----------|--------|
| `WebRust-macOS-*.zip` | Always (Developer ID + notarized when Application cert + API key set) |
| `WebRust-macOS-*.pkg` | Application + **Installer** cert; notarized when API key set |

### Copy secrets from WebDock

If WebDock already signs releases, copy the same repo secrets into
`alice-cli/WebDock-Rust` (Settings → Secrets and variables → Actions):

```bash
# names only — values must be set in the GitHub UI or via `gh secret set`
for s in \
  APPLE_CERTIFICATE_BASE64 APPLE_CERTIFICATE_PASSWORD \
  APPLE_INSTALLER_CERTIFICATE_BASE64 APPLE_INSTALLER_CERTIFICATE_PASSWORD \
  APPLE_SIGN_IDENTITY APPLE_INSTALLER_IDENTITY \
  APPLE_API_KEY_P8 APPLE_API_KEY_ID APPLE_API_ISSUER_ID
do
  echo "$s"
done
```

Local notarize helper: `./release_notarize.sh` (requires the same env/secrets as CI).

## Windows / Linux

No Apple codesign. Release workflow ships zip/tar.gz of the `WebRust` binary + `webui/`.
