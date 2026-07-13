//! Cross-platform settings window + system tray (macOS / Windows / Linux).
//!
//! Uses tao + wry + tray-icon so the same GUI runs on every desktop OS.

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
    Icon, TrayIconBuilder, TrayIconEvent,
};
use wry::WebViewBuilder;

use crate::host::{Host, SharedHost};
use crate::updater;

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

    if let Err(e) = host.start_if_enabled() {
        tracing::warn!(error = %e, "auto-start failed");
    }

    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();

    let window = WindowBuilder::new()
        .with_title("WebRust Settings")
        .with_inner_size(tao::dpi::LogicalSize::new(520.0, 760.0))
        .with_resizable(true)
        .build(&event_loop)?;

    let menu = Menu::new();
    let item_settings = MenuItem::new("Open Settings…", true, None);
    let item_start = MenuItem::new("Start Server", true, None);
    let item_stop = MenuItem::new("Stop Server", true, None);
    let item_open = MenuItem::new("Open Remote in Browser", true, None);
    let item_update = MenuItem::new("Check for Updates…", true, None);
    let item_quit = MenuItem::new("Quit", true, None);
    menu.append(&item_settings)?;
    menu.append(&PredefinedMenuItem::separator())?;
    menu.append(&item_start)?;
    menu.append(&item_stop)?;
    menu.append(&PredefinedMenuItem::separator())?;
    menu.append(&item_open)?;
    menu.append(&item_update)?;
    menu.append(&PredefinedMenuItem::separator())?;
    menu.append(&item_quit)?;

    let id_settings = item_settings.id().as_ref().to_string();
    let id_start = item_start.id().as_ref().to_string();
    let id_stop = item_stop.id().as_ref().to_string();
    let id_open = item_open.id().as_ref().to_string();
    let id_update = item_update.id().as_ref().to_string();
    let id_quit = item_quit.id().as_ref().to_string();

    let tray_icon = make_tray_icon(host.is_running());
    let mut tray_builder = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("WebRust")
        .with_icon(tray_icon);

    // Title text in the menu bar is macOS-only (and readable there).
    #[cfg(target_os = "macos")]
    {
        tray_builder = tray_builder.with_title(if host.is_running() {
            "WebRust ●"
        } else {
            "WebRust ○"
        });
    }

    let tray = tray_builder.build()?;

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

    // Background: check GitHub for a newer release once at startup (non-blocking).
    let host_upd = host.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(4));
        match updater::check_for_update() {
            Ok(info) if info.update_available => {
                tracing::info!(
                    latest = %info.latest_version,
                    current = %info.current_version,
                    "update available"
                );
                // Stash for status_json to surface in settings UI.
                host_upd.set_update_info(Some(info));
            }
            Ok(_) => {}
            Err(e) => tracing::debug!(error = %e, "update check skipped"),
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
                refresh_tray(tray.as_ref(), host_loop.is_running());
            }
            Event::UserEvent(UserEvent::Refresh) => {
                push(None);
                refresh_tray(tray.as_ref(), host_loop.is_running());
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
                        Ok(_) => push(Some("Server started".into())),
                        Err(e) => push(Some(format!("Start failed: {e}"))),
                    }
                    refresh_tray(tray.as_ref(), host_loop.is_running());
                } else if id == id_stop {
                    host_loop.stop();
                    push(Some("Server stopped".into()));
                    refresh_tray(tray.as_ref(), host_loop.is_running());
                } else if id == id_open {
                    if host_loop.is_running() {
                        host_loop.open_remote_ui();
                    } else {
                        push(Some("Start the server first".into()));
                    }
                } else if id == id_update {
                    match updater::check_for_update() {
                        Ok(info) => {
                            host_loop.set_update_info(Some(info.clone()));
                            if info.update_available {
                                push(Some(format!(
                                    "Update available: v{} → v{}",
                                    info.current_version, info.latest_version
                                )));
                                if let Some(url) =
                                    info.html_url.as_ref().or(info.download_url.as_ref())
                                {
                                    crate::util::open_url(url);
                                }
                            } else {
                                push(Some(format!("Up to date (v{})", info.current_version)));
                            }
                        }
                        Err(e) => push(Some(format!("Update check failed: {e}"))),
                    }
                } else if id == id_quit {
                    let _ = proxy.send_event(UserEvent::Quit);
                }
            }
            Event::UserEvent(UserEvent::Ipc(body)) => {
                handle_ipc(&host_loop, &body, &push);
                refresh_tray(tray.as_ref(), host_loop.is_running());
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
                // Keep running in tray (like Swift WebDock).
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
                Ok(()) => push(Some("Saved".into())),
                Err(e) => push(Some(format!("Error: {e}"))),
            }
        }
        "genToken" => match host.gen_token() {
            Ok(_) => push(Some("Token generated".into())),
            Err(e) => push(Some(format!("Error: {e}"))),
        },
        "openRemote" => {
            if host.is_running() {
                host.open_remote_ui();
                push(None);
            } else {
                push(Some("Server is off. Turn the server switch on.".into()));
            }
        }
        "start" => match host.start() {
            Ok(_) => push(Some("Server started".into())),
            Err(e) => push(Some(format!("Start failed: {e}"))),
        },
        "stop" => {
            host.stop();
            push(Some("Server stopped".into()));
        }
        "checkUpdate" => match updater::check_for_update() {
            Ok(info) => {
                host.set_update_info(Some(info.clone()));
                if info.update_available {
                    push(Some(format!(
                        "Update available: v{} → open release page",
                        info.latest_version
                    )));
                } else {
                    push(Some(format!("Up to date (v{})", info.current_version)));
                }
            }
            Err(e) => push(Some(format!("Update check failed: {e}"))),
        },
        "openUpdate" => {
            if let Some(info) = host.update_info() {
                if let Some(url) = info.html_url.or(info.download_url) {
                    crate::util::open_url(&url);
                    push(Some("Opened download page".into()));
                    return;
                }
            }
            // Fresh check
            match updater::check_for_update() {
                Ok(info) => {
                    host.set_update_info(Some(info.clone()));
                    if let Some(url) = info.html_url.or(info.download_url) {
                        crate::util::open_url(&url);
                        push(Some("Opened download page".into()));
                    } else {
                        push(Some("No download URL found".into()));
                    }
                }
                Err(e) => push(Some(format!("Update check failed: {e}"))),
            }
        }
        "quit" => {
            host.stop();
            std::process::exit(0);
        }
        other => push(Some(format!("unknown cmd: {other}"))),
    }
}

