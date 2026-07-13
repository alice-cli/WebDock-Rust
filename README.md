# WebRust

Cross-platform remote desktop host — **Rust** reimplementation of [WebDock](https://github.com/alice-cli/WebDock).

Stream a window or full display to any browser; control with mouse, keyboard, and Hangul compose.

**Languages / 언어:** [English](README.md) · [한국어](docs/README.ko.md) · [日本語](docs/README.ja.md) · [中文](docs/README.zh.md) · [Deutsch](docs/README.de.md) · [Français](docs/README.fr.md)

The **web UI** supports EN / KO / JA / ZH / DE / FR (language menu in the header).

| Product | Stack | Bundle ID | Default port |
|---------|-------|-----------|--------------|
| **WebDock** | Swift | `com.poc.webdock` | 8080 |
| **WebRust** | Rust | `com.poc.webrust` | 8090 |

Separate app identity → Screen Recording / Accessibility do not collide with WebDock.

---

## Features

| Feature | Description |
|---------|-------------|
| Window / display streaming | `xcap` (macOS / Windows / Linux) |
| Remote input | Mouse, keyboard, scroll, client-side Hangul |
| Quality | Fast / Balanced / Live + JPEG / PNG / **H.264** |
| H.264 | **macOS:** VideoToolbox HW · **Win/Linux:** OpenH264 SW |
| Auth | Optional access token (+ Origin check for WebSocket) |
| LAN | Connect from other devices (use a **strong token**) |
| Desktop GUI | Tray + settings on **Windows / macOS / Linux** |
| Auto-update | GitHub Releases check (`--check-update` / Settings) |
| App icons | Platform icons (icns / Shell / FreeDesktop) |

**Security:** If LAN is enabled, set a strong token. Do not expose the port to the public internet unprotected.  
**H.264 over LAN:** browsers require **HTTPS or localhost** for WebCodecs — use JPG on plain `http://192.168.x.x`, or put a TLS reverse proxy in front.

---

## Requirements

### Host

| OS | Notes |
|----|--------|
| **macOS 14+** | Screen Recording + Accessibility; Rust (`rustup`) |
| **Windows 10/11** | Interactive session; Graphics Capture |
| **Linux** | Prefer **X11**; Wayland is limited; `xdotool` helps focus |

### Client

- Modern Chrome / Edge / Safari / Firefox  
- H.264: Chrome/Edge with WebCodecs (**localhost or HTTPS**)

---

## Install

### From Releases (when published)

1. Open [**Releases**](https://github.com/alice-cli/WebDock-Rust/releases)
2. Download for your OS:
   - **macOS:** `WebRust-macOS-*.zip` → open **WebRust.app**
   - **Windows:** `WebRust-Setup-*.exe` (installer, recommended) or portable zip
   - **Linux:** `WebRust-linux-*.tar.gz` → run `./WebRust` (tray) or `--cli`
3. Grant host permissions (macOS: Screen Recording + Accessibility)
4. Default GUI starts the tray + settings; remote UI at `http://127.0.0.1:8090`
5. Updates: Settings → **Check for updates**, or `WebRust --check-update`

### From source (macOS app — recommended)

```bash
git clone https://github.com/alice-cli/WebDock-Rust.git
cd WebDock-Rust
chmod +x setup_dev_cert.sh build_app.sh install_home.sh

# Once: local self-signed cert (no Apple Developer account) — same pattern as WebDock
./setup_dev_cert.sh

# Build → sign → install ~/WebRust.app → launch
./install_home.sh
```

Then **System Settings → Privacy & Security**:

- **Screen Recording** → **WebRust** (`com.poc.webrust`)
- **Accessibility** → **WebRust**

Rebuilds to the same path keep TCC rows when signed with **WebRust Dev**.

### CLI (all platforms)

```bash
cargo build -p webdock-server --release
./target/release/WebRust --cli --port 8090 --lan --gen-token
```

Windows: `target\release\WebRust.exe --cli --port 8090`

---

## Build matrix

| Target | Command | Artifact |
|--------|---------|----------|
| macOS app | `./build_app.sh` | `WebRust.app` |
| Install home | `./install_home.sh` | `~/WebRust.app` |
| Server binary | `cargo build -p webdock-server --release` | `WebRust` / `WebRust.exe` |
| CI | GitHub Actions | macOS · Windows · Ubuntu |

### macOS signing (local — same idea as WebDock)

| Script | Role |
|--------|------|
| [`setup_dev_cert.sh`](./setup_dev_cert.sh) | Keychain `WebRust.keychain-db` + **WebRust Dev** cert |
| [`build_app.sh`](./build_app.sh) | Release build + codesign (stable identity / hash) |
| [`install_home.sh`](./install_home.sh) | Install `~/WebRust.app` |
| [`release_notarize.sh`](./release_notarize.sh) | Optional Developer ID + notarytool (Apple Program) |

Env overrides (optional): `WEBRUST_KEYCHAIN`, `WEBRUST_KEYCHAIN_PASS_FILE`, `WEBRUST_IDENTITY_HASH_FILE`.

---

## Config

`~/Library/Application Support/WebRust/config.json` (macOS)  
`%AppData%\WebRust\config.json` (Windows) · `~/.config/webrust/config.json` (Linux)

```json
{
  "serverEnabled": true,
  "port": 8090,
  "allowLan": false,
  "token": "your-secret-token",
  "trustProxyHeaders": false
}
```

---

## Tips

- **UI language:** header language selector (`localStorage`)
- **Hangul:** **한 / A** or Ctrl+Space on the remote canvas
- **Quality:** Fast / Balanced / Live · JPG / PNG / H.264
- **H.264 freeze on LAN:** open via `https://…` or use JPG; see status message `h264NeedsHttps`

---

## Project layout

```text
WebDock-Rust/
├── crates/
│   ├── webdock-protocol   # WS types + H.264 framing
│   ├── webdock-core       # config, input seat, tuning
│   ├── webdock-platform   # capture / input / windows (native)
│   ├── webdock-encoder    # JPEG/PNG + H.264 (VT / OpenH264)
│   └── webdock-server     # axum host + macOS tray/settings
├── webui/                 # browser client (i18n EN…FR)
├── docs/                  # multi-language README + release notes
├── .github/workflows/     # CI + Release
├── setup_dev_cert.sh      # macOS local codesign identity
├── build_app.sh
└── install_home.sh
```

Architecture notes: [PLAN.md](./PLAN.md) · signing: [docs/SIGNING.md](./docs/SIGNING.md)

---

## Troubleshooting

| Issue | Check |
|-------|--------|
| Black screen / empty list | Screen Recording; wake display |
| Clicks / keys ignored | Accessibility (macOS) |
| Cannot connect | Server on, port, LAN, firewall, token |
| H.264 frozen on phone / LAN | Use **localhost/HTTPS**, or JPG |
| Permissions every rebuild | `./setup_dev_cert.sh` then reinstall to `~/WebRust.app` |

---

## License

[MIT](LICENSE)

Use responsibly. You are responsible for tokens, firewall, and network exposure.
