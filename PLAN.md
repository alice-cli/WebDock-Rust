# WebDock-Rust 구현 계획서

> WebDock(macOS/Swift)을 **Rust + Tauri 2.x** 기반으로 재구현하여 macOS / Windows / Linux 전 플랫폼을 지원한다.
> 원칙: **① 클린코드 ② 함수·코드·컴포넌트 재활용 ③ 정확한 타입 지정 및 자동 검증(zod 등)**

작성일: 2026-07-13 · 원본 분석 대상: `~/Documents/GitHub/WebDock` (Swift 5.9, SPM, macOS 14+)

### 구현 진행 (2026-07-13)

| 단계 | 상태 | 비고 |
|---|---|---|
| P0 스캐폴딩 | **완료** | workspace, crates, CI |
| P1 서버+프로토콜 | **완료** | axum HTTP/WS, 토큰/IP, WebUI 정적 서빙 |
| P2 캡처 | **완료(실배포)** | `xcap` 폴링 + JPEG (창/디스플레이) |
| P3 입력 | **완료(실배포)** | `enigo` + raise/bounds/앱런처 |
| P4~P6 | 부분 | Tauri 셸 스텁, H.264/IME 고도화 남음 |

로컬 실행: `cargo run -p webdock-server -- --port 8080`

---

## 1. 프로젝트 개요

WebDock은 **호스트 PC의 특정 창(또는 전체 화면)을 LAN의 웹 브라우저에서 실시간으로 보고 조작하는** 앱이다.

포팅해야 할 핵심 기능:

| 영역 | 기능 |
|---|---|
| 캡처/스트리밍 | 창 단위·디스플레이 단위 캡처, JPEG/PNG/H.264 스트리밍, 품질 프리셋(Fast/Balanced/Live), 클라이언트 stats 기반 적응형 비트레이트 |
| 원격 입력 | 마우스(이동/클릭/드래그/스크롤), 키보드(DOM code → OS 키코드), 유니코드 텍스트 주입, 한/영 IME 전환·한글 조합, 클립보드 동기화 |
| 창 제어 | z-order 최상위로 올리기(가려진 창 클릭 문제 해결), 창 리사이즈/닫기, 앱 실행/종료, 앱 런처 |
| 서버 | HTTP/1.1 + WebSocket 단일 포트(기본 8080), 토큰 인증(쿼리/헤더/쿠키), IP·도메인 허용목록, Cloudflare 터널 헤더 지원, 입력 중재(input seat) |
| 시스템 | CPU/RAM/디스크 메트릭, 디스플레이 절전 제어, 잠금화면 감지, 트레이 아이콘, 설정 UI, 자동 시작 |
| 클라이언트 | 순수 HTML/CSS/JS SPA(WebCodecs H.264 디코딩, Canvas 렌더링, i18n 6개 언어) — **그대로 재사용 가능** |

**최우선 설계 결정: 기존 WS 프로토콜(메시지 스키마 + 바이너리 프레이밍)을 그대로 유지**하면 기존 브라우저 클라이언트(`AppJS` 2,044줄)를 무수정 재사용할 수 있다. 이것이 "재활용" 원칙의 가장 큰 승리 지점이다.

---

## 2. 기술 스택 요약

| 레이어 | 선택 | 근거 |
|---|---|---|
| 앱 셸 | **Tauri 2.x** | 크로스플랫폼 트레이/창/설정 UI, 경량 번들, Rust 네이티브 |
| 설정 UI 프론트엔드 | **TypeScript + Vite + SolidJS(또는 React)** | 컴포넌트 재활용, strict 타입 |
| 임베디드 서버 | **axum + tokio-tungstenite** | 손수 만든 Swift HTTP/WS 파서 대체, 검증된 RFC 6455 구현 |
| 직렬화/타입 | **serde + specta + tauri-specta** | Rust 타입 → TS 타입 자동 생성(단일 소스 오브 트루스) |
| 프론트 런타임 검증 | **zod v4** | WS 경계(신뢰 불가 입력) 런타임 검증 |
| 화면 캡처 | **xcap**(스틸) + OS별 네이티브 스트림(§5.2) | 크로스플랫폼 창/화면 캡처 |
| 입력 주입 | **enigo** + OS별 보완 | 크로스플랫폼 마우스/키보드 시뮬레이션 |
| H.264 인코딩 | OS별 HW 인코더 + **openh264** 폴백(§5.3) | 저지연 스트리밍 |

