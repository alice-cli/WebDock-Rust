//! macOS settings window + menu bar tray (like Swift WebDock).

use std::rc::Rc;
use std::sync::Arc;

use serde::Deserialize;
use tao::{
    event::{Event, StartCause, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder},
    window::WindowBuilder,
};
use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    TrayIconBuilder, TrayIconEvent,
};
use wry::WebViewBuilder;

use crate::host::{Host, SharedHost};

const SETTINGS_HTML: &str = include_str!("../settings.html");

#[derive(Debug, Clone)]
enum UserEvent {
    TrayMenu(String),
    TrayClick,
    Ipc(String),
    Refresh,
    Quit,
}

#[derive(Debug, Deserialize)]
struct IpcMsg {
    cmd: String,
    #[serde(default, rename = "serverEnabled")]
    server_enabled: Option<bool>,
    #[serde(default)]
    port: Option<u16>,
    #[serde(default, rename = "allowLan")]
    allow_lan: Option<bool>,
    #[serde(default)]
    token: Option<String>,
    #[serde(default, rename = "allowedDomains")]
    allowed_domains: Option<Vec<String>>,
    #[serde(default, rename = "ipAllowlistEnabled")]
    ip_allowlist_enabled: Option<bool>,
    #[serde(default, rename = "allowedIps")]
    allowed_ips: Option<Vec<String>>,
}

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let webui = discover_bundled_webui();
    let host: SharedHost = Arc::new(Host::new(webui)?);

    // Auto-start only if user left the toggle ON (same as Swift WebDock).
    if let Err(e) = host.start_if_enabled() {
        tracing::warn!(error = %e, "auto-start failed");
    }

    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();

    let window = WindowBuilder::new()
        .with_title("WebRust 설정")
        .with_inner_size(tao::dpi::LogicalSize::new(520.0, 720.0))
        .with_resizable(true)
        .build(&event_loop)?;

    // Tray menu
    let menu = Menu::new();
    let item_settings = MenuItem::new("설정 열기…", true, None);
    let item_start = MenuItem::new("서버 시작", true, None);
    let item_stop = MenuItem::new("서버 중지", true, None);
    let item_open = MenuItem::new("브라우저에서 원격 열기", true, None);
    let item_quit = MenuItem::new("종료", true, None);
    menu.append(&item_settings)?;
    menu.append(&PredefinedMenuItem::separator())?;
    menu.append(&item_start)?;
    menu.append(&item_stop)?;
    menu.append(&PredefinedMenuItem::separator())?;
    menu.append(&item_open)?;
    menu.append(&PredefinedMenuItem::separator())?;
    menu.append(&item_quit)?;

    let id_settings = item_settings.id().as_ref().to_string();
    let id_start = item_start.id().as_ref().to_string();
    let id_stop = item_stop.id().as_ref().to_string();
    let id_open = item_open.id().as_ref().to_string();
    let id_quit = item_quit.id().as_ref().to_string();

    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("WebRust")
        .with_title(if host.is_running() {
            "WebRust ●"
        } else {
            "WebRust ○"
        })
        .build()?;

    let proxy_tray = proxy.clone();
    std::thread::spawn(move || {
        let rx = TrayIconEvent::receiver();
        while let Ok(ev) = rx.recv() {
            if let TrayIconEvent::Click { .. } = ev {
                let _ = proxy_tray.send_event(UserEvent::TrayClick);
            }
        }
    });
    let proxy_menu = proxy.clone();
    std::thread::spawn(move || {
        let rx = MenuEvent::receiver();
        while let Ok(ev) = rx.recv() {
            let id = ev.id.as_ref().to_string();
            let _ = proxy_menu.send_event(UserEvent::TrayMenu(id));
        }
    });

    let proxy_ipc = proxy.clone();
    let webview = Rc::new(
        WebViewBuilder::new()
            .with_html(SETTINGS_HTML)
            .with_ipc_handler(move |req| {
                let body = req.body().to_string();
                let _ = proxy_ipc.send_event(UserEvent::Ipc(body));
            })
            .build(&window)?,
    );

    let proxy_tick = proxy.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_secs(2));
        if proxy_tick.send_event(UserEvent::Refresh).is_err() {
            break;
        }
    });

    window.set_focus();

    let mut tray = Some(tray);
    let host_loop = host.clone();
    let webview_loop = webview.clone();

    event_loop.run(move |event, _target, control_flow| {
        *control_flow = ControlFlow::Wait;

        let push = |message: Option<String>| {
            let mut state = host_loop.status_json();
            if let Some(m) = message {
                if let Some(o) = state.as_object_mut() {
                    o.insert("message".into(), serde_json::Value::String(m));
                }
            }
            let js = format!("window.__onHost && window.__onHost({});", state);
            let _ = webview_loop.evaluate_script(&js);
        };

        match event {
            Event::NewEvents(StartCause::Init) => {
                push(None);
                update_tray_title(tray.as_ref(), host_loop.is_running());
            }
            Event::UserEvent(UserEvent::Refresh) => {
                push(None);
                update_tray_title(tray.as_ref(), host_loop.is_running());
            }
            Event::UserEvent(UserEvent::TrayClick) => {
                window.set_visible(true);
                window.set_focus();
                push(None);
            }
            Event::UserEvent(UserEvent::TrayMenu(id)) => {
                if id == id_settings {
                    window.set_visible(true);
                    window.set_focus();
                    push(None);
                } else if id == id_start {
                    match host_loop.start() {
                        Ok(_) => push(Some("서버 시작됨".into())),
                        Err(e) => push(Some(format!("시작 실패: {e}"))),
                    }
                    update_tray_title(tray.as_ref(), host_loop.is_running());
                } else if id == id_stop {
                    host_loop.stop();
                    push(Some("서버 중지됨".into()));
                    update_tray_title(tray.as_ref(), host_loop.is_running());
                } else if id == id_open {
                    if host_loop.is_running() {
                        host_loop.open_remote_ui();
                    } else {
                        push(Some("먼저 서버를 켜 주세요".into()));
                    }
                } else if id == id_quit {
                    let _ = proxy.send_event(UserEvent::Quit);
                }
            }
            Event::UserEvent(UserEvent::Ipc(body)) => {
                handle_ipc(&host_loop, &body, &push);
                update_tray_title(tray.as_ref(), host_loop.is_running());
            }
            Event::UserEvent(UserEvent::Quit) => {
                host_loop.stop();
                tray.take();
                *control_flow = ControlFlow::Exit;
            }
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                // Keep running in menu bar (like Swift WebDock).
                window.set_visible(false);
            }
            _ => {}
        }
    });
}

