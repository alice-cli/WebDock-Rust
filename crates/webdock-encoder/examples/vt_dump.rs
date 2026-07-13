use webdock_encoder::H264Session;

fn main() {
    let w = 640u32;
    let h = 360u32;
    let mut s = H264Session::open(w, h, 30, 4_000_000).expect("open");
    println!("backend={}", s.backend_name());
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    for f in 0..8 {
        for (i, px) in rgba.chunks_mut(4).enumerate() {
            let x = (i as u32) % w;
            px[0] = ((x + f * 20) % 255) as u8;
            px[1] = 80;
            px[2] = 160;
            px[3] = 255;
        }
        match s.encode_rgba(&rgba, w, h, f as i64 * 33_333) {
            Ok(out) => {
                let head: Vec<String> = out
                    .avcc_au
                    .iter()
                    .take(24)
                    .map(|b| format!("{:02x}", b))
                    .collect();
                println!(
                    "f{f}: key={} au={} avcc={} codec={} head={}",
                    out.keyframe,
                    out.avcc_au.len(),
                    out.avcc_config.as_ref().map(|c| c.len()).unwrap_or(0),
                    out.codec,
                    head.join(" ")
                );
            }
            Err(e) => println!("f{f}: ERR {e}"),
        }
    }
}