---

## 3. Tauri 2.x 공식 플러그인 전수 조사 (총 31개)

### 3.1 채택 (10개)

| 플러그인 | 플랫폼 | 용도 |
|---|---|---|
| `tauri-plugin-autostart` | Win/mac/Linux | 시스템 시작 시 자동 실행 — 원본은 INI 플래그뿐이었으나 정식 기능으로 승격 |
| `tauri-plugin-clipboard-manager` | 전체 | 클립보드 동기화 (`NSPasteboard` 대체) |
| `tauri-plugin-store` | 전체 | 설정 영속화 (INI → JSON 스토어; §7.2) |
| `tauri-plugin-log` | 전체 | 구조화 로깅 (`tracing` 연동) |
| `tauri-plugin-single-instance` | 전체 | 중복 실행 방지 (포트 충돌 예방) |
| `tauri-plugin-updater` | 전체 | 인앱 자동 업데이트 (원본에 없던 개선) |
| `tauri-plugin-os` | 전체 | OS 정보 → 웹 UI 메트릭 헤더/진단 |
| `tauri-plugin-notification` | 전체 | 원격 접속 알림 (신규 보안 UX) |
| `tauri-plugin-opener` | 전체 | "설정 폴더 열기", 브라우저로 접속 URL 열기 |
| `tauri-plugin-dialog` | 전체 | 오류/확인 다이얼로그 |

### 3.2 조건부 채택 (3개)

| 플러그인 | 판단 |
|---|---|
| `tauri-plugin-shell` | 앱 런처의 폴백 실행 경로로만 제한적 사용(보안 스코프 필수) |
| `tauri-plugin-window-state` | 설정 창 위치 기억 — 편의 기능 |
| `tauri-plugin-global-shortcut` | "서버 긴급 정지" 단축키 등 후순위 |

### 3.3 미채택 (18개) — 사유 명시

| 플러그인 | 미채택 사유 |
|---|---|
| `http`, `websocket`, `upload` | 이들은 **프론트가 클라이언트로 쓰는** 플러그인. WebDock은 서버가 필요하므로 axum 사용 |
| `localhost` | Tauri 웹뷰 자산 서빙용. 우리 서버는 외부 브라우저 대상이므로 부적합 |
| `sql`, `stronghold` | DB 불필요. 토큰은 store + OS 키체인 수준이면 충분 |
| `fs`, `persisted-scope` | 프론트에서 직접 파일 접근할 일 없음(전부 Rust 커맨드 경유) |
| `deep-link`, `cli`, `positioner`, `process` | 해당 기능 없음 |
| `barcode-scanner`, `biometric`, `geolocation`, `haptics`, `nfc` | 모바일 전용 |
| `dialog` 외 나머지 모바일 특화 | 데스크톱 타깃 아님 |

> 트레이 아이콘·메뉴는 플러그인이 아니라 **Tauri 코어의 `tray-icon` 기능**(`tauri = { features = ["tray-icon"] }`)으로 처리한다.

---

## 4. 아키텍처 — Cargo Workspace

클린코드·재활용 원칙을 구조로 강제한다: **플랫폼 독립 로직과 OS별 구현을 crate 경계로 분리**하고, OS별 코드는 trait 뒤에만 존재하게 한다.

