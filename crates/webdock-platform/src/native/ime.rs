//! IME control. Full Hangul switching is OS-specific; capability-flagged.

use crate::traits::*;

pub struct NativeIme;

impl ImeControl for NativeIme {
    fn current_korean(&self) -> Option<(bool, String)> {
        #[cfg(target_os = "macos")]
        {
            return macos_ime_state();
        }
        #[cfg(not(target_os = "macos"))]
        {
            None
        }
    }

    fn set_korean(&self, korean: bool) -> Result<(bool, String), PlatformError> {
        #[cfg(target_os = "macos")]
        {
            return macos_set_korean(korean);
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = korean;
            Err(PlatformError::Other(
                "IME control not available on this platform yet".into(),
            ))
        }
    }

    fn capabilities(&self) -> Vec<String> {
        let mut caps = vec![
            "capture".into(),
            "input".into(),
            "jpeg".into(),
            "h264".into(),
            "window-list".into(),
            "display-list".into(),
        ];
        #[cfg(target_os = "macos")]
        {
            caps.push("ime-korean".into());
        }
        caps
    }
}

#[cfg(target_os = "macos")]
fn macos_ime_state() -> Option<(bool, String)> {
    // Read current input source via `defaults` / Carbon is heavy; use `xcrun` free path:
    // `defaults read` is unreliable. Prefer `hidutil` no — use shell `xkbswitch` free:
    // AppleScript System Events may not expose layout. Use `ioreg` free path via:
    // `defaults read com.apple.HIToolbox AppleSelectedInputSources`
    let output = std::process::Command::new("defaults")
        .args(["read", "com.apple.HIToolbox", "AppleSelectedInputSources"])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    let korean = text.contains("2SetKorean")
        || text.contains("Korean")
        || text.contains("Hangul")
        || text.contains("com.apple.inputmethod.Korean");
    Some((korean, if korean { "한".into() } else { "A".into() }))
}

#[cfg(target_os = "macos")]
fn macos_set_korean(want: bool) -> Result<(bool, String), PlatformError> {
    // Toggle via Ctrl+Space is layout-dependent. Prefer selecting by input source id.
    let source = if want {
        "com.apple.inputmethod.Korean.2SetKorean"
    } else {
        "com.apple.keylayout.ABC"
    };
    // `input source` selection via Carbon TIS is ideal; use open-source free shell:
    // im-select if installed, else try AppleScript menu (fragile).
    if let Ok(status) = std::process::Command::new("im-select").arg(source).status() {
        if status.success() {
            return Ok((want, if want { "한".into() } else { "A".into() }));
        }
    }
    // Fallback: simulate Globe / Ctrl-Space once if current state mismatches.
    if let Some((cur, _)) = macos_ime_state() {
        if cur != want {
            let _ = std::process::Command::new("osascript")
                .args([
                    "-e",
                    r#"tell application "System Events" to key code 49 using control down"#,
                ])
                .status();
        }
    }
    let state = macos_ime_state().unwrap_or((want, if want { "한".into() } else { "A".into() }));
    Ok(state)
}
