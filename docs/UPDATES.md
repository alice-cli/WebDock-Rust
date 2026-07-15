# WebRust in-app updates

WebRust checks **GitHub Releases** (`alice-cli/WebDock-Rust`) and can
download + install without a separate CDN or Cloudflare.

## User flow

1. **Settings → Check for updates** (or tray *Check for Updates…*)
2. If a newer version exists → **Install update**
3. App downloads the platform asset, runs the installer path, then exits so files can be replaced.
4. macOS opens the signed **`.pkg`** (Installer UI). Windows runs **Setup.exe** silently. Linux extracts the **tar.gz** over the install directory.

**Open release page** remains as a manual fallback.

## CLI

```bash
WebRust --check-update      # print latest / download URL
WebRust --install-update    # download + install (may exit)
```

## Platform assets (priority)

| OS | Preferred asset |
|----|-----------------|
| macOS | `WebRust-macOS-*.pkg` (then `.zip`) |
| Windows | `WebRust-Setup-*.exe` |
| Linux | `WebRust-linux-*.tar.gz` |

## Notes

- No custom update server — only `api.github.com` + `browser_download_url`.
- Override repo: `WEBRUST_UPDATE_REPO=owner/name`
- macOS `/Applications` installs need the official Installer (admin). Zip in-place replace only works when the install path is writable.
- Windows Inno Setup uses `/VERYSILENT /CLOSEAPPLICATIONS`.
- TCC permissions on macOS stay valid when reinstalling to the **same path** with the **same Developer ID**.
