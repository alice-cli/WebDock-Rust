# WebRust

Rust-Host für Fernsteuerung von Fenstern im Browser.  
Cross-Platform-Neuimplementierung von [WebDock](https://github.com/alice-cli/WebDock) (Swift).

**Sprachen:** [English](../README.md) · [한국어](README.ko.md) · [日本語](README.ja.md) · [中文](README.zh.md) · [Deutsch](README.de.md) · [Français](README.fr.md)

Web-UI: EN / KO / JA / ZH / DE / FR.

| Produkt | Stack | Bundle-ID | Port |
|---------|-------|-----------|------|
| WebDock | Swift | `com.poc.webdock` | 8080 |
| **WebRust** | Rust | `com.poc.webrust` | 8090 |

---

## Funktionen

- Fenster-/Display-Streaming (xcap)
- Maus, Tastatur, Scroll, Hangul
- JPEG / PNG / H.264 (macOS: VideoToolbox)
- Optionaler Token, LAN

**Sicherheit:** Bei LAN starken Token setzen.  
**H.264 im LAN:** WebCodecs braucht HTTPS oder localhost.

---

## Installation

```bash
git clone https://github.com/alice-cli/WebDock-Rust.git
cd WebDock-Rust
./setup_dev_cert.sh   # macOS: lokale Codesign-Identität wie bei WebDock
./install_home.sh
```

Berechtigungen: **Bildschirmaufnahme** + **Bedienungshilfen** → WebRust.

---

## Lizenz

[MIT](../LICENSE)
