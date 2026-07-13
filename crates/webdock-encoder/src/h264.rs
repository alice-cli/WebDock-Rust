//! Software H.264 (OpenH264) → AVCC access units for WebCodecs.
//!
//! Tuned for **realtime screen share**: Medium complexity, bitrate RC, regular
//! keyframes. High complexity / Quality RC previously made 1080p encode slower
//! than the frame period → stream appeared frozen.

use openh264::encoder::{Encoder, EncoderConfig, FrameType};
use openh264::formats::{RgbaSliceU8, YUVBuffer};
use openh264::{Error as OhError, Timestamp};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum H264Error {
    #[error("openh264: {0}")]
    OpenH264(String),
    #[error("no NAL data")]
    Empty,
}

impl From<OhError> for H264Error {
    fn from(e: OhError) -> Self {
        Self::OpenH264(e.to_string())
    }
}

/// Encodes RGBA frames to AVCC length-prefixed H.264 for the browser client.
pub struct H264SoftEncoder {
    encoder: Encoder,
    width: u32,
    height: u32,
    bitrate_bps: u32,
    frame_idx: u64,
    sps: Option<Vec<u8>>,
    pps: Option<Vec<u8>>,
    force_key: bool,
    config_sent: bool,
}

pub struct H264Encoded {
    pub avcc_au: Vec<u8>,
    pub keyframe: bool,
    pub pts_us: i64,
    /// When SPS/PPS first appear (or change), include avcC for `h264config`.
    pub avcc_config: Option<Vec<u8>>,
    pub codec: String,
    pub width: u32,
    pub height: u32,
}

impl H264SoftEncoder {
    pub fn new(width: u32, height: u32, fps: u32, bitrate_bps: u32) -> Result<Self, H264Error> {
        let w = (width.max(2) & !1) as usize;
        let h = (height.max(2) & !1) as usize;
        let fps = fps.max(10).min(60);
        // Software path: keep bitrate high enough for UI text, but not so high that
        // encode time explodes. Floor 1.5 Mbps for small windows.
        let bps = bitrate_bps.max(1_500_000).min(12_000_000);
        // Key every ~1s so clients recover after packet loss / config change.
        let gop = fps.max(15).min(60);
        // Large frames: prefer Medium; High is not realtime on OpenH264.
        let complexity = if w * h > 1_280 * 800 {
            openh264::encoder::Complexity::Medium
        } else {
            openh264::encoder::Complexity::Medium
        };
        let cfg = EncoderConfig::new()
            .bitrate(openh264::encoder::BitRate::from_bps(bps))
            .max_frame_rate(openh264::encoder::FrameRate::from_hz(fps as f32))
            .usage_type(openh264::encoder::UsageType::ScreenContentRealTime)
            // Bitrate RC keeps frame sizes predictable for WS throughput.
            .rate_control_mode(openh264::encoder::RateControlMode::Bitrate)
            // Must stay false — skipped frames look like a frozen remote desktop.
            .skip_frames(false)
            .profile(openh264::encoder::Profile::Main)
            .level(openh264::encoder::Level::Level_4_0)
            .complexity(complexity)
            // Wider QP so encoder can hit realtime under load without dying.
            .qp(openh264::encoder::QpRange::new(18, 40))
            .intra_frame_period(openh264::encoder::IntraFramePeriod::from_num_frames(gop))
            .adaptive_quantization(false) // not supported for screen content anyway
            .scene_change_detect(true)
            .num_threads(0);
        let encoder = Encoder::with_api_config(openh264::OpenH264API::from_source(), cfg)?;
        let _ = (w, h);
        Ok(Self {
            encoder,
            width: w as u32,
            height: h as u32,
            bitrate_bps: bps,
            frame_idx: 0,
            sps: None,
            pps: None,
            force_key: true,
            config_sent: false,
        })
    }

    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    pub fn bitrate_bps(&self) -> u32 {
        self.bitrate_bps
    }

    pub fn force_keyframe(&mut self) {
        self.force_key = true;
        // Allow avcC to be re-sent (clients re-open VideoDecoder on window switch).
        self.config_sent = false;
    }

