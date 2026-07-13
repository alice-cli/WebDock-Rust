//! Layout-independent unicode typing via CGEvent (main thread).
//!
//! Used so Hangul / CJK text is not re-interpreted by the host keyboard layout
//! (enigo's Key::Unicode path uses TIS → 한글 자판에서 ㅁㄴㅇ… 로 깨짐).

#![cfg(target_os = "macos")]

use core_graphics::event::{CGEvent, CGEventFlags, CGEventTapLocation, CGKeyCode, KeyCode};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use tracing::warn;

use crate::traits::InputError;

/// Type a unicode string (Hangul syllables, jamo, emoji…) without layout mapping.
pub fn type_unicode(s: &str) -> Result<(), InputError> {
    if s.is_empty() {
        return Ok(());
    }
    // CGEventKeyboardSetUnicodeString has a practical length limit; chunk it.
    for chunk in s.chars().collect::<Vec<_>>().chunks(16) {
        let part: String = chunk.iter().collect();
        post_unicode(&part)?;
    }
    Ok(())
}

/// Backspace `n` times using hardware keycode (layout-independent).
pub fn backspace_n(n: u32) -> Result<(), InputError> {
    for _ in 0..n {
        key_tap(KeyCode::DELETE)?;
    }
    Ok(())
}

fn post_unicode(s: &str) -> Result<(), InputError> {
    // combinedSessionState matches Swift postUnicode — more reliable for text fields.
    let source = CGEventSource::new(CGEventSourceStateID::CombinedSessionState)
        .map_err(|_| InputError::Other("CGEventSource failed".into()))?;
    // keycode 0 + unicode string — standard technique for layout-independent input.
    let down = CGEvent::new_keyboard_event(source.clone(), 0 as CGKeyCode, true)
        .map_err(|_| InputError::Other("CGEvent keyboard down failed".into()))?;
    down.set_flags(CGEventFlags::CGEventFlagNull);
    down.set_string(s);
    down.post(CGEventTapLocation::HID);

    let up = CGEvent::new_keyboard_event(source, 0 as CGKeyCode, false)
        .map_err(|_| InputError::Other("CGEvent keyboard up failed".into()))?;
    up.set_flags(CGEventFlags::CGEventFlagNull);
    up.post(CGEventTapLocation::HID);
    Ok(())
}

fn key_tap(code: CGKeyCode) -> Result<(), InputError> {
    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
        .map_err(|_| InputError::Other("CGEventSource failed".into()))?;
    let down = CGEvent::new_keyboard_event(source.clone(), code, true)
        .map_err(|_| InputError::Other("CGEvent key down failed".into()))?;
    down.set_flags(CGEventFlags::CGEventFlagNull);
    down.post(CGEventTapLocation::HID);

    let up = CGEvent::new_keyboard_event(source, code, false)
        .map_err(|_| InputError::Other("CGEvent key up failed".into()))?;
    up.set_flags(CGEventFlags::CGEventFlagNull);
    up.post(CGEventTapLocation::HID);
    Ok(())
}

/// Optional: log if Accessibility is missing (events silently dropped by macOS).
pub fn warn_if_needed(err: &InputError) {
    warn!(error = %err, "CGEvent input failed");
}
