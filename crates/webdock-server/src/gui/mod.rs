//! Desktop settings shell (menu bar + settings window).
//!
//! macOS: tao + wry (AppKit-backed). Other platforms: fall back to CLI note.

#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "macos")]
pub use macos::run;

#[cfg(not(target_os = "macos"))]
pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    eprintln!("GUI is currently implemented for macOS only. Use --cli mode:");
    eprintln!("  WebRust --cli --port 8090");
    Err("GUI not available on this platform".into())
}
