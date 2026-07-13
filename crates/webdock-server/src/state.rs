//! Shared server state. **One mutex** owns peer maps to avoid ABBA deadlocks.

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use tokio::sync::mpsc;
use webdock_core::{tuning, AppConfig, InputSeat};
use webdock_encoder::{pack_h264_frame, pack_jpeg_frame, pack_png_frame};
use webdock_platform::{CaptureTarget, PlatformServices, StreamConfig};
use webdock_protocol::{ClientInfo, H264Config, MetricsPayload, ServerMessage, WindowInfo};

use crate::peer::PeerOutbound;

pub type PeerId = u64;

/// All peer-related maps under a single lock (prevents viewing↔peers ABBA).
struct PeerHub {
    peers: HashMap<PeerId, PeerOutbound>,
    viewing: HashMap<PeerId, i64>,
    /// Routes with an active capture task + generation (restart-safe unmark).
    streaming: HashMap<i64, u64>,
}

pub struct AppState {
    pub config: Mutex<AppConfig>,
    pub platform: PlatformServices,
    pub seat: InputSeat,
    next_id: AtomicU64,
    stream_gen: AtomicU64,
    hub: Mutex<PeerHub>,
    /// Shared stream quality settings.
    pub stream_cfg: Mutex<StreamConfig>,
    /// Optional filesystem WebUI root (else rust-embed).
    webui_dir: Option<PathBuf>,
    /// Login attempts: ip → timestamps (for rate limit).
    login_attempts: Mutex<HashMap<String, VecDeque<Instant>>>,
}

impl AppState {
    pub fn new(config: AppConfig, platform: PlatformServices, webui_dir: Option<PathBuf>) -> Self {
        Self {
            config: Mutex::new(config),
            platform,
            seat: InputSeat::new(),
            next_id: AtomicU64::new(1),
            stream_gen: AtomicU64::new(1),
            hub: Mutex::new(PeerHub {
                peers: HashMap::new(),
                viewing: HashMap::new(),
                streaming: HashMap::new(),
            }),
            stream_cfg: Mutex::new(StreamConfig::default()),
            webui_dir,
            login_attempts: Mutex::new(HashMap::new()),
        }
    }

    /// Max 10 login attempts per IP per 60 seconds.
    pub fn login_rate_check(&self, ip: &str) -> bool {
        const MAX: usize = 10;
        const WINDOW: Duration = Duration::from_secs(60);
        let mut map = self.login_attempts.lock();
        let now = Instant::now();
        let q = map.entry(ip.to_string()).or_default();
        while let Some(t) = q.front() {
            if now.duration_since(*t) > WINDOW {
                q.pop_front();
            } else {
                break;
            }
        }
        if q.len() >= MAX {
            return false;
        }
        q.push_back(now);
        true
    }

    /// True if `pid` appears in the current window list (or is 0).
    pub fn pid_in_window_list(&self, pid: i32) -> bool {
        if pid <= 0 {
            return false;
        }
        self.platform
            .capture
            .list_windows()
            .ok()
            .map(|list| list.iter().any(|w| w.pid == pid))
            .unwrap_or(false)
    }

    /// True if route id is currently listed (window or display).
    pub fn route_in_window_list(&self, route: i64) -> bool {
        self.platform
            .capture
            .list_windows()
            .ok()
            .map(|list| list.iter().any(|w| w.id.as_i64() == route))
            .unwrap_or(false)
    }

    pub fn webui_dir(&self) -> Option<&std::path::Path> {
        self.webui_dir.as_deref()
    }

    pub fn alloc_peer_id(&self) -> PeerId {
        self.next_id.fetch_add(1, Ordering::Relaxed)
    }

