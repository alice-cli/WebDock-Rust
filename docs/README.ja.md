# WebRust

ブラウザから PC/Mac のウィンドウを遠隔操作する **Rust** ホスト。  
Swift [WebDock](https://github.com/alice-cli/WebDock) のクロスプラットフォーム再実装です。

**言語:** [English](../README.md) · [한국어](README.ko.md) · [日本語](README.ja.md) · [中文](README.zh.md) · [Deutsch](README.de.md) · [Français](README.fr.md)

Web UI: EN / KO / JA / ZH / DE / FR。

| 製品 | スタック | Bundle ID | 既定ポート |
|------|----------|-----------|------------|
| WebDock | Swift | `com.poc.webdock` | 8080 |
| **WebRust** | Rust | `com.poc.webrust` | 8090 |

---

## 機能

- ウィンドウ / 全画面ストリーミング（xcap）
- マウス・キーボード・スクロール・ハングル入力
- JPEG / PNG / H.264（macOS は VideoToolbox）
- 任意トークン認証、LAN 接続

**セキュリティ:** LAN 利用時は強いトークンを。  
**H.264 と LAN:** WebCodecs は HTTPS または localhost が必要です。平文 LAN では JPG を使うか TLS プロキシを置いてください。

---

## インストール

```bash
git clone https://github.com/alice-cli/WebDock-Rust.git
cd WebDock-Rust
./setup_dev_cert.sh   # macOS: ローカル署名（WebDock と同じ方式）
./install_home.sh     # → ~/WebRust.app
```

権限: **画面収録** · **アクセシビリティ** → WebRust。

CLI: `cargo build -p webdock-server --release && ./target/release/WebRust --cli`

---

## 署名 (macOS)

`setup_dev_cert.sh` → `WebRust Dev` 証明書 · `build_app.sh` で codesign。詳細は [SIGNING.md](./SIGNING.md)。

---

## ライセンス

[MIT](../LICENSE)
