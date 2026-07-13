//! Cross-platform H.264 session: pick best backend per OS.
//!
//! | OS     | Preferred              | Fallback   |
//! |--------|------------------------|------------|
//! | macOS  | VideoToolbox (HW)      | OpenH264   |
//! | Win/Linux | OpenH264 (SW)       | —          |
//!
//! Future: FFmpeg (`h264_nvenc` / `h264_vaapi` / `h264_mf`) as optional feature.

use tracing::{info, warn};

use crate::h264::{H264Encoded, H264Error, H264SoftEncoder};

#[cfg(target_os = "macos")]
use crate::macos_vt::VtEncoder;

/// Opaque H.264 encoder that hides the active backend.
pub struct H264Session {
    inner: Inner,
}

enum Inner {
    Soft(H264SoftEncoder),
    #[cfg(target_os = "macos")]
    VideoToolbox(VtEncoder),
}

impl H264Session {
    /// Open the best available encoder for this platform.
    ///
    /// `WEBRUST_H264_BACKEND=sw` forces OpenH264 — lets a macOS host exercise
    /// the exact Windows/Linux software path for debugging.
    pub fn open(width: u32, height: u32, fps: u32, bitrate_bps: u32) -> Result<Self, H264Error> {
        let force_sw = std::env::var("WEBRUST_H264_BACKEND")
            .map(|v| v.eq_ignore_ascii_case("sw") || v.eq_ignore_ascii_case("openh264"))
            .unwrap_or(false);
        #[cfg(not(target_os = "macos"))]
        let _ = force_sw;
        #[cfg(target_os = "macos")]
        if !force_sw {
            match VtEncoder::new(width, height, fps, bitrate_bps) {
                Ok(vt) => {
                    info!(
                        w = width,
                        h = height,
                        bitrate_bps,
                        "H.264 backend: VideoToolbox (hardware)"
                    );
                    return Ok(Self {
                        inner: Inner::VideoToolbox(vt),
                    });
                }
                Err(e) => {
                    warn!(
                        error = %e,
                        "VideoToolbox unavailable — falling back to OpenH264 software"
                    );
                }
            }
        }

        let soft = H264SoftEncoder::new(width, height, fps, bitrate_bps)?;
        info!(
            w = width,
            h = height,
            bitrate_bps,
            "H.264 backend: OpenH264 (software)"
        );
        Ok(Self {
            inner: Inner::Soft(soft),
        })
    }

    pub fn backend_name(&self) -> &'static str {
        match &self.inner {
            Inner::Soft(_) => "openh264",
            #[cfg(target_os = "macos")]
            Inner::VideoToolbox(_) => "videotoolbox",
        }
    }

    pub fn dimensions(&self) -> (u32, u32) {
        match &self.inner {
            Inner::Soft(e) => e.dimensions(),
            #[cfg(target_os = "macos")]
            Inner::VideoToolbox(e) => e.dimensions(),
        }
    }

    pub fn bitrate_bps(&self) -> u32 {
        match &self.inner {
            Inner::Soft(e) => e.bitrate_bps(),
            #[cfg(target_os = "macos")]
            Inner::VideoToolbox(e) => e.bitrate_bps(),
        }
    }

    pub fn force_keyframe(&mut self) {
        match &mut self.inner {
            Inner::Soft(e) => e.force_keyframe(),
            #[cfg(target_os = "macos")]
            Inner::VideoToolbox(e) => e.force_keyframe(),
        }
    }

    pub fn encode_rgba(
        &mut self,
        rgba: &[u8],
        width: u32,
        height: u32,
        pts_us: i64,
    ) -> Result<H264Encoded, H264Error> {
        match &mut self.inner {
            Inner::Soft(e) => e.encode_rgba(rgba, width, height, pts_us),
            #[cfg(target_os = "macos")]
            Inner::VideoToolbox(e) => e.encode_rgba(rgba, width, height, pts_us),
        }
    }
}