```
WebDock-Rust/
├── Cargo.toml                  # [workspace]
├── crates/
│   ├── webdock-protocol/       # ★ WS 메시지 타입 정의 (serde + specta). 의존성 최소.
│   ├── webdock-core/           # 도메인 로직: 입력 중재, 세션, 라우트ID, 적응형 비트레이트
│   ├── webdock-platform/       # ★ trait 정의: Capture, InputInjector, WindowControl,
│   │   │                       #   ImeControl, PowerControl, AppCatalog, Metrics
│   │   ├── src/lib.rs          # trait + 공용 타입만
│   │   ├── src/macos/          # cfg(target_os = "macos")  — SCK/CGEvent/AX
│   │   ├── src/windows/        # cfg(target_os = "windows") — WGC/SendInput/Win32
│   │   └── src/linux/          # cfg(target_os = "linux")  — PipeWire/X11/uinput
│   ├── webdock-encoder/        # 프레임 인코딩 trait + JPEG/PNG/H.264 구현 (HW/SW 백엔드)
│   └── webdock-server/         # axum HTTP/WS 서버, 인증, 허용목록, 피어 관리, 팬아웃
├── src-tauri/                  # Tauri 앱: 트레이, 설정 커맨드, 서버 lifecycle
│   ├── src/commands.rs         # #[tauri::command] + #[specta::specta]
│   └── tauri.conf.json
├── ui-settings/                # 설정 창 프론트 (TS + SolidJS + zod)
│   └── src/bindings.ts         # tauri-specta 자동 생성 (커밋 대상)
├── webui/                      # ★ 원본 WebUI 이식: Swift 문자열 → 실제 정적 파일
│   ├── index.html / app.js / styles.css / i18n.js
│   └── schema/messages.ts      # zod 스키마 (protocol crate에서 생성·검증)
└── .github/workflows/release.yml
```

### 핵심 trait 설계 (`webdock-platform`)

```rust
/// 창/디스플레이 열거 + 프레임 스트림. 모든 OS 구현이 이 trait만 노출한다.
pub trait CaptureBackend: Send + Sync {
    fn list_windows(&self) -> Result<Vec<WindowInfo>, CaptureError>;
    fn list_displays(&self) -> Result<Vec<DisplayInfo>, CaptureError>;
    fn start_stream(&self, target: CaptureTarget, cfg: StreamConfig)
        -> Result<FrameStream, CaptureError>;   // FrameStream = tokio mpsc<RawFrame>
}

pub trait InputInjector: Send + Sync {
    fn mouse(&self, ev: MouseEvent, target: &WindowRef) -> Result<(), InputError>;
    fn key(&self, ev: KeyEvent, target: &WindowRef) -> Result<(), InputError>;
    fn text(&self, s: &str, replace: u32) -> Result<(), InputError>;
}

pub trait WindowControl: Send + Sync {
    fn raise(&self, w: &WindowRef) -> Result<(), WindowError>;   // z-order 최상위
    fn resize(&self, w: &WindowRef, size: Size) -> Result<(), WindowError>;
    fn close(&self, w: &WindowRef) -> Result<(), WindowError>;
    fn bounds(&self, w: &WindowRef) -> Result<Rect, WindowError>; // 좌표 매핑용
}

// ImeControl / PowerControl / AppCatalog / MetricsProvider 동일 패턴
```

- 팩토리 함수 `platform::current() -> PlatformServices` 하나만 `cfg`로 분기. **나머지 코드에는 `cfg(target_os)`가 등장하지 않는 것을 목표**로 한다(클린코드 규칙 §8-3).
- `webdock-core`/`webdock-server`는 trait에만 의존 → 단위 테스트에서 `MockPlatform`으로 전체 서버 로직 테스트 가능.

---

## 5. 크로스플랫폼 구현 전략 (난제 우선)

### 5.1 원본 macOS API → 크로스플랫폼 매핑