    pub fn register_peer(
        &self,
        id: PeerId,
        ctrl: mpsc::Sender<String>,
        video: mpsc::Sender<Vec<u8>>,
        ip: String,
    ) {
        self.hub
            .lock()
            .peers
            .insert(id, PeerOutbound::new(ctrl, video, ip));
        // Keep display awake while any remote session is connected.
        let _ = self.platform.power.keep_awake(true);
        let _ = self.platform.power.wake_display();
    }

    /// Unregister peer; returns previous viewing route if any (for stream release).
    pub fn unregister_peer(&self, id: PeerId) -> Option<i64> {
        let mut hub = self.hub.lock();
        hub.peers.remove(&id);
        let prev = hub.viewing.remove(&id);
        self.seat.release(id);
        drop(hub);
        if let Some(route) = prev {
            self.release_stream_if_unused(route);
        }
        // Pair with keep_awake(true) on register (refcounted).
        let _ = self.platform.power.keep_awake(false);
        self.broadcast_clients();
        prev
    }

    pub fn set_clip_auto(&self, id: PeerId, on: bool) {
        let hub = self.hub.lock();
        if let Some(p) = hub.peers.get(&id) {
            p.set_clip_auto(on);
        }
    }

    pub fn peer_clip_auto(&self, id: PeerId) -> bool {
        self.hub
            .lock()
            .peers
            .get(&id)
            .map(|p| p.clip_auto())
            .unwrap_or(false)
    }

    pub fn push_clipboard_to(&self, id: PeerId, value: String, force: bool) {
        if let Ok(s) = (ServerMessage::Clipboard {
            empty: value.is_empty(),
            force,
            value,
        })
        .to_json()
        {
            self.send_text(id, s);
        }
    }

    /// Set viewing route; returns previous route if changed.
    pub fn set_viewing(&self, id: PeerId, route: i64) -> Option<i64> {
        let mut hub = self.hub.lock();
        let prev = hub.viewing.insert(id, route);
        drop(hub);
        if let Some(prev) = prev {
            if prev != route {
                self.release_stream_if_unused(prev);
            }
        }
        prev
    }

    pub fn viewing_of(&self, id: PeerId) -> Option<i64> {
        self.hub.lock().viewing.get(&id).copied()
    }

    /// Returns `Some(generation)` if a new capture task should start.
    pub fn mark_streaming(&self, route: i64) -> Option<u64> {
        let mut hub = self.hub.lock();
        if hub.streaming.contains_key(&route) {
            return None;
        }
        let gen = self.stream_gen.fetch_add(1, Ordering::Relaxed);
        hub.streaming.insert(route, gen);
        Some(gen)
    }

    /// Only clear if generation still matches (ignore stale task exit after restart).
    pub fn unmark_streaming(&self, route: i64, gen: u64) {
        let mut hub = self.hub.lock();
        if hub.streaming.get(&route).copied() == Some(gen) {
            hub.streaming.remove(&route);
        }
    }

    /// Force-clear streaming marker (used by restart_stream before re-spawn).
    pub fn clear_streaming(&self, route: i64) {
        self.hub.lock().streaming.remove(&route);
    }

    pub fn release_stream_if_unused(&self, route: i64) {
        let mut hub = self.hub.lock();
        let still = hub.viewing.values().any(|&r| r == route);
        if still {
            return;
        }
        hub.streaming.remove(&route);
        drop(hub);
        let target = route_to_target(route);
        self.platform.capture.stop_stream(target);
    }

    pub fn capture_target(route: i64) -> CaptureTarget {
        route_to_target(route)
    }

    pub fn send_text(&self, id: PeerId, text: String) {
        let hub = self.hub.lock();
        if let Some(p) = hub.peers.get(&id) {
            // Control: try full queue, never steal video capacity.
            let _ = p.ctrl.try_send(text);
        }
    }

    pub fn broadcast_text(&self, text: &str) {
        let hub = self.hub.lock();
        for p in hub.peers.values() {
            let _ = p.ctrl.try_send(text.to_string());
        }
    }

