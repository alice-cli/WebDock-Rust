//! Dump OpenH264 soft-encoder output (avcC + AVCC AUs) as JSON for a
//! WebCodecs decode harness. Debug tool — not shipped.
//!
//! cargo run -p webdock-encoder --example soft_dump -- /tmp/dump.json

use webdock_encoder::H264SoftEncoder;

fn b64(data: &[u8]) -> String {
    use base64::{engine::general_purpose::STANDARD, Engine};
    STANDARD.encode(data)
}

fn main() {
    let out_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "soft_dump.json".into());
    let (w, h) = (1280u32, 720u32);
    let mut enc = H264SoftEncoder::new(w, h, 30, 4_000_000).expect("encoder");
    let mut rgba = vec![0u8; (w * h * 4) as usize];

    // Screen-content-ish: text-like blocks + a moving box.
    for (i, px) in rgba.chunks_mut(4).enumerate() {
        let x = (i as u32) % w;
        let y = (i as u32) / w;
        let v = if (x / 8 + y / 16) % 2 == 0 { 235 } else { 30 };
        px[0] = v;
        px[1] = v;
        px[2] = v;
        px[3] = 255;
    }

    let mut frames = Vec::new();
    let mut config = None;
    let mut codec = String::new();
    for f in 0..60i64 {
        // Move a 64x64 box across the frame.
        let bx = ((f as u32) * 16) % (w - 64);
        for yy in 100..164u32 {
            for xx in bx..bx + 64 {
                let idx = ((yy * w + xx) * 4) as usize;
                rgba[idx] = 220;
                rgba[idx + 1] = 60;
                rgba[idx + 2] = 60;
            }
        }
        match enc.encode_rgba(&rgba, w, h, f * 33_333) {
            Ok(out) => {
                if let Some(c) = &out.avcc_config {
                    config = Some(b64(c));
                }
                codec = out.codec.clone();
                frames.push(format!(
                    "{{\"key\":{},\"data\":\"{}\"}}",
                    out.keyframe,
                    b64(&out.avcc_au)
                ));
            }
            Err(e) => {
                eprintln!("frame {f}: encode error: {e}");
            }
        }
    }

    let json = format!(
        "{{\"codec\":\"{}\",\"width\":{},\"height\":{},\"description\":\"{}\",\"frames\":[{}]}}",
        codec,
        w,
        h,
        config.unwrap_or_default(),
        frames.join(",")
    );
    std::fs::write(&out_path, json).expect("write dump");
    eprintln!("wrote {out_path} ({} frames, codec {codec})", frames.len());
}