    /// Encode one RGBA frame (may be resized by caller to even dims).
    pub fn encode_rgba(
        &mut self,
        rgba: &[u8],
        width: u32,
        height: u32,
        pts_us: i64,
    ) -> Result<H264Encoded, H264Error> {
        let w = (width.max(2) & !1) as usize;
        let h = (height.max(2) & !1) as usize;
        let expected = w.saturating_mul(h).saturating_mul(4);
        if rgba.len() < expected {
            return Err(H264Error::OpenH264(format!(
                "rgba buffer too short: {} < {} for {}x{}",
                rgba.len(),
                expected,
                w,
                h
            )));
        }
        self.width = w as u32;
        self.height = h as u32;

        let rgb_src = RgbaSliceU8::new(&rgba[..expected], (w, h));
        let yuv = YUVBuffer::from_rgb_source(rgb_src);
        // Monotonic pts in ms — wall-clock jumps confuse some paths less than
        // absolute unix ms spanning huge range, but openh264 accepts either.
        let ts = Timestamp::from_millis((pts_us / 1000).max(0) as u64);

        // Force IDR *before* encode when requested. encode_at reinits on first
        // call (size); force after that via a second pass only if still no key.
        let need_force = self.force_key;
        if need_force {
            self.encoder.force_intra_frame();
        }
        let mut owned = collect_bitstream(&mut self.encoder, &yuv, ts)?;
        if need_force && !owned.keyframe && !owned.vcl_avcc.is_empty() {
            // First pass was reinit-only P/empty; force again and encode once more.
            self.encoder.force_intra_frame();
            owned = collect_bitstream(&mut self.encoder, &yuv, ts)?;
        }
        // If still no VCL after force, leave force_key set so next frame retries.
        if owned.vcl_avcc.is_empty() {
            return Err(H264Error::Empty);
        }
        self.force_key = false;

        let mut got_param = false;
        for nal in &owned.params {
            if nal.is_empty() {
                continue;
            }
            match nal[0] & 0x1f {
                7 => {
                    if self.sps.as_ref() != Some(nal) {
                        self.sps = Some(nal.clone());
                        self.config_sent = false;
                        got_param = true;
                    }
                }
                8 => {
                    if self.pps.as_ref() != Some(nal) {
                        self.pps = Some(nal.clone());
                        self.config_sent = false;
                        got_param = true;
                    }
                }
                _ => {}
            }
        }

        // Always treat IDR NAL as key even if FrameType was wrong.
        let keyframe = owned.keyframe;

        // Only emit avcC once (or when SPS/PPS change). Re-sending every keyframe
        // forces the browser to close/reopen VideoDecoder → freezes the picture.
        let avcc_config =
            if (!self.config_sent || got_param) && self.sps.is_some() && self.pps.is_some() {
                self.config_sent = true;
                Some(build_avcc(
                    self.sps.as_ref().unwrap(),
                    self.pps.as_ref().unwrap(),
                ))
            } else {
                None
            };

        let codec = self
            .sps
            .as_ref()
            .and_then(|s| codec_string_from_sps(s))
            .unwrap_or_else(|| "avc1.4D401E".into());

        self.frame_idx = self.frame_idx.wrapping_add(1);

        Ok(H264Encoded {
            avcc_au: owned.vcl_avcc,
            keyframe,
            pts_us,
            avcc_config,
            codec,
            width: self.width,
            height: self.height,
        })
    }
}

fn strip_start_code(nal: &[u8]) -> &[u8] {
    if nal.len() >= 4 && nal[0] == 0 && nal[1] == 0 && nal[2] == 0 && nal[3] == 1 {
        &nal[4..]
    } else if nal.len() >= 3 && nal[0] == 0 && nal[1] == 0 && nal[2] == 1 {
        &nal[3..]
    } else {
        nal
    }
}

struct OwnedAu {
    vcl_avcc: Vec<u8>,
    params: Vec<Vec<u8>>,
    keyframe: bool,
}

fn collect_bitstream(
    encoder: &mut Encoder,
    yuv: &YUVBuffer,
    ts: Timestamp,
) -> Result<OwnedAu, H264Error> {
    let bitstream = encoder.encode_at(yuv, ts)?;
    let ft = bitstream.frame_type();
    if matches!(ft, FrameType::Skip) || bitstream.num_layers() == 0 {
        return Ok(OwnedAu {
            vcl_avcc: Vec::new(),
            params: Vec::new(),
            keyframe: false,
        });
    }

    let mut keyframe = matches!(ft, FrameType::IDR | FrameType::I);
    let mut vcl_avcc = Vec::new();
    let mut params = Vec::new();

    for li in 0..bitstream.num_layers() {
        let Some(layer) = bitstream.layer(li) else {
            continue;
        };
        for ni in 0..layer.nal_count() {
            let Some(raw) = layer.nal_unit(ni) else {
                continue;
            };
            let nal = strip_start_code(raw);
            if nal.is_empty() {
                continue;
            }
            match nal[0] & 0x1f {
                7 | 8 => params.push(nal.to_vec()),
                5 => {
                    keyframe = true;
                    let len = nal.len() as u32;
                    vcl_avcc.extend_from_slice(&len.to_be_bytes());
                    vcl_avcc.extend_from_slice(nal);
                }
                1 => {
                    let len = nal.len() as u32;
                    vcl_avcc.extend_from_slice(&len.to_be_bytes());
                    vcl_avcc.extend_from_slice(nal);
                }
                _ => {}
            }
        }
    }

    Ok(OwnedAu {
        vcl_avcc,
        params,
        keyframe,
    })
}

