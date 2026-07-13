use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket};
use axum::extract::{ConnectInfo, Query, State, WebSocketUpgrade};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use axum::Json;
use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};
use webdock_core::{InputKind, SeatResult};
use webdock_platform::{KeyEvent, MouseEvent, MousePhase, WindowRef};
use webdock_protocol::{ClientMessage, ImePayload, RouteId, ServerMessage};

use crate::auth;
use crate::state::{AppState, SharedState};
use crate::static_files::TokenQuery;

pub async fn health() -> &'static str {
    "ok"
}

pub async fn status_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(q): Query<TokenQuery>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> Response {
    let cfg = state.config.lock().clone();
    let ip = addr.ip().to_string();
    if let Err(e) = auth::check_access(&cfg, &headers, q.token.as_deref(), &ip) {
        return e.into_ws_response();
    }
    Json(json!({
        "serverEnabled": cfg.server_enabled,
        "port": cfg.port,
        "allowLan": cfg.allow_lan,
        "hasToken": cfg.has_token(),
        "platform": state.platform.platform_name,
        "urls": cfg.connection_urls(&crate::lan_addresses()),
    }))
    .into_response()
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(q): Query<TokenQuery>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> Response {
    let cfg = state.config.lock().clone();
    let ip = addr.ip().to_string();
    if let Err(e) = auth::check_access(&cfg, &headers, q.token.as_deref(), &ip) {
        warn!(%ip, "ws auth failed");
        return e.into_ws_response();
    }
    if let Err(e) = auth::check_ws_origin(&cfg, &headers) {
        warn!(%ip, "ws origin rejected");
        return e.into_ws_response();
    }
    ws.on_upgrade(move |socket| peer_loop(socket, state, ip))
}

async fn peer_loop(socket: WebSocket, state: SharedState, ip: String) {
    let peer_id = state.alloc_peer_id();
    let (mut sink, mut stream) = socket.split();
    let (ctrl_tx, mut ctrl_rx) = mpsc::channel::<String>(AppState::ctrl_capacity());
    let (video_tx, mut video_rx) = mpsc::channel::<Vec<u8>>(AppState::video_capacity());
    state.register_peer(peer_id, ctrl_tx, video_tx, ip.clone());
    info!(peer_id, %ip, "ws connected");

    state.send_text(peer_id, state.hello_json());
    state.push_windows_to(peer_id);
    state.broadcast_metrics();
    state.broadcast_clients();

    // Outbound: prefer control messages so video backpressure never starves JSON.
    let out = tokio::spawn(async move {
        loop {
            tokio::select! {
                biased;
                msg = ctrl_rx.recv() => {
                    match msg {
                        Some(t) => {
                            if sink.send(Message::Text(t.into())).await.is_err() {
                                break;
                            }
                        }
                        None => break,
                    }
                }
                msg = video_rx.recv() => {
                    match msg {
                        Some(b) => {
                            if sink.send(Message::Binary(b.into())).await.is_err() {
                                break;
                            }
                        }
                        None => break,
                    }
                }
            }
        }
    });

    while let Some(Ok(msg)) = stream.next().await {
        match msg {
            Message::Text(t) => {
                if let Err(e) = handle_client_text(&state, peer_id, &ip, &t) {
                    debug!(error = %e, "client message error");
                }
            }
            Message::Binary(_) => {}
            Message::Ping(_) | Message::Pong(_) => {}
            Message::Close(_) => break,
        }
    }

    out.abort();
    state.unregister_peer(peer_id);
    info!(peer_id, "ws disconnected");
}

fn handle_client_text(
    state: &Arc<AppState>,
    peer_id: u64,
    ip: &str,
    text: &str,
) -> Result<(), String> {
    let msg = ClientMessage::from_json(text).map_err(|e| e.to_string())?;
    match msg {
        ClientMessage::Select { id } => {
            let prev = state.set_viewing(peer_id, id.as_i64());
            state.broadcast_clients();
            if webdock_platform::is_display_route(id) {
                let _ = state.platform.power.wake_display();
            }
            // Always (re)start capture for the selected route so H.264 gets a fresh
            // encoder + h264config. Re-selecting a stuck stream used to no-op.
            if prev != Some(id.as_i64()) {
                restart_stream(state.clone(), id);
            } else {
                // Same window: ensure stream is alive and force a keyframe/config.
                ensure_stream(state.clone(), id);
                let target = AppState::capture_target(id.as_i64());
                state.platform.capture.request_keyframe(target);
            }
            state.push_windows_to(peer_id);
        }
        ClientMessage::Refresh => {
            state.push_windows_to(peer_id);
            state.broadcast_metrics();
        }
        ClientMessage::Apps => {
            if let Ok(list) = state.platform.apps.list_apps() {
                if let Ok(s) = (ServerMessage::Apps { list }).to_json() {
                    state.send_text(peer_id, s);
                }
            }
        }
        ClientMessage::Down {
            x,
            y,
            button,
            count,
        } => {
            if !claim(state, peer_id, ip, InputKind::Down) {
                return Ok(());
            }
            let _ = state.platform.input.mouse(
                &MouseEvent {
                    phase: MousePhase::Down,
                    x: x.clamp().get(),
                    y: y.clamp().get(),
                    button: button.0,
                    click_count: count,
                },
                target_window(state, peer_id).as_ref(),
            );
        }
        ClientMessage::Move {
            x,
            y,
            button,
            count,
        } => {
            if !claim(state, peer_id, ip, InputKind::Move) {
                return Ok(());
            }
            let _ = state.platform.input.mouse(
                &MouseEvent {
                    phase: MousePhase::Move,
                    x: x.clamp().get(),
                    y: y.clamp().get(),
                    button: button.0,
                    click_count: count,
                },
                target_window(state, peer_id).as_ref(),
            );
        }
        ClientMessage::Up {
            x,
            y,
            button,
            count,
        } => {
            if !claim(state, peer_id, ip, InputKind::Up) {
                return Ok(());
            }
            let _ = state.platform.input.mouse(
                &MouseEvent {
                    phase: MousePhase::Up,
                    x: x.clamp().get(),
                    y: y.clamp().get(),
                    button: button.0,
                    click_count: count,
                },
                target_window(state, peer_id).as_ref(),
            );
        }
        ClientMessage::Click { x, y } => {
            if !claim(state, peer_id, ip, InputKind::Down) {
                return Ok(());
            }
            let t = target_window(state, peer_id);
            let _ = state.platform.input.mouse(
                &MouseEvent {
                    phase: MousePhase::Down,
                    x: x.clamp().get(),
                    y: y.clamp().get(),
                    button: 0,
                    click_count: 1,
                },
                t.as_ref(),
            );
            let _ = state.platform.input.mouse(
                &MouseEvent {
                    phase: MousePhase::Up,
                    x: x.clamp().get(),
                    y: y.clamp().get(),
                    button: 0,
                    click_count: 1,
                },
                t.as_ref(),
            );
            let _ = claim(state, peer_id, ip, InputKind::Up);
        }
        ClientMessage::Scroll { x, y, dx, dy } => {
            if !claim(state, peer_id, ip, InputKind::Scroll) {
                return Ok(());
            }
            let _ = state.platform.input.scroll(
                dx,
                dy,
                x.clamp().get(),
                y.clamp().get(),
                target_window(state, peer_id).as_ref(),
            );
        }
        ClientMessage::Key {
            code,
            meta,
            ctrl,
            shift,
            alt,
            ime: _,
        } => {
            if !claim(state, peer_id, ip, InputKind::Key) {
                return Ok(());
            }
            // Cmd/Ctrl+C·X → host pasteboard → browser (when clipAuto on).
            let is_copy = (meta || ctrl) && !shift && !alt && (code == "KeyC" || code == "KeyX");
            let before = if is_copy && state.peer_clip_auto(peer_id) {
                Some(webdock_platform::clipboard::change_count())
            } else {
                None
            };
            let _ = state.platform.input.key(
                &KeyEvent {
                    code: code.clone(),
                    meta,
                    ctrl,
                    shift,
                    alt,
                },
                target_window(state, peer_id).as_ref(),
            );
            if let Some(from) = before {
                let st = state.clone();
                std::thread::spawn(move || {
                    let text = webdock_platform::clipboard::read_string_after_change(from, 900);
                    if !st.peer_clip_auto(peer_id) {
                        return;
                    }
                    st.push_clipboard_to(peer_id, text, false);
                });
            }
        }
        ClientMessage::Text { value, replace } => {
            if !claim(state, peer_id, ip, InputKind::Text) {
                return Ok(());
            }
            let _ =
                state
                    .platform
                    .input
                    .text(&value, replace, target_window(state, peer_id).as_ref());
        }
        ClientMessage::Ime { korean } => {
            let result = if let Some(k) = korean {
                state.platform.ime.set_korean(k)
            } else {
                let cur = state
                    .platform
                    .ime
                    .current_korean()
                    .map(|(k, _)| k)
                    .unwrap_or(false);
                state.platform.ime.set_korean(!cur)
            };
            if let Ok((k, label)) = result {
                if let Ok(s) = (ServerMessage::Ime {
                    payload: ImePayload { korean: k, label },
                })
                .to_json()
                {
                    state.send_text(peer_id, s);
                }
            }
        }
        ClientMessage::ImeState | ClientMessage::ImeHeal { .. } => {
            if let Some((k, label)) = state.platform.ime.current_korean() {
                if let Ok(s) = (ServerMessage::Ime {
                    payload: ImePayload { korean: k, label },
                })
                .to_json()
                {
                    state.send_text(peer_id, s);
                }
            }
        }
        ClientMessage::Launch { path, new_instance } => {
            // Allowlist enforced inside platform::apps::launch.
            if let Err(e) = state.platform.apps.launch(&path, new_instance) {
                warn!(error = %e, path = %path, "launch rejected");
            }
        }
        ClientMessage::Close { id, pid, title } => {
            // Only close routes that appear in the current window list.
            if !state.route_in_window_list(id.as_i64()) {
                warn!(route = id.as_i64(), "close rejected — unknown route");
                return Ok(());
            }
            let resolved_pid = {
                let from_list = webdock_platform::native_pid_for_route(id).unwrap_or(0);
                let from_client = pid.unwrap_or(0);
                // Prefer host-resolved pid; only accept client pid if it matches list.
                if from_list > 0 {
                    from_list
                } else if from_client > 0 && state.pid_in_window_list(from_client) {
                    from_client
                } else {
                    0
                }
            };
            let w = WindowRef {
                id,
                pid: resolved_pid,
            };
            match state.platform.windows.close(&w) {
                Ok(()) => {
                    state.push_windows_to(peer_id);
                }
                Err(e) => {
                    warn!(error = %e, "close window failed");
                }
            }
            let _ = title;
        }
        ClientMessage::Quit { pid } => {
            if !state.pid_in_window_list(pid) {
                warn!(pid, "quit rejected — pid not in window list");
                return Ok(());
            }
            if let Err(e) = state.platform.apps.quit_pid(pid) {
                warn!(error = %e, pid, "quit failed");
            }
        }
        ClientMessage::ClipAuto { value } => {
            state.set_clip_auto(peer_id, value);
            debug!(peer_id, value, "clipAuto");
        }
        ClientMessage::ClipboardGet => {
            let text = webdock_platform::clipboard::read_string();
            state.push_clipboard_to(peer_id, text, true);
        }
        ClientMessage::Quality { value } => {
            state.stream_cfg.lock().jpeg_quality = value.clamp(0.2, 1.0);
        }
        ClientMessage::Format { value } => {
            use webdock_platform::StreamFormat;
            let fmt = match value.to_ascii_lowercase().as_str() {
                "png" => StreamFormat::Png,
                "h264" | "avc" | "video" => StreamFormat::H264,
                _ => StreamFormat::Jpeg,
            };
            let prev;
            {
                // Single lock — double-lock on parking_lot deadlocks H.264 switch.
                let mut cfg = state.stream_cfg.lock();
                prev = cfg.format;
                cfg.format = fmt;
                if fmt == StreamFormat::H264 {
                    cfg.fps = cfg.fps.max(24).min(30);
                    // SW OpenH264 (Win/Linux) can't hold 1920 realtime — clamp harder.
                    cfg.max_width = cfg
                        .max_width
                        .max(1280)
                        .min(webdock_core::tuning::h264_max_width());
                    cfg.bitrate_bps = cfg
                        .bitrate_bps
                        .max(webdock_core::tuning::BROADCAST_BITRATE_BPS)
                        .min(12_000_000);
                }
            }
            info!(?fmt, "stream format");
            if prev != fmt {
                if let Some(route) = state.viewing_of(peer_id) {
                    restart_stream(state.clone(), RouteId(route));
                }
            }
        }
        ClientMessage::Preset { value } => {
            let mut cfg = state.stream_cfg.lock();
            match value.to_ascii_lowercase().as_str() {
                "fast" | "low" => {
                    cfg.fps = 15;
                    cfg.jpeg_quality = 0.7;
                    cfg.max_width = 1280;
                    cfg.format = webdock_platform::StreamFormat::Jpeg;
                    cfg.bitrate_bps = 1_600_000;
                }
                "broadcast" | "high" | "live" | "h264" => {
                    // HW VideoToolbox (macOS): 1080p30. SW OpenH264: 720p-class.
                    cfg.fps = 30;
                    cfg.jpeg_quality = 1.0;
                    cfg.max_width = webdock_core::tuning::h264_max_width();
                    cfg.format = webdock_platform::StreamFormat::H264;
                    cfg.bitrate_bps = webdock_core::tuning::BROADCAST_BITRATE_BPS;
                }
                _ => {
                    cfg.fps = 20;
                    cfg.jpeg_quality = 0.92;
                    cfg.max_width = 1600;
                    cfg.format = webdock_platform::StreamFormat::Jpeg;
                    cfg.bitrate_bps = 2_800_000;
                }
            }
            let route = state.viewing_of(peer_id);
            drop(cfg);
            if let Some(route) = route {
                restart_stream(state.clone(), RouteId(route));
            }
        }
        ClientMessage::Keyframe { id } => {
            let target = AppState::capture_target(id.as_i64());
            // Resets need_idr + re-emits avcC on next IDR (see force_keyframe).
            state.platform.capture.request_keyframe(target);
        }
        ClientMessage::Stats { pressure, .. } => {
            // Adaptive bitrate ladder under playout stress (H.264) — live apply.
            if let Some(p) = pressure {
                let ladder = webdock_core::tuning::BITRATE_LADDER_MBPS;
                let idx = (p.clamp(0, 3)) as usize;
                let br = (ladder[idx] * 1_000_000.0) as u32;
                state.stream_cfg.lock().bitrate_bps = br;
                if let Some(route) = state.viewing_of(peer_id) {
                    let target = AppState::capture_target(route);
                    state.platform.capture.set_bitrate(target, br);
                }
            }
        }
        ClientMessage::Fps { value } => {
            state.stream_cfg.lock().fps = value.clamp(1, 60) as u32;
            if let Some(route) = state.viewing_of(peer_id) {
                restart_stream(state.clone(), RouteId(route));
            }
        }
        ClientMessage::Resize { w, h } => {
            if let Some(target) = target_window(state, peer_id) {
                let _ = state.platform.windows.resize(
                    &target,
                    webdock_platform::Size {
                        w: w as f64,
                        h: h as f64,
                    },
                );
            }
        }
    }
    Ok(())
}

fn claim(state: &AppState, peer_id: u64, ip: &str, kind: InputKind) -> bool {
    match state.seat.acquire(peer_id, kind, ip) {
        SeatResult::Allowed => true,
        SeatResult::Busy { who } => {
            if state.seat.should_notify_busy(peer_id) {
                if let Ok(s) = (ServerMessage::InputBusy {
                    message: format!("다른 곳에서 입력 중 ({who}) — 잠시 후 다시 시도"),
                    who,
                })
                .to_json()
                {
                    state.send_text(peer_id, s);
                }
            }
            false
        }
    }
}

fn target_window(state: &AppState, peer_id: u64) -> Option<WindowRef> {
    let id = state.viewing_of(peer_id)?;
    Some(WindowRef {
        id: RouteId(id),
        pid: 0,
    })
}

/// Start capture if not already marked streaming.
fn ensure_stream(state: SharedState, route: RouteId) {
    let route_i = route.as_i64();
    let Some(gen) = state.mark_streaming(route_i) else {
        return;
    };
    let target = AppState::capture_target(route_i);
    let cfg = state.stream_cfg.lock().clone();
    tokio::spawn(async move {
        let rx = match state.platform.capture.start_stream(target, cfg).await {
            Ok(rx) => rx,
            Err(e) => {
                warn!(error = %e, "start_stream failed — check Screen Recording permission");
                state.unmark_streaming(route_i, gen);
                return;
            }
        };
        let mut rx = rx;
        while let Some(frame) = rx.recv().await {
            state
                .fanout_frame_async(
                    route_i,
                    frame.pts_us,
                    frame.bytes,
                    frame.format,
                    frame.keyframe,
                    frame.h264_avcc,
                    frame.h264_codec,
                    frame.width,
                    frame.height,
                )
                .await;
        }
        // Only unmark if we are still the active generation (restart-safe).
        state.unmark_streaming(route_i, gen);
    });
}

/// Stop and re-start capture for a route (format / fps / window change).
fn restart_stream(state: SharedState, route: RouteId) {
    let route_i = route.as_i64();
    let target = AppState::capture_target(route_i);
    state.platform.capture.stop_stream(target);
    state.clear_streaming(route_i);
    let still = state.viewing_of_any(route_i);
    if still {
        ensure_stream(state, route);
    }
}
