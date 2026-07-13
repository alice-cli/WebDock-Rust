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

## GitHub Release (Developer ID + notarization)

Same secrets pattern as WebDock’s `.github/workflows/release.yml`:

| Secret | Purpose |
|--------|---------|
| `APPLE_CERTIFICATE_BASE64` | Developer ID Application `.p12` |
| `APPLE_CERTIFICATE_PASSWORD` | p12 password |
| `APPLE_INSTALLER_CERTIFICATE_BASE64` | Developer ID Installer (optional, for `.pkg`) |
| `APPLE_INSTALLER_CERTIFICATE_PASSWORD` | Installer p12 password |
| `APPLE_SIGN_IDENTITY` | e.g. `Developer ID Application: Name (TEAMID)` |
| `APPLE_INSTALLER_IDENTITY` | e.g. `Developer ID Installer: Name (TEAMID)` |
| `APPLE_API_KEY_P8` | App Store Connect API key body |
| `APPLE_API_KEY_ID` | Key ID |
| `APPLE_API_ISSUER_ID` | Issuer UUID |

Local notarize helper: `./release_notarize.sh` (requires the same env/secrets as CI).

## Windows / Linux

No Apple codesign. Release workflow ships zip/tar.gz of the `WebRust` binary + `webui/`.