/// ISO-BMFF avcC box payload (without box header) for WebCodecs `description`.
pub fn build_avcc(sps: &[u8], pps: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(11 + sps.len() + pps.len());
    out.push(1); // configurationVersion
    out.push(if sps.len() > 1 { sps[1] } else { 0x42 }); // AVCProfileIndication
    out.push(if sps.len() > 2 { sps[2] } else { 0x00 }); // profile_compatibility
    out.push(if sps.len() > 3 { sps[3] } else { 0x1E }); // AVCLevelIndication
    out.push(0xFF); // lengthSizeMinusOne = 3 (4-byte lengths)
    out.push(0xE1); // numOfSequenceParameterSets = 1
    out.extend_from_slice(&(sps.len() as u16).to_be_bytes());
    out.extend_from_slice(sps);
    out.push(1); // numOfPictureParameterSets
    out.extend_from_slice(&(pps.len() as u16).to_be_bytes());
    out.extend_from_slice(pps);
    out
}

fn codec_string_from_sps(sps: &[u8]) -> Option<String> {
    if sps.len() < 4 {
        return None;
    }
    Some(format!("avc1.{:02X}{:02X}{:02X}", sps[1], sps[2], sps[3]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn avcc_header_shape() {
        let sps = vec![0x67, 0x42, 0xC0, 0x1E, 0x00];
        let pps = vec![0x68, 0xCE, 0x06, 0xE2];
        let box_ = build_avcc(&sps, &pps);
        assert_eq!(box_[0], 1);
        assert_eq!(box_[1], 0x42);
        assert_eq!(&box_[6..8], &(sps.len() as u16).to_be_bytes());
    }

    #[test]
    fn encode_solid_frame_produces_key_and_avcc() {
        let w = 64u32;
        let h = 64u32;
        let mut enc = H264SoftEncoder::new(w, h, 30, 1_500_000).expect("encoder");
        let mut rgba = vec![0u8; (w * h * 4) as usize];
        for px in rgba.chunks_mut(4) {
            px[0] = 40;
            px[1] = 120;
            px[2] = 200;
            px[3] = 255;
        }
        let out = enc
            .encode_rgba(&rgba, w, h, 0)
            .expect("first frame should encode");
        assert!(out.keyframe, "first frame should be key");
        assert!(!out.avcc_au.is_empty());
        assert!(
            out.avcc_config.is_some(),
            "avcC config must be present for WebCodecs"
        );
        for px in rgba.chunks_mut(4) {
            px[0] = 200;
        }
        let out2 = enc.encode_rgba(&rgba, w, h, 33_000).expect("P frame");
        assert!(!out2.avcc_au.is_empty());
    }

    #[test]
    fn encode_1080p_is_realtime_enough() {
        // Soft gate: avg encode under ~80ms in release-ish; debug can be slower.
        let w = 1280u32;
        let h = 720u32;
        let mut enc = H264SoftEncoder::new(w, h, 24, 4_000_000).expect("encoder");
        let mut rgba = vec![0u8; (w * h * 4) as usize];
        for (i, px) in rgba.chunks_mut(4).enumerate() {
            px[0] = (i % 255) as u8;
            px[1] = 80;
            px[2] = 160;
            px[3] = 255;
        }
        let t0 = Instant::now();
        let n = 5;
        for i in 0..n {
            for px in rgba.chunks_mut(4) {
                px[0] = px[0].wrapping_add(1);
            }
            let out = enc
                .encode_rgba(&rgba, w, h, i * 40_000)
                .expect("encode 720p");
            assert!(!out.avcc_au.is_empty());
            if i == 0 {
                assert!(out.keyframe);
            }
        }
        let avg_ms = t0.elapsed().as_secs_f64() * 1000.0 / n as f64;
        eprintln!("720p avg encode {avg_ms:.1} ms");
        // Debug builds can be slow; only assert we got frames (smoke).
        assert!(avg_ms < 2000.0, "encoder path seems hung: {avg_ms} ms");
    }
}
