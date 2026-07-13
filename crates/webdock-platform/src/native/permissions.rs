//! Host permission diagnostics for real capture/input.

use tracing::{info, warn};

/// Log what the OS requires for production use.
pub fn log_requirements() {
    #[cfg(target_os = "macos")]
    {
        info!("macOS permissions required for full remote control:");
        info!("  • Screen Recording  — capture windows & displays");
        info!("  • Accessibility     — mouse/keyboard injection & window raise");
        info!("System Settings → Privacy & Security → grant both to WebDock (or your terminal if cargo run)");
        // Probe capture
        match xcap::Window::all() {
            Ok(w) => info!(windows = w.len(), "screen capture probe: OK"),
            Err(e) => warn!(error = %e, "screen capture probe: FAILED"),
        }
        match xcap::Monitor::all() {
            Ok(m) => info!(monitors = m.len(), "display capture probe: OK"),
            Err(e) => warn!(error = %e, "display capture probe: FAILED"),
        }
    }
    #[cfg(target_os = "windows")]
    {
        info!("Windows: ensure the app is allowed to capture the screen (Graphics Capture).");
        info!("Input injection may require running as the interactive user session.");
    }
    #[cfg(target_os = "linux")]
    {
        info!("Linux: X11 works best. Wayland needs xdg-desktop-portal screen share.");
        info!("For window focus, install xdotool. Input may need uinput permissions.");
    }
}