    pub fn list_windows_json(&self) -> String {
        let list = self.platform.capture.list_windows().unwrap_or_default();
        ServerMessage::Windows { list }
            .to_json()
            .unwrap_or_else(|_| r#"{"type":"windows","list":[]}"#.into())
    }

    pub fn push_windows_to(&self, id: PeerId) {
        self.send_text(id, self.list_windows_json());
    }

    pub fn broadcast_clients(&self) {
        // Resolve titles outside the peer lock (capture can be slow).
        let windows = self.platform.capture.list_windows().unwrap_or_default();
        let list: Vec<ClientInfo> = {
            let hub = self.hub.lock();
            hub.peers
                .iter()
                .map(|(id, peer)| {
                    let win = hub.viewing.get(id).copied();
                    let viewing = win.and_then(|rid| {
                        windows.iter().find(|w| w.id.as_i64() == rid).map(|w| {
                            if w.title.is_empty() {
                                w.name.clone()
                            } else {
                                format!("{} — {}", w.name, w.title)
                            }
                        })
                    });
                    ClientInfo {
                        id: id.to_string(),
                        ip: peer.ip.clone(),
                        viewing,
                        window_id: win.map(webdock_protocol::RouteId),
                    }
                })
                .collect()
        };
        if let Ok(s) = (ServerMessage::Clients { list }).to_json() {
            self.broadcast_text(&s);
        }
    }

    pub fn broadcast_metrics(&self) {
        let m = self.platform.metrics.sample();
        let msg = ServerMessage::Metrics {
            payload: MetricsPayload {
                cpu: Some(m.cpu),
                ram: Some(m.ram_pct),
                disk: Some(m.disk_pct),
                ram_used_gb: Some(m.ram_used_gb),
                ram_total_gb: Some(m.ram_total_gb),
                disk_used_gb: Some(m.disk_used_gb),
                disk_total_gb: Some(m.disk_total_gb),
            },
        };
        if let Ok(s) = msg.to_json() {
            self.broadcast_text(&s);
        }
    }

    pub fn hello_json(&self) -> String {
        ServerMessage::Hello {
            version: webdock_protocol::PROTOCOL_VERSION,
            capabilities: self.platform.ime.capabilities(),
            platform: self.platform.platform_name.to_string(),
        }
        .to_json()
        .unwrap_or_else(|_| {
            r#"{"type":"hello","version":1,"capabilities":[],"platform":"?"}"#.into()
        })
    }

    /// Fan out a captured frame to peers viewing `route_id`.
    ///
    /// **H.264 keyframes + h264config** are delivered with a short await so a full
    /// video queue cannot permanently freeze the client (stuck on `h264WaitingKey`).
    pub async fn fanout_frame_async(
        &self,
        route_id: i64,
        pts_us: i64,
        bytes: Vec<u8>,
        format: webdock_platform::PixelFormat,
        keyframe: bool,
        h264_avcc: Option<Vec<u8>>,
        h264_codec: Option<String>,
        width: u32,
        height: u32,
    ) {
        if let Some(avcc) = h264_avcc.as_ref() {
            let b64 = {
                use base64::{engine::general_purpose::STANDARD, Engine};
                STANDARD.encode(avcc)
            };
            if let Ok(s) = (ServerMessage::H264Config {
                config: H264Config {
                    codec: h264_codec.as_deref().unwrap_or("avc1.42E01E").to_string(),
                    width,
                    height,
                    description: Some(b64),
                },
            })
            .to_json()
            {
                let peers: Vec<_> = {
                    let hub = self.hub.lock();
                    hub.peers
                        .iter()
                        .filter(|(id, _)| hub.viewing.get(id).copied() == Some(route_id))
                        .map(|(_, p)| p.ctrl.clone())
                        .collect()
                };
                for ctrl in peers {
                    // Config must not be dropped — client cannot decode without it.
                    let _ = tokio::time::timeout(
                        std::time::Duration::from_millis(200),
                        ctrl.send(s.clone()),
                    )
                    .await;
                }
            }
        }

        let packed = match format {
            webdock_platform::PixelFormat::Jpeg => pack_jpeg_frame(pts_us, &bytes),
            webdock_platform::PixelFormat::Png => pack_png_frame(pts_us, &bytes),
            webdock_platform::PixelFormat::H264 => pack_h264_frame(pts_us, keyframe, &bytes),
            _ => return,
        };

        let peers: Vec<_> = {
            let hub = self.hub.lock();
            hub.peers
                .iter()
                .filter(|(id, _)| hub.viewing.get(id).copied() == Some(route_id))
                .map(|(_, p)| p.video.clone())
                .collect()
        };

        for video in peers {
            if keyframe
                || matches!(
                    format,
                    webdock_platform::PixelFormat::Jpeg | webdock_platform::PixelFormat::Png
                )
            {
                // Key / still: wait briefly so the client can resync.
                let _ = tokio::time::timeout(
                    std::time::Duration::from_millis(80),
                    video.send(packed.clone()),
                )
                .await;
            } else if video.try_send(packed.clone()).is_err() {
                // Delta under backpressure: drop (prefer live over lag).
            }
        }
    }

    /// Sync helper for non-async call sites (metrics etc. unused).
    pub fn fanout_frame(
        &self,
        route_id: i64,
        pts_us: i64,
        bytes: &[u8],
        format: webdock_platform::PixelFormat,
        keyframe: bool,
        h264_avcc: Option<&[u8]>,
        h264_codec: Option<&str>,
        width: u32,
        height: u32,
    ) {
        // Fire-and-forget via try_send only (legacy). Prefer fanout_frame_async.
        if let Some(avcc) = h264_avcc {
            let b64 = {
                use base64::{engine::general_purpose::STANDARD, Engine};
                STANDARD.encode(avcc)
            };
            if let Ok(s) = (ServerMessage::H264Config {
                config: H264Config {
                    codec: h264_codec.unwrap_or("avc1.42E01E").to_string(),
                    width,
                    height,
                    description: Some(b64),
                },
            })
            .to_json()
            {
                let hub = self.hub.lock();
                for (id, peer) in hub.peers.iter() {
                    if hub.viewing.get(id).copied() == Some(route_id) {
                        let _ = peer.ctrl.try_send(s.clone());
                    }
                }
            }
        }
        let packed = match format {
            webdock_platform::PixelFormat::Jpeg => pack_jpeg_frame(pts_us, bytes),
            webdock_platform::PixelFormat::Png => pack_png_frame(pts_us, bytes),
            webdock_platform::PixelFormat::H264 => pack_h264_frame(pts_us, keyframe, bytes),
            _ => return,
        };
        let hub = self.hub.lock();
        for (id, peer) in hub.peers.iter() {
            if hub.viewing.get(id).copied() == Some(route_id) {
                let _ = peer.video.try_send(packed.clone());
            }
        }
    }

    pub fn viewing_of_any(&self, route: i64) -> bool {
        self.hub.lock().viewing.values().any(|&r| r == route)
    }

    pub fn window_list(&self) -> Vec<WindowInfo> {
        self.platform.capture.list_windows().unwrap_or_default()
    }

    pub fn ctrl_capacity() -> usize {
        tuning::CTRL_QUEUE_DEPTH
    }

    pub fn video_capacity() -> usize {
        tuning::FRAME_QUEUE_DEPTH
    }
}

fn route_to_target(route: i64) -> CaptureTarget {
    use webdock_platform::{display_id_from_route, is_display_route};
    use webdock_protocol::RouteId;
    let id = RouteId(route);
    if is_display_route(id) {
        CaptureTarget::Display(display_id_from_route(id).unwrap_or(0))
    } else {
        CaptureTarget::Window(id)
    }
}

/// Shared state pointer used by axum.
pub type SharedState = Arc<AppState>;
