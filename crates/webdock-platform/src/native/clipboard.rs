//! Host clipboard read for remote → browser sync.

use std::time::{Duration, Instant};

const MAX_CHARS: usize = 512_000;

/// Current general pasteboard change count (macOS). Other OS: 0.
pub fn change_count() -> i64 {
    #[cfg(target_os = "macos")]
    {
        return macos_change_count();
    }
    #[cfg(not(target_os = "macos"))]
    {
        0
    }
}

/// Best-effort plain text from the host clipboard.
pub fn read_string() -> String {
    #[cfg(target_os = "macos")]
    {
        if let Some(s) = macos_read_string() {
            return clip(s);
        }
    }
    // Fallback / non-macOS
    match arboard::Clipboard::new().and_then(|mut c| c.get_text()) {
        Ok(s) => clip(s),
        Err(_) => String::new(),
    }
}

/// After a copy shortcut, wait until pasteboard `changeCount` moves (or timeout).
pub fn read_string_after_change(from: i64, timeout_ms: u64) -> String {
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    while Instant::now() < deadline {
        let now = change_count();
        if now != from {
            std::thread::sleep(Duration::from_millis(30));
            let s = read_string();
            if !s.is_empty() {
                return s;
            }
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    read_string()
}

fn clip(s: String) -> String {
    if s.chars().count() > MAX_CHARS {
        s.chars().take(MAX_CHARS).collect()
    } else {
        s
    }
}

#[cfg(target_os = "macos")]
fn macos_change_count() -> i64 {
    use objc2_app_kit::NSPasteboard;
    let pb = NSPasteboard::generalPasteboard();
    pb.changeCount() as i64
}

#[cfg(target_os = "macos")]
fn macos_read_string() -> Option<String> {
    use objc2::rc::Retained;
    use objc2_app_kit::{NSPasteboard, NSPasteboardTypeString};
    use objc2_foundation::NSString;

    let pb = NSPasteboard::generalPasteboard();
    let s: Option<Retained<NSString>> = pb.stringForType(unsafe { NSPasteboardTypeString });
    s.map(|ns| ns.to_string()).filter(|t| !t.is_empty())
}
