use async_trait::async_trait;
use thiserror::Error;
use webdock_protocol::{AppInfo, WindowInfo};

use crate::types::*;

#[derive(Debug, Error)]
pub enum CaptureError {
    #[error("not supported: {0}")]
    NotSupported(&'static str),
    #[error("target not found")]
    NotFound,
    #[error("{0}")]
    Other(String),
}

#[derive(Debug, Error)]
pub enum InputError {
    #[error("not supported: {0}")]
    NotSupported(&'static str),
    #[error("{0}")]
    Other(String),
}

#[derive(Debug, Error)]
pub enum WindowError {
    #[error("not found")]
    NotFound,
    #[error("{0}")]
    Other(String),
}

#[derive(Debug, Error)]
pub enum PlatformError {
    #[error("{0}")]
    Other(String),
}

/// Window/display enumeration + frame stream.
#[async_trait]
pub trait CaptureBackend: Send + Sync {
    fn list_windows(&self) -> Result<Vec<WindowInfo>, CaptureError>;
    fn list_displays(&self) -> Result<Vec<DisplayInfo>, CaptureError>;

    /// Start streaming frames for `target`. Implementations may use polling (MVP).
    async fn start_stream(
        &self,
        target: CaptureTarget,
        cfg: StreamConfig,
    ) -> Result<tokio::sync::mpsc::Receiver<RawFrame>, CaptureError>;

    fn stop_stream(&self, target: CaptureTarget);
    fn request_keyframe(&self, target: CaptureTarget);
    /// Live H.264 bitrate tweak (bps). No-op if stream not running.
    fn set_bitrate(&self, target: CaptureTarget, bitrate_bps: u32) {
        let _ = (target, bitrate_bps);
    }
}

#[derive(Debug, Clone)]
pub struct DisplayInfo {
    pub id: u32,
    pub name: String,
    pub width: u32,
    pub height: u32,
}

pub trait InputInjector: Send + Sync {
    fn mouse(&self, ev: &MouseEvent, target: Option<&WindowRef>) -> Result<(), InputError>;
    fn key(&self, ev: &KeyEvent, target: Option<&WindowRef>) -> Result<(), InputError>;
    fn text(&self, s: &str, replace: u32, target: Option<&WindowRef>) -> Result<(), InputError>;
    fn scroll(
        &self,
        dx: f64,
        dy: f64,
        x: f64,
        y: f64,
        target: Option<&WindowRef>,
    ) -> Result<(), InputError>;
}

pub trait WindowControl: Send + Sync {
    fn raise(&self, w: &WindowRef) -> Result<(), WindowError>;
    fn resize(&self, w: &WindowRef, size: Size) -> Result<(), WindowError>;
    fn close(&self, w: &WindowRef) -> Result<(), WindowError>;
    fn bounds(&self, w: &WindowRef) -> Result<Rect, WindowError>;
}

pub trait AppCatalog: Send + Sync {
    fn list_apps(&self) -> Result<Vec<AppInfo>, PlatformError>;
    fn launch(&self, path: &str, new_instance: bool) -> Result<(), PlatformError>;
    fn quit_pid(&self, pid: i32) -> Result<(), PlatformError>;
}

pub trait MetricsProvider: Send + Sync {
    fn sample(&self) -> HostMetrics;
}

pub trait ImeControl: Send + Sync {
    fn current_korean(&self) -> Option<(bool, String)>;
    fn set_korean(&self, korean: bool) -> Result<(bool, String), PlatformError>;
    fn capabilities(&self) -> Vec<String>;
}

pub trait PowerControl: Send + Sync {
    fn wake_display(&self) -> Result<(), PlatformError>;
    fn keep_awake(&self, enable: bool) -> Result<(), PlatformError>;
}
