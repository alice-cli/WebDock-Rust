# Building WebRust

## All platforms

```bash
cargo build -p webdock-server --release
# binary: target/release/WebRust  (Windows: WebRust.exe)
```

## Linux dependencies (Debian/Ubuntu)

**Ubuntu 24.04+ recommended.** Ubuntu 22.04’s PipeWire/SPA headers break `libspa` 0.9 (used by `xcap` 0.9).

```bash
sudo apt-get update
sudo apt-get install -y \
  pkg-config libssl-dev libclang-dev clang \
  libpipewire-0.3-dev libspa-0.2-dev \
  libxcb1-dev libxrandr-dev libdbus-1-dev \
  libwayland-dev libegl-dev libgbm-dev \
  libwebkit2gtk-4.1-dev   # only if building tray/WebView extras
```

## macOS

```bash
./setup_dev_cert.sh   # once
./build_app.sh        # → WebRust.app
./install_home.sh     # → ~/WebRust.app
```

See [SIGNING.md](./SIGNING.md).

## Windows

```powershell
cargo build -p webdock-server --release
# GUI (tray + settings):
.\target\release\WebRust.exe
# Headless:
.\target\release\WebRust.exe --cli --port 8090
```

### Installer (Inno Setup)

```powershell
# Install Inno Setup 6, then:
.\packaging\windows\build_installer.ps1 -Version 0.1.1
# → dist\WebRust-Setup-0.1.1.exe  +  dist\WebRust-windows-0.1.1.zip
```

Interactive desktop session required for capture. GUI needs **WebView2** (preinstalled on modern Windows 10/11).

## Auto-update

`WebRust --check-update` queries GitHub Releases for `alice-cli/WebDock-Rust`.  
Override repo with `WEBRUST_UPDATE_REPO=owner/name`.

## CI

GitHub Actions: `macos-14`, `windows-latest`, `ubuntu-24.04`  
Release workflow publishes mac zip, Windows **Setup.exe** + zip, Linux tar.gz.
