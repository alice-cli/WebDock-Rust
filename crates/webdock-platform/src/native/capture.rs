//! Real window/display capture via `xcap` + JPEG / H.264 encode.

use std::collections::HashMap;
use std::io::Cursor;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use image::codecs::jpeg::JpegEncoder;
use image::{imageops, ColorType, ImageEncoder};
use parking_lot::Mutex;
use tracing::{debug, info, warn};
use webdock_encoder::H264Session;
use webdock_protocol::{RouteId, WindowInfo};
use xcap::{Monitor, Window};

use crate::route::{
    display_id_from_route, display_route, is_display_route, window_id_from_route, window_route,
};
use crate::traits::*;
use crate::types::*;

struct StreamCtrl {
    cancel: Arc<AtomicBool>,
    force_key: Arc<AtomicBool>,
    /// Live H.264 target bitrate (bps); encoder recreated when this changes.
    bitrate: Arc<AtomicU32>,
    /// Unix-ms of last encoder recreate (debounce adaptive bitrate thrash).
    last_reconfig_ms: Arc<AtomicU64>,
}

pub struct NativeCapture {
    /// route_id → active stream controls
    active: Mutex<HashMap<i64, StreamCtrl>>,
}

impl NativeCapture {
    pub fn new() -> Self {
        Self {
            active: Mutex::new(HashMap::new()),
        }
    }
}

impl Default for NativeCapture {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl CaptureBackend for NativeCapture {
    fn list_windows(&self) -> Result<Vec<WindowInfo>, CaptureError> {
        let mut out = Vec::new();

        let locked = super::session::is_screen_locked();

        // Full displays first (full-screen remote).
        if let Ok(monitors) = Monitor::all() {
            for m in monitors {
                let id = m.id().unwrap_or(0);
                let mon_name = m
                    .friendly_name()
                    .or_else(|_| m.name())
                    .unwrap_or_else(|_| format!("Display {id}"));
                let w = m.width().ok().map(|v| v as i32);
                let h = m.height().ok().map(|v| v as i32);
                let icon_key = "system:display".to_string();
                let icon = super::icons::data_url_for_key(&icon_key);
                let (name, title) = if locked {
                    ("잠금 화면".into(), format!("화면 잠김 · {mon_name}"))
                } else {
                    ("전체 화면".into(), mon_name)
                };
                out.push(WindowInfo {
                    id: display_route(id),
                    pid: 0,
                    name,
                    title,
                    path: None,
                    w,
                    h,
                    icon,
                    icon_key: Some(icon_key),
                });
            }
        }

        match Window::all() {
            Ok(windows) => {
                let total = windows.len();
                let mut kept = 0usize;
                for w in windows {
                    let minimized = w.is_minimized().unwrap_or(false);
                    if minimized {
                        continue;
                    }
                    let width = w.width().unwrap_or(0);
                    let height = w.height().unwrap_or(0);
                    // Filter tiny / invisible chrome (matches Swift size>40-ish intent).
                    if width < 40 || height < 40 {
                        continue;
                    }
                    let id = match w.id() {
                        Ok(id) => id,
                        Err(_) => continue,
                    };
                    let name = w.app_name().unwrap_or_else(|_| "?".into());
                    let title = w.title().unwrap_or_default();
                    // Skip empty untitled tool windows when app also empty.
                    if title.is_empty() && (name.is_empty() || name == "?") {
                        continue;
                    }
                    // System chrome (Dock, Wallpaper, …) — same blocklist as Swift WebDock.
                    if is_system_app(&name) {
                        continue;
                    }
                    let pid = w.pid().unwrap_or(0) as i32;
                    // Don't list our own WebRust window.
                    if pid > 0 && pid == std::process::id() as i32 {
                        continue;
                    }
                    let path = super::icons::path_for_pid(pid);
                    let icon_key = path.clone().unwrap_or_else(|| format!("pid:{pid}"));
                    let icon = super::icons::data_url_for_key(&icon_key);
                    out.push(WindowInfo {
                        id: window_route(id),
                        pid,
                        name,
                        title,
                        path,
                        w: Some(width as i32),
                        h: Some(height as i32),
                        icon,
                        icon_key: Some(icon_key),
                    });
                    kept += 1;
                }
                if total > 0 && kept == 0 {
                    warn!(
                        total,
                        "all windows filtered (minimized/small); displays still available"
                    );
                }
                if total == 0 {
                    warn!(
                        "Window::all() returned 0 — grant Screen Recording to this process \
                         (System Settings → Privacy & Security → Screen Recording)"
                    );
                }
            }
            Err(e) => {
                warn!(
                    error = %e,
                    "window enumeration failed — grant Screen Recording permission"
                );
            }
        }

        Ok(out)
    }