| 기능 | macOS (원본) | Windows | Linux | Rust 크레이트 |
|---|---|---|---|---|
| 창/화면 캡처 | ScreenCaptureKit | Windows.Graphics.Capture | PipeWire(Wayland) / X11 | `xcap`(열거+스틸), 스트림은 §5.2 |
| H.264 HW 인코딩 | VideoToolbox | Media Foundation | VA-API | §5.3 |
| 입력 주입 | CGEvent | SendInput | X11 XTEST / Wayland libei | `enigo` |
| 창 raise/포커스 | AX API + NSRunningApplication | `SetForegroundWindow`/`SetWindowPos` | EWMH `_NET_ACTIVE_WINDOW` | `windows` / `x11rb` 직접 |
| 창 열거+bounds | CGWindowList | `EnumWindows`+DWM | X11/wlr 프로토콜 | `xcap` 창 목록 + OS 보완 |
| IME 전환 | Carbon TIS | IMM32/TSF | IBus/Fcitx D-Bus | OS별 직접 구현(§5.4) |
| 클립보드 | NSPasteboard | — | — | `arboard` 또는 clipboard-manager 플러그인 |
| 절전 제어 | IOPMAssertion | `SetThreadExecutionState` | logind D-Bus Inhibit | `keepawake` 또는 직접 |
| 잠금 감지 | CGSession | `WTSRegisterSessionNotification` | logind `LockedHint` | OS별 직접 |
| 메트릭 | Mach API | — | — | `sysinfo` |
| LAN IP | getifaddrs | — | — | `if-addrs` |
| 앱 열거/실행 | /Applications 스캔 + NSWorkspace | 시작 메뉴 `.lnk` | `.desktop` 파일 | `freedesktop-desktop-entry` + OS별 |
| 앱 아이콘 | NSWorkspace.icon | exe 아이콘 추출 | 아이콘 테마 조회 | OS별 직접 (`IconCache` 재현) |

### 5.2 캡처 스트림 — 최대 리스크 영역

- **1차(MVP)**: `xcap`으로 폴링 캡처(창·화면 스틸 → JPEG). 세 OS 모두 동작, 원본의 JPEG 모드와 프로토콜 호환. 15~30fps 확보 가능.
- **2차(성능)**: OS별 네이티브 스트림 백엔드를 `CaptureBackend` 뒤에 추가.
  - macOS: `screencapturekit` 크레이트 (SCStream)
  - Windows: `windows-capture` 크레이트 (WGC, 매우 성숙)
  - Linux: `ashpd`(xdg-desktop-portal) + PipeWire — **Wayland은 창 단위 캡처에 포털 승인 UI가 강제됨**을 UX에 반영
- **Linux/Wayland 제약 명시**: 창 단위 캡처·입력 주입 모두 제한적 → X11 full 지원 + Wayland는 "전체 화면 스트림 + libei 입력(실험)"으로 단계적 지원. 기능 매트릭스를 README에 문서화.

### 5.3 인코딩

`webdock-encoder`에 `Encoder` trait 하나, 백엔드 4개:

1. `JpegEncoder` (`turbojpeg`, 폴백 `image`) — MVP 기본
2. `PngEncoder` (`image`)
3. `H264Hw` — macOS `VideoToolbox`(FFI), Windows Media Foundation(`windows` crate), Linux VA-API(`cros-codecs`)
4. `H264Sw` — `openh264` (Cisco, 전 OS 컴파일 확인됨) — HW 실패 시 폴백

바이너리 패킷 포맷은 원본 그대로: `[type u8][flags u8][pts_us i64 BE][len u32 BE][AVCC AU]` → 기존 클라이언트 WebCodecs 디코더 무수정 호환.

### 5.4 IME (한/영 전환)

원본의 차별화 기능이므로 유지하되 OS별 격리:
- macOS: TIS FFI (`core-foundation` + 직접 바인딩) — 원본 로직 이식
- Windows: `ActivateKeyboardLayout` + IMM32 열림 상태 제어
- Linux: IBus/Fcitx5 D-Bus (`zbus`)
- 미지원 환경에서는 `ime` 능력 플래그를 클라이언트에 내려서 UI 버튼 자동 숨김 (**capability negotiation** — `hello` 메시지에 서버 능력 포함, 프로토콜의 유일한 확장점)

---

## 6. 타입 안전성 전략 (요구사항 ③)

**원칙: 타입은 Rust에서 한 번만 정의하고, TS는 전부 자동 생성. 신뢰 경계에서는 런타임 검증.**

```
webdock-protocol (Rust: serde + specta 파생)
      │
      ├─► tauri-specta ──► ui-settings/src/bindings.ts   (설정 UI: 컴파일타임 타입)
      │
      └─► specta-typescript ──► webui/schema/types.ts
                                    │
                                    └─► zod 스키마 (WS 런타임 검증)
```

