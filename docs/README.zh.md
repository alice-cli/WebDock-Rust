# WebRust

用浏览器远程控制电脑窗口的 **Rust** 主机端。  
Swift [WebDock](https://github.com/alice-cli/WebDock) 的跨平台重写。

**语言:** [English](../README.md) · [한국어](README.ko.md) · [日本語](README.ja.md) · [中文](README.zh.md) · [Deutsch](README.de.md) · [Français](README.fr.md)

Web UI：EN / KO / JA / ZH / DE / FR。

| 产品 | 技术栈 | Bundle ID | 默认端口 |
|------|--------|-----------|----------|
| WebDock | Swift | `com.poc.webdock` | 8080 |
| **WebRust** | Rust | `com.poc.webrust` | 8090 |

---

## 功能

- 窗口 / 全屏串流（xcap）
- 鼠标、键盘、滚动、韩文输入
- JPEG / PNG / H.264（macOS 使用 VideoToolbox）
- 可选访问令牌、局域网访问

**安全：** 开启局域网时请设置强令牌。  
**H.264 与局域网：** WebCodecs 需要 HTTPS 或 localhost；明文 `http://192.168…` 请用 JPG 或 TLS 反向代理。

---

## 安装

```bash
git clone https://github.com/alice-cli/WebDock-Rust.git
cd WebDock-Rust
./setup_dev_cert.sh   # macOS 本地签名（与 WebDock 相同模式）
./install_home.sh
```

权限：**屏幕录制** · **辅助功能** → WebRust。

CLI：`cargo build -p webdock-server --release`

---

## 签名 (macOS)

见 [SIGNING.md](./SIGNING.md)。

---

## 许可

[MIT](../LICENSE)