    fn list_displays(&self) -> Result<Vec<DisplayInfo>, CaptureError> {
        let monitors = Monitor::all().map_err(|e| CaptureError::Other(e.to_string()))?;
        let mut out = Vec::new();
        for m in monitors {
            let id = m.id().unwrap_or(0);
            out.push(DisplayInfo {
                id,
                name: m
                    .friendly_name()
                    .or_else(|_| m.name())
                    .unwrap_or_else(|_| format!("Display {id}")),
                width: m.width().unwrap_or(0),
                height: m.height().unwrap_or(0),
            });
        }
        Ok(out)
    }

    async fn start_stream(
        &self,
        target: CaptureTarget,
        cfg: StreamConfig,
    ) -> Result<tokio::sync::mpsc::Receiver<RawFrame>, CaptureError> {
        let route = route_from_target(target);
        // Cancel previous loop for this route.
        self.stop_stream(target);

        let cancel = Arc::new(AtomicBool::new(false));
        let force_key = Arc::new(AtomicBool::new(true));
        let bitrate_live = Arc::new(AtomicU32::new(cfg.bitrate_bps.max(400_000)));
        let last_reconfig_ms = Arc::new(AtomicU64::new(0));
        self.active.lock().insert(
            route.as_i64(),
            StreamCtrl {
                cancel: cancel.clone(),
                force_key: force_key.clone(),
                bitrate: bitrate_live.clone(),
                last_reconfig_ms: last_reconfig_ms.clone(),
            },
        );

        // Deeper queue: VT is fast but fan-out / WS can lag without dropping video.
        let (tx, rx) = tokio::sync::mpsc::channel(8);
        // H.264: allow up to 30fps; HW backends keep up, SW may drop via try_send.
        let fps = if cfg.format == StreamFormat::H264 {
            cfg.fps.max(1).min(30)
        } else {
            cfg.fps.max(1).min(60)
        };
        let period = Duration::from_millis(1000 / u64::from(fps));
        let max_width = cfg.max_width.max(320);
        let quality = ((cfg.jpeg_quality.clamp(0.2, 1.0)) * 100.0) as u8;
        let format = cfg.format;

        info!(
            route = route.as_i64(),
            ?format,
            fps,
            max_width,
            bitrate = bitrate_live.load(Ordering::Relaxed),
            "start capture stream"
        );

        tokio::task::spawn_blocking(move || {
            let mut last = Instant::now()
                .checked_sub(period)
                .unwrap_or_else(Instant::now);
            let mut h264: Option<H264Session> = None;
            let mut empty_streak: u32 = 0;
            // Relative pts so WebCodecs timestamps stay sane.
            let stream_start = Instant::now();

            while !cancel.load(Ordering::Relaxed) {
                let elapsed = last.elapsed();
                if elapsed < period {
                    std::thread::sleep(period - elapsed);
                }
                // If previous iteration overran, don't pile up — start now.
                last = Instant::now();

                let mut captured = match capture_rgba(target, max_width) {
                    Ok(v) => v,
                    Err(e) => {
                        // Window closed / gone → end stream so the blocking thread exits.
                        if matches!(e, CaptureError::NotFound) {
                            info!(
                                route = route.as_i64(),
                                "capture target gone — ending stream"
                            );
                            break;
                        }
                        warn!(error = %e, "capture frame failed");
                        std::thread::sleep(Duration::from_millis(200));
                        continue;
                    }
                };

                // H.264 needs even dimensions.
                if format == StreamFormat::H264 {
                    let w = captured.width() & !1;
                    let h = captured.height() & !1;
                    if w < 2 || h < 2 {
                        continue;
                    }
                    if w != captured.width() || h != captured.height() {
                        captured = imageops::crop_imm(&captured, 0, 0, w, h).to_image();
                    }
                }

                let pts = stream_start.elapsed().as_micros() as i64;

                let frame = match format {
                    StreamFormat::H264 => {
                        let w = captured.width();
                        let h = captured.height();
                        let want_br = bitrate_live.load(Ordering::Relaxed).max(400_000);
                        let now_ms = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .map(|d| d.as_millis() as u64)
                            .unwrap_or(0);
                        let last_rc = last_reconfig_ms.load(Ordering::Relaxed);
                        let reconfig_ok = last_rc == 0 || now_ms.saturating_sub(last_rc) >= 4_000;
                        let need_new = match h264.as_ref() {
                            None => true,
                            Some(e) => {
                                e.dimensions() != (w, h)
                                    || (reconfig_ok
                                        && e.bitrate_bps().abs_diff(want_br) > want_br / 4)
                            }
                        };
                        if need_new {
                            match H264Session::open(w, h, fps, want_br) {
                                Ok(enc) => {
                                    info!(
                                        w,
                                        h,
                                        bitrate = want_br,
                                        backend = enc.backend_name(),
                                        "h264 encoder (re)created"
                                    );
                                    last_reconfig_ms.store(now_ms, Ordering::Relaxed);
                                    h264 = Some(enc);
                                    empty_streak = 0;
                                }
                                Err(e) => {
                                    warn!(error = %e, "h264 encoder init failed — falling back JPEG");
                                    if let Ok((width, height, jpeg)) =
                                        encode_jpeg(&captured, quality)
                                    {
                                        let f = RawFrame {
                                            width,
                                            height,
                                            pts_us: pts,
                                            bytes: jpeg,
                                            format: PixelFormat::Jpeg,
                                            keyframe: true,
                                            h264_avcc: None,
                                            h264_codec: None,
                                        };
                                        // Non-blocking: never stall the capture loop.
                                        let _ = tx.try_send(f);
                                    }
                                    continue;
                                }
                            }
                        }
                        let enc = h264.as_mut().unwrap();
                        if force_key.swap(false, Ordering::Relaxed) {
                            enc.force_keyframe();
                        }
                        match enc.encode_rgba(captured.as_raw(), w, h, pts) {
                            Ok(out) => {
                                empty_streak = 0;
                                RawFrame {
                                    width: out.width,
                                    height: out.height,
                                    pts_us: out.pts_us,
                                    bytes: out.avcc_au,
                                    format: PixelFormat::H264,
                                    keyframe: out.keyframe,
                                    h264_avcc: out.avcc_config,
                                    h264_codec: Some(out.codec),
                                }
                            }
                            Err(e) => {
                                empty_streak = empty_streak.saturating_add(1);
                                if empty_streak >= 3 {
                                    // Unstick encoder: force key / recreate after streak of empty.
                                    warn!(error = %e, streak = empty_streak, "h264 empty streak — force key");
                                    enc.force_keyframe();
                                }
                                if empty_streak >= 15 {
                                    warn!("h264 recreate after empty streak");
                                    h264 = None;
                                    empty_streak = 0;
                                }
                                continue;
                            }
                        }
                    }
                    StreamFormat::Png => match encode_png(&captured) {
                        Ok((width, height, png)) => RawFrame {
                            width,
                            height,
                            pts_us: pts,
                            bytes: png,
                            format: PixelFormat::Png,
                            keyframe: true,
                            h264_avcc: None,
                            h264_codec: None,
                        },
                        Err(e) => {
                            warn!(error = %e, "png encode failed");
                            continue;
                        }
                    },
                    StreamFormat::Jpeg => match encode_jpeg(&captured, quality) {
                        Ok((width, height, jpeg)) => RawFrame {
                            width,
                            height,
                            pts_us: pts,
                            bytes: jpeg,
                            format: PixelFormat::Jpeg,
                            keyframe: true,
                            h264_avcc: None,
                            h264_codec: None,
                        },
                        Err(e) => {
                            warn!(error = %e, "jpeg encode failed");
                            continue;
                        }
                    },
                };

                // Prefer keyframes: if queue full, block briefly so H.264 can resync.
                let is_key = frame.keyframe;
                if is_key {
                    if tx.blocking_send(frame).is_err() {
                        debug!("capture stream consumer closed");
                        break;
                    }
                } else {
                    match tx.try_send(frame) {
                        Ok(()) => {}
                        Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {
                            debug!("capture delta dropped (queue full)");
                        }
                        Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {
                            debug!("capture stream consumer closed");
                            break;
                        }
                    }
                }
            }
            debug!(route = route.as_i64(), "capture loop ended");
        });

        Ok(rx)
    }

