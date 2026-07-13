# WebRust

브라우저에서 PC/맥 창을 원격 조작하는 **Rust** 호스트 앱입니다.  
Swift [WebDock](https://github.com/alice-cli/WebDock) 의 크로스플랫폼 재구현.

**언어:** [English](../README.md) · [한국어](README.ko.md) · [日本語](README.ja.md) · [中文](README.zh.md) · [Deutsch](README.de.md) · [Français](README.fr.md)

웹 UI: EN / KO / JA / ZH / DE / FR (상단 언어 메뉴).

| 제품 | 스택 | 번들 ID | 기본 포트 |
|------|------|---------|-----------|
| WebDock | Swift | `com.poc.webdock` | 8080 |
| **WebRust** | Rust | `com.poc.webrust` | 8090 |

---

## 기능

| 기능 | 설명 |
|------|------|
| 창 / 전체 화면 스트리밍 | xcap (macOS / Windows / Linux) |
| 원격 입력 | 마우스, 키보드, 스크롤, 한글 조합 |
| 화질 | 프리셋 + JPEG / PNG / H.264 |
| H.264 | macOS: VideoToolbox · Win/Linux: OpenH264 |
| 인증 | 선택 토큰 + WebSocket Origin 검사 |
| macOS | 메뉴바 + 설정 창 (WebDock과 유사 UX) |

**보안:** LAN을 열면 토큰을 강하게 설정하세요.  
**H.264 + LAN:** 브라우저는 HTTPS/localhost에서만 WebCodecs 지원 — 평문 `http://192.168…` 에서는 JPG를 쓰거나 TLS 프록시를 두세요.

---

## 설치

### Releases

1. [Releases](https://github.com/alice-cli/WebDock-Rust/releases)  
2. OS별 아티팩트 다운로드 (mac zip/pkg · win zip · linux tar.gz)  
3. 권한 허용 후 `http://127.0.0.1:8090`

### 소스 (macOS 앱 · 권장)

```bash
git clone https://github.com/alice-cli/WebDock-Rust.git
cd WebDock-Rust
chmod +x setup_dev_cert.sh build_app.sh install_home.sh
./setup_dev_cert.sh   # 한 번: WebRust Dev 로컬 인증서 (WebDock과 동일 패턴)
./install_home.sh     # ~/WebRust.app
```

**시스템 설정 → 개인정보 보호:** 화면 기록 · 손쉬운 사용 → **WebRust** (WebDock과 별개).

### CLI

```bash
cargo build -p webdock-server --release
./target/release/WebRust --cli --port 8090 --lan --gen-token
```

---

## 서명 (macOS · WebDock과 동일 구조)

| 스크립트 | 역할 |
|----------|------|
| `setup_dev_cert.sh` | `WebRust.keychain-db` + **WebRust Dev** 인증서 |
| `build_app.sh` | 릴리스 빌드 + codesign |
| `install_home.sh` | `~/WebRust.app` 설치 |
| `release_notarize.sh` | (선택) Developer ID + 공증 |

자세한 내용: [SIGNING.md](./SIGNING.md)

---

## 설정

`~/Library/Application Support/WebRust/config.json`

```json
{
  "serverEnabled": true,
  "port": 8090,
  "allowLan": false,
  "token": "비밀-토큰"
}
```

---

## 문제 해결

| 증상 | 확인 |
|------|------|
| 검은 화면 | 화면 기록 권한 |
| 클릭/키보드 무시 | 손쉬운 사용 |
| H.264 멈춤 (폰/LAN) | localhost 또는 HTTPS · 아니면 JPG |
| 재빌드마다 권한 초기화 | `setup_dev_cert.sh` 후 같은 경로 재설치 |

---

## 라이선스

[MIT](../LICENSE)
