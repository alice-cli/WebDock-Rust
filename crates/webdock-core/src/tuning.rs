//! Tunable constants ported from Swift WebDock (with rationale comments).

/// Focus/raise cache TTL — skip redundant AX raises within this window.
pub const FOCUS_CACHE_SECS: f64 = 0.45;

/// After pointer down, allow move/up from same peer even if seat would otherwise block.
pub const DRAG_GRACE_SECS: f64 = 2.0;

/// Per-peer outbound **video** queue depth before drop (backpressure).
/// Depth 2 dropped H.264 keyframes under load → permanent freeze (client waits for key).
pub const FRAME_QUEUE_DEPTH: usize = 12;

/// Control (JSON) outbound queue depth.
pub const CTRL_QUEUE_DEPTH: usize = 64;

/// Adaptive bitrate ladder (Mbps) for H.264 under network pressure 0..=3.
/// Softer steps so software OpenH264 can keep realtime (no freeze on recreate).
pub const BITRATE_LADDER_MBPS: [f64; 4] = [6.0, 4.0, 2.8, 1.8];

/// Broadcast (Live) preset target bitrate.
pub const BROADCAST_BITRATE_BPS: u32 = 6_000_000;

/// Broadcast max capture width.
/// VideoToolbox handles 1920 fine; software OpenH264 prefers ≤1280 — session
/// layer may clamp further when soft backend is active.
pub const BROADCAST_MAX_WIDTH: u32 = 1920;

/// Max capture width when H.264 runs on the **software** OpenH264 backend
/// (Windows/Linux — no HW encoder wired yet). 1920 software encode misses the
/// frame period on typical CPUs → stream looks frozen.
pub const H264_SW_MAX_WIDTH: u32 = 1280;

/// Effective H.264 width cap for this host (HW on macOS, SW elsewhere).
pub fn h264_max_width() -> u32 {
    if cfg!(target_os = "macos") {
        BROADCAST_MAX_WIDTH
    } else {
        H264_SW_MAX_WIDTH
    }
}

/// Min interval between live H.264 encoder recreates (bitrate adapt).
pub const H264_RECONFIG_MIN_SECS: f64 = 4.0;

/// Default JPEG quality (0.2..=1.0).
pub const DEFAULT_JPEG_QUALITY: f64 = 0.92;

/// Default capture FPS target for JPEG MVP.
pub const DEFAULT_JPEG_FPS: u32 = 20;

/// Default HTTP/WS port for **WebRust** (8080 is commonly used by Swift WebDock).
pub const DEFAULT_PORT: u16 = 8090;

/// Rate-limit for inputBusy notifications (seconds).
pub const INPUT_BUSY_NOTIFY_INTERVAL_SECS: f64 = 1.0;

/// How long an exclusive input seat is held without activity (matches Swift ~0.85s).
pub const INPUT_SEAT_TTL_SECS: f64 = 0.85;