fn handle_ipc(host: &SharedHost, body: &str, push: &impl Fn(Option<String>)) {
    let msg: IpcMsg = match serde_json::from_str(body) {
        Ok(m) => m,
        Err(e) => {
            push(Some(format!("ipc error: {e}")));
            return;
        }
    };
    match msg.cmd.as_str() {
        "get" => push(None),
        "set" => {
            let cur = host.config();
            let r = host.set_from_status_fields(
                msg.server_enabled.unwrap_or(cur.server_enabled),
                msg.port.unwrap_or(cur.port),
                msg.allow_lan.unwrap_or(cur.allow_lan),
                msg.token.unwrap_or(cur.token),
                msg.allowed_domains.unwrap_or(cur.allowed_domains),
                msg.ip_allowlist_enabled.unwrap_or(cur.ip_allowlist_enabled),
                msg.allowed_ips.unwrap_or(cur.allowed_ips),
            );
            match r {
                Ok(()) => push(Some("저장됨".into())),
                Err(e) => push(Some(format!("오류: {e}"))),
            }
        }
        "genToken" => match host.gen_token() {
            Ok(_) => push(Some("토큰 생성됨".into())),
            Err(e) => push(Some(format!("오류: {e}"))),
        },
        "openRemote" => {
            if host.is_running() {
                host.open_remote_ui();
                push(None);
            } else {
                push(Some(
                    "서버가 꺼져 있습니다. 서버 스위치를 켜 주세요.".into(),
                ));
            }
        }
        "start" => match host.start() {
            Ok(_) => push(Some("서버 시작됨".into())),
            Err(e) => push(Some(format!("시작 실패: {e}"))),
        },
        "stop" => {
            host.stop();
            push(Some("서버 중지됨".into()));
        }
        "quit" => {
            host.stop();
            std::process::exit(0);
        }
        other => push(Some(format!("unknown cmd: {other}"))),
    }
}

fn update_tray_title(tray: Option<&tray_icon::TrayIcon>, running: bool) {
    if let Some(t) = tray {
        let _ = t.set_title(Some(if running {
            "WebRust ●"
        } else {
            "WebRust ○"
        }));
    }
}

fn discover_bundled_webui() -> Option<std::path::PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let mac_os = exe.parent()?;
    let contents = mac_os.parent()?;
    let bundled = contents.join("Resources").join("webui");
    if bundled.join("index.html").is_file() {
        Some(bundled)
    } else {
        let dev = std::path::PathBuf::from("webui");
        if dev.join("index.html").is_file() {
            Some(dev)
        } else {
            None
        }
    }
}
