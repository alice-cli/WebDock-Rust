use serde::{Deserialize, Serialize};
use webdock_protocol::RouteId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Size {
    pub w: f64,
    pub h: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureTarget {
    Window(RouteId),
    Display(u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StreamFormat {
    #[default]
    Jpeg,
    Png,
    H264,
}

#[derive(Debug, Clone)]
pub struct StreamConfig {
    pub fps: u32,
    pub max_width: u32,
    pub jpeg_quality: f64,
    pub format: StreamFormat,
    /// Target bitrate for H.264 (bits/s).
    pub bitrate_bps: u32,
}

impl Default for StreamConfig {
    fn default() -> Self {
        Self {
            fps: 20,
            max_width: 1920,
            jpeg_quality: 0.92,
            format: StreamFormat::Jpeg,
            bitrate_bps: 2_800_000,
        }
    }
}

/// Encoded or raw frame from capture backend.
#[derive(Debug, Clone)]
pub struct RawFrame {
    pub width: u32,
    pub height: u32,
    pub pts_us: i64,
    pub bytes: Vec<u8>,
    pub format: PixelFormat,
    pub keyframe: bool,
    /// When set, server should emit `h264config` before this sample.
    pub h264_avcc: Option<Vec<u8>>,
    pub h264_codec: Option<String>,
}

impl Default for RawFrame {
    fn default() -> Self {
        Self {
            width: 0,
            height: 0,
            pts_us: 0,
            bytes: Vec::new(),
            format: PixelFormat::Jpeg,
            keyframe: true,
            h264_avcc: None,
            h264_codec: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    Bgra8,
    Rgba8,
    Jpeg,
    Png,
    H264,
}

#[derive(Debug, Clone)]
pub struct MouseEvent {
    pub phase: MousePhase,
    pub x: f64,
    pub y: f64,
    pub button: i32,
    pub click_count: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MousePhase {
    Down,
    Move,
    Up,
}

#[derive(Debug, Clone)]
pub struct KeyEvent {
    pub code: String,
    pub meta: bool,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
}

#[derive(Debug, Clone)]
pub struct WindowRef {
    pub id: RouteId,
    pub pid: i32,
}

#[derive(Debug, Clone)]
pub struct HostMetrics {
    pub cpu: f64,
    pub ram_pct: f64,
    pub disk_pct: f64,
    pub ram_used_gb: f64,
    pub ram_total_gb: f64,
    pub disk_used_gb: f64,
    pub disk_total_gb: f64,
}