1. **WS 프로토콜 단일 정의** — `webdock-protocol` crate에 모든 메시지를 태그드 enum으로:
   ```rust
   #[derive(Serialize, Deserialize, specta::Type)]
   #[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
   pub enum ClientMessage {
       Select { id: RouteId },
       Move { x: Fraction, y: Fraction, button: MouseButton },
       Key { code: DomKeyCode, meta: bool, ctrl: bool, shift: bool, alt: bool },
       // … 원본 26개 메시지 타입 전부
   }
   ```
   - `deny_unknown_fields` + newtype(`Fraction`은 0.0..=1.0 검증하는 `TryFrom`)으로 **역직렬화 = 검증**이 되게 한다. 별도 validator 크레이트 불필요.
2. **Tauri 커맨드** — 전 커맨드에 `#[specta::specta]` 부착, `tauri-specta`가 debug 빌드 시 `bindings.ts` 생성. 프론트에서 `invoke("...")` 문자열 호출 금지, 생성된 함수만 사용(ESLint 규칙으로 강제).
3. **웹 클라이언트 경계** — 브라우저 ↔ 서버 WS는 외부 입력이므로 **zod v4**로 수신 메시지 파싱(`z.discriminatedUnion("type", …)`). zod 스키마는 생성된 TS 타입에 `satisfies z.ZodType<ServerMessage>`를 붙여 **Rust 타입과 어긋나면 tsc가 실패**하게 한다.
4. **Rust 측 강제 장치** — `#![deny(clippy::unwrap_used, clippy::expect_used)]`(라이브러리 crate), `#[must_use]`, `thiserror` 기반 에러 enum(문자열 에러 금지), `cargo clippy -- -D warnings` CI 게이트.
5. **TS 측 강제 장치** — `strict: true`, `noUncheckedIndexedAccess`, `exactOptionalPropertyTypes`. `any` 금지(ESLint `@typescript-eslint/no-explicit-any: error`).
6. **바이너리 프레임** — `zerocopy` 또는 수동 파서 + 라운드트립 property 테스트(`proptest`)로 원본 Swift 인코더와 골든 파일 비교.

---

## 7. 클린코드·재사용 규칙 (요구사항 ①②)

### 7.1 코드 규칙
- 함수 단일 책임, 파일 400줄/함수 60줄 소프트 리밋 (원본 `AppJS.swift` 2,044줄 단일 문자열 같은 구조 금지 — 웹 UI는 실제 파일로 분리, Vite로 번들)
- 모든 public 아이템 rustdoc, 모듈 헤더에 "왜 존재하는가" 1문단
- 포맷/린트 자동화: `rustfmt` + `clippy`(pedantic 선별) / `prettier` + `eslint` — CI 필수 통과
- 매직넘버 금지: 원본의 튜닝 상수(포커스 캐시 0.45s, 드래그 grace 2.0s, 백프레셔 큐 2, 비트레이트 사다리 {4.5, 2.8, 1.6, 0.9}Mbps)는 `core/src/tuning.rs`에 상수 + 근거 주석으로 집약

### 7.2 재사용 목록 (원본 → 신규)
| 원본 자산 | 재사용 방식 |
|---|---|
| WebUI (HTML/CSS/JS/i18n) | **그대로 이식** (Swift 문자열 → 정적 파일), 이후 점진적 TS 전환 |
| WS 프로토콜 26개 메시지 + 바이너리 포맷 | **무변경 유지** — 클라이언트/서버 어느 쪽도 재작성 불필요한 계약 |
| 입력 중재/백프레셔/적응 비트레이트 로직 | 알고리즘 그대로 `webdock-core`에 이식 (플랫폼 무관 순수 로직) |
| 토큰 인증 흐름(쿼리/헤더/쿠키, 303 로그인, 상수시간 비교) | `webdock-server` 미들웨어로 이식 (`subtle` crate) |
| 설정 스키마 | INI → `tauri-plugin-store` JSON. **기존 `config.ini` 발견 시 1회 마이그레이션** |
| 브랜드 아이콘/파비콘 | 그대로 |
| 설정 UI | 신규 작성 (AppKit → SolidJS 컴포넌트: `Switch`, `EditableList`(IP/도메인 목록 공용), `TokenField` — 컴포넌트 재사용 강제) |

