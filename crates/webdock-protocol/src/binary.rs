//! Binary wire format matching the original Swift WebDock host.
//!
//! - **JPEG / PNG**: raw image bytes only (client sniffs `FF D8` / `89 50 4E 47`).
//! - **H.264 sample**: framed packet  
//!   `[0]=0x01` type · `[1]=flags(key=1)` · `[2..9]=pts_us BE i64` · `[10..13]=len BE u32` · AVCC payload

use thiserror::Error;

/// Header size for H.264 sample packets only.
pub const H264_HEADER_LEN: usize = 1 + 1 + 8 + 4;

/// Legacy alias used by older call sites.
pub const FRAME_HEADER_LEN: usize = H264_HEADER_LEN;

/// Wire type byte for H.264 samples (must be `0x01` — client checks this first).
pub const H264_TYPE_BYTE: u8 = 0x01;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameType {
    /// H.264 sample packet type byte on the wire.
    H264 = 0x01,
}

impl FrameType {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0x01 => Some(Self::H264),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct H264Header {
    pub keyframe: bool,
    pub pts_us: i64,
    pub payload_len: u32,
}

#[derive(Debug, Error)]
pub enum FrameError {
    #[error("buffer too short for header")]
    TruncatedHeader,
    #[error("unknown frame type {0:#x} (expected H.264 0x01)")]
    UnknownType(u8),
    #[error("payload length mismatch")]
    LengthMismatch,
}

impl H264Header {
    pub fn encode(&self) -> [u8; H264_HEADER_LEN] {
        let mut buf = [0u8; H264_HEADER_LEN];
        buf[0] = H264_TYPE_BYTE;
        buf[1] = if self.keyframe { 0x01 } else { 0x00 };
        buf[2..10].copy_from_slice(&self.pts_us.to_be_bytes());
        buf[10..14].copy_from_slice(&self.payload_len.to_be_bytes());
        buf
    }

    pub fn decode(buf: &[u8]) -> Result<Self, FrameError> {
        if buf.len() < H264_HEADER_LEN {
            return Err(FrameError::TruncatedHeader);
        }
        if buf[0] != H264_TYPE_BYTE {
            return Err(FrameError::UnknownType(buf[0]));
        }
        let keyframe = (buf[1] & 1) == 1;
        let pts_us = i64::from_be_bytes(buf[2..10].try_into().unwrap());
        let payload_len = u32::from_be_bytes(buf[10..14].try_into().unwrap());
        Ok(Self {
            keyframe,
            pts_us,
            payload_len,
        })
    }

    /// Pack H.264 sample: header + AVCC access unit.
    pub fn pack(keyframe: bool, pts_us: i64, payload: &[u8]) -> Vec<u8> {
        let header = H264Header {
            keyframe,
            pts_us,
            payload_len: payload.len() as u32,
        };
        let mut out = Vec::with_capacity(H264_HEADER_LEN + payload.len());
        out.extend_from_slice(&header.encode());
        out.extend_from_slice(payload);
        out
    }
}

/// Deprecated name kept for call-site clarity during H.264 work.
pub type FrameHeader = H264Header;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn h264_roundtrip_matches_swift() {
        let payload = b"\x00\x00\x00\x01fake";
        let packed = H264Header::pack(true, 1_234_567, payload);
        assert_eq!(packed[0], 0x01);
        assert_eq!(packed[1], 0x01);
        assert_eq!(packed.len(), H264_HEADER_LEN + payload.len());
        let h = H264Header::decode(&packed).unwrap();
        assert!(h.keyframe);
        assert_eq!(h.pts_us, 1_234_567);
        assert_eq!(h.payload_len, payload.len() as u32);
        assert_eq!(&packed[H264_HEADER_LEN..], payload);
    }

    #[test]
    fn jpeg_must_not_use_type_byte_01() {
        // Client treats 0x01 as H.264; JPEG starts with FF D8.
        let jpeg = [0xFFu8, 0xD8, 0xFF, 0xD9];
        assert_ne!(jpeg[0], H264_TYPE_BYTE);
    }

    #[test]
    fn non_h264_type_rejected() {
        let mut buf = [0u8; 14];
        buf[0] = 0x03; // old mistaken enum value
        assert!(matches!(
            H264Header::decode(&buf),
            Err(FrameError::UnknownType(0x03))
        ));
    }
}
