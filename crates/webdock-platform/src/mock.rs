//! Mock platform — **tests only**. Production uses [`crate::native`].

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use webdock_protocol::{AppInfo, RouteId, WindowInfo};

use crate::traits::*;
use crate::types::*;
use crate::PlatformServices;

pub struct MockPlatform;

impl MockPlatform {
    pub fn services() -> PlatformServices {
        PlatformServices {
            capture: Arc::new(MockCapture),
            input: Arc::new(MockInput),
            windows: Arc::new(MockWindows),
            apps: Arc::new(MockApps),
            metrics: Arc::new(MockMetrics),
            ime: Arc::new(MockIme),
            power: Arc::new(MockPower),
            platform_name: "mock",
        }
    }
}

struct MockCapture;
struct MockInput;
struct MockWindows;
struct MockApps;
struct MockMetrics;
struct MockIme;
struct MockPower;

#[async_trait]
impl CaptureBackend for MockCapture {
    fn list_windows(&self) -> Result<Vec<WindowInfo>, CaptureError> {
        Ok(vec![WindowInfo {
            id: RouteId(1),
            pid: std::process::id() as i32,
            name: "Mock".into(),
            title: "Mock Window".into(),
            path: None,
            w: Some(800),
            h: Some(600),
            icon: None,
            icon_key: Some("mock".into()),
        }])
    }

    fn list_displays(&self) -> Result<Vec<DisplayInfo>, CaptureError> {
        Ok(vec![DisplayInfo {
            id: 1,
            name: "Mock Display".into(),
            width: 1920,
            height: 1080,
        }])
    }

    async fn start_stream(
        &self,
        _target: CaptureTarget,
        cfg: StreamConfig,
    ) -> Result<tokio::sync::mpsc::Receiver<RawFrame>, CaptureError> {
        let (tx, rx) = tokio::sync::mpsc::channel(2);
        let fps = cfg.fps.max(1);
        let period = std::time::Duration::from_millis(1000 / u64::from(fps));
        tokio::spawn(async move {
            loop {
                let pts = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_micros() as i64)
                    .unwrap_or(0);
                // Minimal JPEG SOI/EOI
                let jpeg = vec![0xFF, 0xD8, 0xFF, 0xD9];
                if tx
                    .send(RawFrame {
                        width: 2,
                        height: 2,
                        pts_us: pts,
                        bytes: jpeg,
                        format: PixelFormat::Jpeg,
                        keyframe: true,
                        h264_avcc: None,
                        h264_codec: None,
                    })
                    .await
                    .is_err()
                {
                    break;
                }
                tokio::time::sleep(period).await;
            }
        });
        Ok(rx)
    }

    fn stop_stream(&self, _target: CaptureTarget) {}
    fn request_keyframe(&self, _target: CaptureTarget) {}
    fn set_bitrate(&self, _target: CaptureTarget, _bitrate_bps: u32) {}
}

impl InputInjector for MockInput {
    fn mouse(&self, _ev: &MouseEvent, _t: Option<&WindowRef>) -> Result<(), InputError> {
        Ok(())
    }
    fn key(&self, _ev: &KeyEvent, _t: Option<&WindowRef>) -> Result<(), InputError> {
        Ok(())
    }
    fn text(&self, _s: &str, _r: u32, _t: Option<&WindowRef>) -> Result<(), InputError> {
        Ok(())
    }
    fn scroll(
        &self,
        _dx: f64,
        _dy: f64,
        _x: f64,
        _y: f64,
        _t: Option<&WindowRef>,
    ) -> Result<(), InputError> {
        Ok(())
    }
}

impl WindowControl for MockWindows {
    fn raise(&self, _w: &WindowRef) -> Result<(), WindowError> {
        Ok(())
    }
    fn resize(&self, _w: &WindowRef, _s: Size) -> Result<(), WindowError> {
        Ok(())
    }
    fn close(&self, _w: &WindowRef) -> Result<(), WindowError> {
        Ok(())
    }
    fn bounds(&self, _w: &WindowRef) -> Result<Rect, WindowError> {
        Ok(Rect {
            x: 0.0,
            y: 0.0,
            w: 800.0,
            h: 600.0,
        })
    }
}

impl AppCatalog for MockApps {
    fn list_apps(&self) -> Result<Vec<AppInfo>, PlatformError> {
        Ok(vec![])
    }
    fn launch(&self, _p: &str, _n: bool) -> Result<(), PlatformError> {
        Ok(())
    }
    fn quit_pid(&self, _p: i32) -> Result<(), PlatformError> {
        Ok(())
    }
}

impl MetricsProvider for MockMetrics {
    fn sample(&self) -> HostMetrics {
        HostMetrics {
            cpu: 12.0,
            ram_pct: 48.0,
            disk_pct: 60.0,
            ram_used_gb: 8.0,
            ram_total_gb: 16.0,
            disk_used_gb: 240.0,
            disk_total_gb: 500.0,
        }
    }
}

impl ImeControl for MockIme {
    fn current_korean(&self) -> Option<(bool, String)> {
        None
    }
    fn set_korean(&self, k: bool) -> Result<(bool, String), PlatformError> {
        Ok((k, "A".into()))
    }
    fn capabilities(&self) -> Vec<String> {
        vec!["mock".into()]
    }
}

impl PowerControl for MockPower {
    fn wake_display(&self) -> Result<(), PlatformError> {
        Ok(())
    }
    fn keep_awake(&self, _e: bool) -> Result<(), PlatformError> {
        Ok(())
    }
}
