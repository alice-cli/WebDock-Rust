//! Frame packing + H.264 encode for WebDock wire protocol.
//!
//! JPEG/PNG travel as **raw** bytes (client magic-byte sniff).
//! H.264 uses the 14-byte framed packet (`0x01` type) + AVCC payload.
//!
//! # H.264 backends
//! - **macOS:** VideoToolbox hardware (preferred) → OpenH264 software fallback
//! - **Windows / Linux:** OpenH264 software (FFmpeg HW optional later)

mod h264;
#[cfg(target_os = "macos")]
mod macos_vt;
mod session;

pub use h264::{build_avcc, H264Encoded, H264Error, H264SoftEncoder};
pub use session::H264Session;

use thiserror::Error;
use webdock_protocol::H264Header;

#[derive(Debug, Error)]
pub enum EncodeError {
    #[error("unsupported format")]
    Unsupported,
    #[error("{0}")]
    Other(String),
}

/// JPEG is sent raw — no custom header (Swift `sendFrame` behavior).
#[inline]
pub fn pack_jpeg_frame(_pts_us: i64, jpeg: &[u8]) -> Vec<u8> {
    jpeg.to_vec()
}

/// PNG is sent raw — no custom header.
#[inline]
pub fn pack_png_frame(_pts_us: i64, png: &[u8]) -> Vec<u8> {
    png.to_vec()
}

/// H.264 sample packet matching Swift `sendH264Sample`.
#[inline]
pub fn pack_h264_frame(pts_us: i64, keyframe: bool, au: &[u8]) -> Vec<u8> {
    H264Header::pack(keyframe, pts_us, au)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jpeg_is_raw_ff_d8() {
        let raw = vec![0xFF, 0xD8, 0xFF, 0xD9];
        let out = pack_jpeg_frame(0, &raw);
        assert_eq!(out, raw);
    }

    #[test]
    fn h264_starts_with_01() {
        let out = pack_h264_frame(99, true, b"nal");
        assert_eq!(out[0], 0x01);
        assert_eq!(out[1], 0x01);
    }

    #[test]
    fn session_open_encodes_frame() {
        let mut s = H264Session::open(64, 64, 24, 2_000_000).expect("open");
        eprintln!("backend={}", s.backend_name());
        let mut rgba = vec![0u8; 64 * 64 * 4];
        for px in rgba.chunks_mut(4) {
            px[0] = 10;
            px[1] = 20;
            px[2] = 30;
            px[3] = 255;
        }
        let out = s.encode_rgba(&rgba, 64, 64, 0).expect("encode");
        assert!(!out.avcc_au.is_empty());
        assert!(out.keyframe || out.avcc_config.is_some() || !out.avcc_au.is_empty());
    }
}
