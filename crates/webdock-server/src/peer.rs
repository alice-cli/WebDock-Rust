use std::sync::atomic::{AtomicBool, Ordering};

use tokio::sync::mpsc;

/// Dual-channel outbound: control JSON is never dropped by video backpressure.
pub struct PeerOutbound {
    pub ctrl: mpsc::Sender<String>,
    pub video: mpsc::Sender<Vec<u8>>,
    /// Real peer IP (or proxy-forwarded).
    pub ip: String,
    /// When true, Cmd/Ctrl+C/X pushes host pasteboard text to this browser.
    clip_auto: AtomicBool,
}

impl PeerOutbound {
    pub fn new(ctrl: mpsc::Sender<String>, video: mpsc::Sender<Vec<u8>>, ip: String) -> Self {
        Self {
            ctrl,
            video,
            ip,
            clip_auto: AtomicBool::new(true),
        }
    }

    pub fn set_clip_auto(&self, on: bool) {
        self.clip_auto.store(on, Ordering::Relaxed);
    }

    pub fn clip_auto(&self) -> bool {
        self.clip_auto.load(Ordering::Relaxed)
    }
}
