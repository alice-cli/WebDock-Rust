# WebRust release notes template

## WebRust v{{VERSION}}

Cross-platform remote desktop host (Rust). Compatible web UI languages: **EN · KO · JA · ZH · DE · FR**.

### Downloads

| File | Platform |
|------|----------|
| `WebRust-macOS-{{VERSION}}.zip` | macOS 14+ app |
| `WebRust-macOS-{{VERSION}}.pkg` | macOS installer (if notarized) |
| `WebRust-windows-{{VERSION}}.zip` | Windows x64 CLI |
| `WebRust-linux-{{VERSION}}.tar.gz` | Linux x64 CLI |

### Install

**macOS:** open zip → `WebRust.app`, or run `.pkg` → Applications.  
Enable **Screen Recording** + **Accessibility**. Default port **8090**.

**Windows / Linux:** extract binary, run:

```bash
./WebRust --cli --port 8090 --gen-token
```

### Docs

[EN](../README.md) · [KO](README.ko.md) · [JA](README.ja.md) · [ZH](README.zh.md) · [DE](README.de.md) · [FR](README.fr.md) · [Signing](SIGNING.md)

### Notes

- H.264 on **localhost/HTTPS** only (WebCodecs). Use JPG on plain LAN HTTP.
- Set a **token** when enabling LAN.