    fn stop_stream(&self, target: CaptureTarget) {
        let route = route_from_target(target).as_i64();
        if let Some(ctrl) = self.active.lock().remove(&route) {
            ctrl.cancel.store(true, Ordering::Relaxed);
        }
    }

    fn request_keyframe(&self, target: CaptureTarget) {
        let route = route_from_target(target).as_i64();
        if let Some(ctrl) = self.active.lock().get(&route) {
            ctrl.force_key.store(true, Ordering::Relaxed);
        }
    }

    fn set_bitrate(&self, target: CaptureTarget, bitrate_bps: u32) {
        let route = route_from_target(target).as_i64();
        let bps = bitrate_bps.max(400_000);
        if let Some(ctrl) = self.active.lock().get(&route) {
            let prev = ctrl.bitrate.swap(bps, Ordering::Relaxed);
            if prev != bps {
                debug!(route, bps, "h264 live bitrate");
                // Force keyframe after reconfigure so clients resync cleanly.
                ctrl.force_key.store(true, Ordering::Relaxed);
            }
        }
    }
}

fn route_from_target(target: CaptureTarget) -> RouteId {
    match target {
        CaptureTarget::Window(id) => id,
        CaptureTarget::Display(id) => display_route(id),
    }
}

/// Matches Swift `CaptureManager.systemAppBlocklist`.
fn is_system_app(name: &str) -> bool {
    matches!(
        name,
        "Dock"
            | "Window Server"
            | "WindowServer"
            | "Wallpaper"
            | "WallpaperAgent"
            | "Control Center"
            | "Notification Center"
            | "Spotlight"
            | "WindowManager"
            | "Menubar"
            | "Menu Bar"
            | "Screenshot"
            | "coreautha"
            | "universalaccessd"
            | "SystemUIServer"
            | "ControlStrip"
            | "loginwindow"
            | "UserNotificationCenter"
            | "TextInputMenuAgent"
            | "TextInputSwitcher"
            | "WebRust"
    ) || name.eq_ignore_ascii_case("Dock")
}

fn capture_rgba(target: CaptureTarget, max_width: u32) -> Result<image::RgbaImage, CaptureError> {
    let mut img = match target {
        CaptureTarget::Window(id) => {
            let wid = window_id_from_route(id).ok_or(CaptureError::NotFound)?;
            let win = find_window(wid).ok_or(CaptureError::NotFound)?;
            win.capture_image().map_err(|e| {
                CaptureError::Other(format!(
                    "window capture failed (grant Screen Recording permission?): {e}"
                ))
            })?
        }
        CaptureTarget::Display(mid) => {
            let mon = find_monitor(mid).ok_or(CaptureError::NotFound)?;
            mon.capture_image().map_err(|e| {
                CaptureError::Other(format!(
                    "display capture failed (grant Screen Recording permission?): {e}"
                ))
            })?
        }
    };

    if img.width() > max_width && img.width() > 0 {
        let scale = max_width as f32 / img.width() as f32;
        let nh = ((img.height() as f32) * scale).round().max(1.0) as u32;
        // Lanczos3 keeps UI text sharper than Triangle when downscaling Retina captures.
        img = imageops::resize(&img, max_width, nh, imageops::FilterType::Lanczos3);
    }

    Ok(img)
}

fn encode_jpeg(img: &image::RgbaImage, quality: u8) -> Result<(u32, u32, Vec<u8>), CaptureError> {
    let (w, h) = img.dimensions();
    let mut buf = Cursor::new(Vec::with_capacity((w * h) as usize / 4));
    let encoder = JpegEncoder::new_with_quality(&mut buf, quality);
    // JPEG encoder wants RGB8
    let rgb = image::DynamicImage::ImageRgba8(img.clone()).into_rgb8();
    encoder
        .write_image(rgb.as_raw(), w, h, ColorType::Rgb8.into())
        .map_err(|e| CaptureError::Other(e.to_string()))?;
    Ok((w, h, buf.into_inner()))
}

fn encode_png(img: &image::RgbaImage) -> Result<(u32, u32, Vec<u8>), CaptureError> {
    let (w, h) = img.dimensions();
    let mut buf = Cursor::new(Vec::with_capacity((w * h) as usize / 2));
    {
        let mut encoder = image::codecs::png::PngEncoder::new(&mut buf);
        encoder
            .write_image(img.as_raw(), w, h, ColorType::Rgba8.into())
            .map_err(|e| CaptureError::Other(e.to_string()))?;
    }
    Ok((w, h, buf.into_inner()))
}

pub fn find_window(id: u32) -> Option<Window> {
    let windows = Window::all().ok()?;
    windows.into_iter().find(|w| w.id().ok() == Some(id))
}

pub fn find_monitor(id: u32) -> Option<Monitor> {
    let monitors = Monitor::all().ok()?;
    monitors.into_iter().find(|m| m.id().ok() == Some(id))
}

/// Screen-space bounds for a route (window or display).
pub fn bounds_for_route(id: RouteId) -> Result<Rect, CaptureError> {
    if let Some(mid) = display_id_from_route(id) {
        let m = find_monitor(mid).ok_or(CaptureError::NotFound)?;
        return Ok(Rect {
            x: m.x().unwrap_or(0) as f64,
            y: m.y().unwrap_or(0) as f64,
            w: m.width().unwrap_or(0) as f64,
            h: m.height().unwrap_or(0) as f64,
        });
    }
    let wid = window_id_from_route(id).ok_or(CaptureError::NotFound)?;
    let w = find_window(wid).ok_or(CaptureError::NotFound)?;
    Ok(Rect {
        x: w.x().unwrap_or(0) as f64,
        y: w.y().unwrap_or(0) as f64,
        w: w.width().unwrap_or(0) as f64,
        h: w.height().unwrap_or(0) as f64,
    })
}

pub fn pid_for_route(id: RouteId) -> Option<i32> {
    if is_display_route(id) {
        return None;
    }
    let wid = window_id_from_route(id)?;
    let w = find_window(wid)?;
    w.pid().ok().map(|p| p as i32)
}
