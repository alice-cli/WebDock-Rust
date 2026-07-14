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

**Same flow as WebDock** (`WebDock/.github/workflows/release.yml`):

1. Import Developer ID Application (+ Installer) p12 into a temp keychain  
2. Write App Store Connect API key (`AuthKey_<id>.p8`)  
3. `SKIP_SIGN=1 ./build_app.sh` → inject version into Info.plist  
4. `codesign --options runtime --timestamp` (binary, then `.app`) — **no** `get-task-allow`  
5. `notarytool submit` + `stapler staple`  
6. Zip + optional `pkgbuild` / `productbuild` / `productsign` + notarize `.pkg`

Missing Application cert or API key **fails the job** (no silent ad-hoc release).

| Secret | Purpose |
|--------|---------|
| `APPLE_CERTIFICATE_BASE64` | Developer ID Application `.p12` (**required**) |
| `APPLE_CERTIFICATE_PASSWORD` | p12 password |
| `APPLE_INSTALLER_CERTIFICATE_BASE64` | Developer ID Installer `.p12` (for `.pkg`) |
| `APPLE_INSTALLER_CERTIFICATE_PASSWORD` | Installer p12 password |
| `APPLE_SIGN_IDENTITY` | e.g. `Developer ID Application: Name (TEAMID)` |
| `APPLE_INSTALLER_IDENTITY` | e.g. `Developer ID Installer: Name (TEAMID)` |
| `APPLE_API_KEY_P8` | App Store Connect API key body (`.p8` PEM text) |
| `APPLE_API_KEY_ID` | Key ID |
| `APPLE_API_ISSUER_ID` | Issuer UUID |
| `APPLE_TEAM_ID` | Team ID (optional metadata) |

### Artifacts

| File | Notes |
|------|--------|
| `WebRust-macOS-*.zip` | Notarized `WebRust.app` |
| `WebRust-macOS-*.pkg` | Installer → `/Applications` (needs Installer cert) |

### Local notarize (same as WebDock)

```bash
# uses ~/private/apple-certs/notary.env
./release_notarize.sh
```


## Windows / Linux

No Apple codesign. Release workflow ships zip/tar.gz of the `WebRust` binary + `webui/`.