fn refresh_tray(tray: Option<&tray_icon::TrayIcon>, running: bool) {
    if let Some(t) = tray {
        let _ = t.set_icon(Some(make_tray_icon(running)));
        #[cfg(target_os = "macos")]
        {
            let _ = t.set_title(Some(if running {
                "WebRust ●"
            } else {
                "WebRust ○"
            }));
        }
        let _ = t.set_tooltip(Some(if running {
            "WebRust — running"
        } else {
            "WebRust — stopped"
        }));
    }
}

fn make_tray_icon(running: bool) -> Icon {
    let size = 32u32;
    let mut rgba = vec![0u8; (size * size * 4) as usize];
    let (r, g, b) = if running {
        (34u8, 197, 94) // green
    } else {
        (59u8, 130, 246) // blue
    };
    for y in 0..size {
        for x in 0..size {
            let i = ((y * size + x) * 4) as usize;
            let cx = x as f32 - 15.5;
            let cy = y as f32 - 15.5;
            let dist = (cx * cx + cy * cy).sqrt();
            if dist < 14.0 {
                let edge = (14.0 - dist).clamp(0.0, 1.5) / 1.5;
                let a = (255.0 * edge.min(1.0)) as u8;
                rgba[i] = r;
                rgba[i + 1] = g;
                rgba[i + 2] = b;
                rgba[i + 3] = a;
            }
            // small "W" mark in center
            if dist < 9.0 {
                let in_w = (x >= 10 && x <= 12 && y >= 10 && y <= 20)
                    || (x >= 19 && x <= 21 && y >= 10 && y <= 20)
                    || (x >= 13 && x <= 14 && y >= 16 && y <= 20)
                    || (x >= 17 && x <= 18 && y >= 16 && y <= 20)
                    || (x >= 15 && x <= 16 && y >= 18 && y <= 21);
                if in_w {
                    rgba[i] = 255;
                    rgba[i + 1] = 255;
                    rgba[i + 2] = 255;
                    rgba[i + 3] = 255;
                }
            }
        }
    }
    Icon::from_rgba(rgba, size, size).expect("tray icon")
}

fn discover_bundled_webui() -> Option<std::path::PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let exe_dir = exe.parent()?;

    // macOS app bundle: Contents/MacOS/WebRust → Contents/Resources/webui
    if let Some(contents) = exe_dir.parent() {
        let bundled = contents.join("Resources").join("webui");
        if bundled.join("index.html").is_file() {
            return Some(bundled);
        }
    }

    // Windows installer / portable: next to exe
    let beside = exe_dir.join("webui");
    if beside.join("index.html").is_file() {
        return Some(beside);
    }

    // Linux tarball layout
    let share = exe_dir
        .parent()
        .map(|p| p.join("share/webrust/webui"))
        .unwrap_or_default();
    if share.join("index.html").is_file() {
        return Some(share);
    }

    // Dev tree
    for rel in ["webui", "../webui", "../../webui"] {
        let p = std::path::PathBuf::from(rel);
        if p.join("index.html").is_file() {
            return Some(p);
        }
    }
    None
}