---

## 8. 마일스톤

| 단계 | 산출물 | 완료 기준 |
|---|---|---|
| **P0 스캐폴딩** (1주) | workspace + Tauri 앱 + CI + specta 파이프라인 | 3 OS에서 트레이 아이콘 뜨고 `bindings.ts` 자동 생성 |
| **P1 서버+프로토콜** (2주) | axum HTTP/WS, 토큰/허용목록, protocol crate, WebUI 정적 서빙 | 기존 브라우저 클라이언트가 접속해 창 목록(모의 데이터) 수신 |
| **P2 캡처 MVP** (2주) | `xcap` 폴링 캡처 + JPEG 스트림 + 팬아웃/백프레셔 | 3 OS에서 창 스트리밍 시청 가능 |
| **P3 입력 주입** (2~3주) | enigo + 좌표매핑 + DOM→OS 키맵 + 창 raise + 입력 중재 | 브라우저에서 클릭/타이핑/드래그 동작 (mac/Win 완전, Linux X11) |
| **P4 시스템 통합** (2주) | 설정 UI, store 마이그레이션, 메트릭, 클립보드, 앱 런처, 절전 | 원본 설정 화면 기능 동등 |
| **P5 고급 스트리밍** (3주) | 네이티브 캡처 스트림 + H.264 HW/SW + 적응 비트레이트 | Live 프리셋에서 원본 수준 지연시간 |
| **P6 IME·마감** (2주) | 한/영 IME 3 OS, capability 협상, Wayland 문서화, 릴리스 파이프라인 | 서명·공증된 3 OS 인스톨러 자동 릴리스 |

각 단계 종료 시: 프로토콜 골든 테스트 + clippy/eslint 클린 + 기능 매트릭스 문서 갱신.

---

## 9. CI/CD·패키징

- `tauri-apps/tauri-action` 기반 매트릭스 빌드 (macos-14 / windows-latest / ubuntu-22.04)
- 산출물: macOS `.dmg`+`.app`(Developer ID 서명 + notarytool 공증 — 기존 워크플로 이식), Windows `.msi`/`.exe`(NSIS, 코드사인), Linux `.deb`/`.rpm`/AppImage
- `tauri-plugin-updater` 서명 키 발급, GitHub Releases를 업데이트 피드로
- macOS 권한: 기존과 동일하게 Screen Recording / Accessibility TCC 안내 플로우 이식 (`NSScreenCaptureUsageDescription` 등 Info.plist 키 유지)

## 10. 리스크 요약

| 리스크 | 심각도 | 완화책 |
|---|---|---|
| Wayland 창 단위 캡처/입력 제약 | 높음 | X11 우선 지원, Wayland는 전체화면+포털, capability 협상으로 UI 자동 축소 |
| H.264 HW 인코딩 3 OS 편차 | 중간 | openh264 SW 폴백 상시 유지, MVP는 JPEG로 가치 검증 |
| Windows `SetForegroundWindow` 제약(포그라운드 잠금) | 중간 | `AttachThreadInput` 우회 + Alt 키 트릭, 실패 시 클릭 좌표 직접 타깃팅 |
| IME 제어 OS별 편차 | 중간 | 능력 플래그로 격리, macOS부터 완성 |
| enigo Wayland 실험 상태 | 중간 | X11 폴백, libei는 feature flag |

## 11. 참고 링크

- Tauri 2.x 플러그인 목록: https://v2.tauri.app/plugin/
- tauri-specta: https://github.com/specta-rs/tauri-specta
- xcap: https://github.com/nashaofu/xcap · windows-capture: https://crates.io/crates/windows-capture
- enigo: https://github.com/enigo-rs/enigo
- openh264-rs: https://github.com/ralfbiedert/openh264-rs · cros-codecs: https://crates.io/crates/cros-codecs
- scap(참고, 유지보수 중단): https://github.com/CapSoftware/scap
