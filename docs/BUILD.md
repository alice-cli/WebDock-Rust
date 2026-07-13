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
  libwayland-dev libegl-dev \
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
.\target\release\WebRust.exe --cli --port 8090
```

Interactive desktop session required for capture.

## CI

GitHub Actions: `macos-14`, `windows-latest`, `ubuntu-24.04`.
