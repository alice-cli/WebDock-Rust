//! Real mouse/keyboard injection.
//!
//! **macOS:** all CGEvent / enigo work runs on the **main thread** (HIToolbox
//! requires it). Text uses `CGEventKeyboardSetUnicodeString` so Hangul is not
//! remapped through the host IME layout (which produced ㅁㄴㅇ…).

use std::sync::Arc;
use std::time::{Duration, Instant};

use super::capture::bounds_for_route;
#[cfg(not(target_os = "macos"))]
use super::capture::pid_for_route;
use super::window_ctl::NativeWindowControl;
use crate::traits::*;
use crate::types::*;
#[cfg(not(target_os = "macos"))]
use enigo::Button;
use enigo::{
    Axis, Coordinate,
    Direction::{Click, Press, Release},
    Enigo, Key, Keyboard, Mouse, Settings,
};
use parking_lot::Mutex;
use tracing::{debug, warn};

/// Match Swift focus cache (~0.45s) — skip redundant raise / Window::all().
const FOCUS_BOUNDS_TTL: Duration = Duration::from_millis(450);

struct FocusCache {
    route: i64,
    raised_at: Instant,
    bounds: Option<(Rect, Instant)>,
}

pub struct NativeInput {
    enigo: Mutex<Option<Enigo>>,
    windows: Arc<NativeWindowControl>,
    focus: Mutex<Option<FocusCache>>,
}

impl NativeInput {
    pub fn new(windows: Arc<NativeWindowControl>) -> Self {
        Self {
            enigo: Mutex::new(None),
            windows,
            focus: Mutex::new(None),
        }
    }

    fn ensure_enigo(enigo: &mut Option<Enigo>) -> Result<&mut Enigo, InputError> {
        if enigo.is_none() {
            match Enigo::new(&Settings::default()) {
                Ok(e) => *enigo = Some(e),
                Err(e) => {
                    warn!(
                        error = %e,
                        "failed to init input injector — grant Accessibility to WebRust"
                    );
                    return Err(InputError::Other(
                        "input injector unavailable (Accessibility permission required)".into(),
                    ));
                }
            }
        }
        Ok(enigo.as_mut().unwrap())
    }

    /// Run enigo on the main queue (macOS TIS safety).
    fn with_enigo<F, R>(&self, f: F) -> Result<R, InputError>
    where
        F: FnOnce(&mut Enigo) -> Result<R, enigo::InputError> + Send,
        R: Send,
    {
        #[cfg(target_os = "macos")]
        {
            return dispatch2::run_on_main(move |_mtm| {
                let mut g = self.enigo.lock();
                let enigo = Self::ensure_enigo(&mut g)?;
                f(enigo).map_err(|e| InputError::Other(e.to_string()))
            });
        }
        #[cfg(not(target_os = "macos"))]
        {
            let mut g = self.enigo.lock();
            let enigo = Self::ensure_enigo(&mut g)?;
            f(enigo).map_err(|e| InputError::Other(e.to_string()))
        }
    }

    /// Run arbitrary main-thread input work.
    #[cfg(target_os = "macos")]
    fn on_main<F, R>(&self, f: F) -> Result<R, InputError>
    where
        F: FnOnce() -> Result<R, InputError> + Send,
        R: Send,
    {
        dispatch2::run_on_main(move |_mtm| f())
    }

    fn ensure_raised(&self, target: Option<&WindowRef>) {
        let Some(t) = target else { return };
        let rid = t.id.as_i64();
        {
            let g = self.focus.lock();
            if let Some(c) = g.as_ref() {
                if c.route == rid && c.raised_at.elapsed() < FOCUS_BOUNDS_TTL {
                    return;
                }
            }
        }
        let _ = self.windows.raise(t);
        // Fresh raise: give the WM a beat to commit z-order/focus so the very
        // first click lands on the target window, not the previous occluder.
        std::thread::sleep(Duration::from_millis(50));
        let mut g = self.focus.lock();
        match g.as_mut() {
            Some(c) if c.route == rid => c.raised_at = Instant::now(),
            _ => {
                *g = Some(FocusCache {
                    route: rid,
                    raised_at: Instant::now(),
                    bounds: None,
                });
            }
        }
    }

    fn abs_point(
        &self,
        x_frac: f64,
        y_frac: f64,
        target: Option<&WindowRef>,
    ) -> Result<(i32, i32), InputError> {
        let rect = if let Some(t) = target {
            let rid = t.id.as_i64();
            // Cached bounds — avoid Window::all() on every mouse move.
            {
                let g = self.focus.lock();
                if let Some(c) = g.as_ref() {
                    if c.route == rid {
                        if let Some((r, at)) = &c.bounds {
                            if at.elapsed() < FOCUS_BOUNDS_TTL {
                                let x =
                                    (r.x + x_frac.clamp(0.0, 1.0) * r.w.max(1.0)).round() as i32;
                                let y =
                                    (r.y + y_frac.clamp(0.0, 1.0) * r.h.max(1.0)).round() as i32;
                                return Ok((x, y));
                            }
                        }
                    }
                }
            }
            let rect = bounds_for_route(t.id).map_err(|e| InputError::Other(e.to_string()))?;
            let mut g = self.focus.lock();
            match g.as_mut() {
                Some(c) if c.route == rid => {
                    c.bounds = Some((rect.clone(), Instant::now()));
                }
                _ => {
                    *g = Some(FocusCache {
                        route: rid,
                        raised_at: Instant::now()
                            .checked_sub(FOCUS_BOUNDS_TTL)
                            .unwrap_or_else(Instant::now),
                        bounds: Some((rect.clone(), Instant::now())),
                    });
                }
            }
            rect
        } else {
            let monitors = xcap::Monitor::all().map_err(|e| InputError::Other(e.to_string()))?;
            let m = monitors
                .into_iter()
                .find(|m| m.is_primary().unwrap_or(false))
                .or_else(|| xcap::Monitor::all().ok().and_then(|v| v.into_iter().next()))
                .ok_or_else(|| InputError::Other("no monitor".into()))?;
            crate::types::Rect {
                x: m.x().unwrap_or(0) as f64,
                y: m.y().unwrap_or(0) as f64,
                w: m.width().unwrap_or(1) as f64,
                h: m.height().unwrap_or(1) as f64,
            }
        };
        let x = (rect.x + x_frac.clamp(0.0, 1.0) * rect.w.max(1.0)).round() as i32;
        let y = (rect.y + y_frac.clamp(0.0, 1.0) * rect.h.max(1.0)).round() as i32;
        Ok((x, y))
    }
}

impl InputInjector for NativeInput {
    fn mouse(&self, ev: &MouseEvent, target: Option<&WindowRef>) -> Result<(), InputError> {
        // Raise on down only (cached). Move must not re-enumerate windows.
        if matches!(ev.phase, MousePhase::Down) {
            self.ensure_raised(target);
        }
        let (x, y) = self.abs_point(ev.x, ev.y, target)?;
        let phase = ev.phase;
        let button = ev.button;
        let click_count = ev.click_count;

        #[cfg(target_os = "macos")]
        {
            return self.on_main(move || {
                super::cg_mouse::inject(phase, x as f64, y as f64, button, click_count)
            });
        }

        #[cfg(not(target_os = "macos"))]
        {
            let btn = map_button(button);
            self.with_enigo(move |enigo| {
                enigo.move_mouse(x, y, Coordinate::Abs)?;
                match phase {
                    MousePhase::Down => {
                        enigo.button(btn, Press)?;
                        if click_count >= 2 {
                            enigo.button(btn, Release)?;
                            enigo.button(btn, Press)?;
                        }
                    }
                    MousePhase::Move => {}
                    MousePhase::Up => {
                        enigo.button(btn, Release)?;
                    }
                }
                Ok(())
            })?;
            debug!(?phase, x, y, "inject mouse");
            Ok(())
        }
    }

    fn key(&self, ev: &KeyEvent, target: Option<&WindowRef>) -> Result<(), InputError> {
        // Cached raise — not every keystroke forks osascript.
        self.ensure_raised(target);

        // Printable Latin letters/digits: inject as **unicode** so host Hangul IME
        // does not remap KeyA→ㅁ. Special keys still use enigo keycodes.
        if let Some(ch) = printable_latin(&ev.code, ev.shift) {
            if !ev.meta && !ev.ctrl && !ev.alt {
                #[cfg(target_os = "macos")]
                {
                    let s = ch.to_string();
                    return self.on_main(move || super::cg_type::type_unicode(&s));
                }
                #[cfg(not(target_os = "macos"))]
                {
                    let s = ch.to_string();
                    return self.with_enigo(move |enigo| enigo.text(&s).map_err(|e| e));
                }
            }
        }

        let key = map_dom_key(&ev.code)
            .ok_or_else(|| InputError::Other(format!("unmapped key code: {}", ev.code)))?;
        let meta = ev.meta;
        let ctrl = ev.ctrl;
        let alt = ev.alt;
        let shift = ev.shift;
        let code = ev.code.clone();

        self.with_enigo(move |enigo| {
            if meta {
                enigo.key(Key::Meta, Press)?;
            }
            if ctrl {
                enigo.key(Key::Control, Press)?;
            }
            if alt {
                enigo.key(Key::Alt, Press)?;
            }
            if shift {
                enigo.key(Key::Shift, Press)?;
            }
            enigo.key(key, Click)?;
            if shift {
                enigo.key(Key::Shift, Release)?;
            }
            if alt {
                enigo.key(Key::Alt, Release)?;
            }
            if ctrl {
                enigo.key(Key::Control, Release)?;
            }
            if meta {
                enigo.key(Key::Meta, Release)?;
            }
            Ok(())
        })?;
        debug!(%code, "inject key");
        Ok(())
    }

    fn text(&self, s: &str, replace: u32, target: Option<&WindowRef>) -> Result<(), InputError> {
        self.ensure_raised(target);
        let owned = s.to_string();

        #[cfg(target_os = "macos")]
        {
            return self.on_main(move || {
                if replace > 0 {
                    super::cg_type::backspace_n(replace)?;
                }
                if !owned.is_empty() {
                    super::cg_type::type_unicode(&owned)?;
                }
                debug!(len = owned.len(), replace, "inject unicode text");
                Ok(())
            });
        }

        #[cfg(not(target_os = "macos"))]
        {
            self.with_enigo(move |enigo| {
                for _ in 0..replace {
                    enigo.key(Key::Backspace, Click)?;
                }
                if !owned.is_empty() {
                    enigo.text(&owned)?;
                }
                Ok(())
            })?;
            let _ = pid_for_route;
            Ok(())
        }
    }

    fn scroll(
        &self,
        dx: f64,
        dy: f64,
        x: f64,
        y: f64,
        target: Option<&WindowRef>,
    ) -> Result<(), InputError> {
        self.ensure_raised(target);
        let (px, py) = self.abs_point(x, y, target)?;
        self.with_enigo(move |enigo| {
            enigo.move_mouse(px, py, Coordinate::Abs)?;
            let sx = (dx * 3.0).round() as i32;
            let sy = (dy * 3.0).round() as i32;
            if sy != 0 {
                enigo.scroll(sy, Axis::Vertical)?;
            }
            if sx != 0 {
                enigo.scroll(sx, Axis::Horizontal)?;
            }
            Ok(())
        })?;
        Ok(())
    }
}

/// Map DOM key codes to US-layout printable chars (shift-aware).
///
/// Injected as unicode so host Hangul IME never remaps KeyA→ㅁ or Slash→nothing.
fn printable_latin(code: &str, shift: bool) -> Option<char> {
    if let Some(rest) = code.strip_prefix("Key") {
        if rest.len() == 1 {
            let c = rest.chars().next()?;
            if c.is_ascii_alphabetic() {
                return Some(if shift {
                    c.to_ascii_uppercase()
                } else {
                    c.to_ascii_lowercase()
                });
            }
        }
    }
    if let Some(rest) = code.strip_prefix("Digit") {
        if rest.len() == 1 && !shift {
            return rest.chars().next();
        }
        // US shifted digits
        if shift {
            return match rest {
                "1" => Some('!'),
                "2" => Some('@'),
                "3" => Some('#'),
                "4" => Some('$'),
                "5" => Some('%'),
                "6" => Some('^'),
                "7" => Some('&'),
                "8" => Some('*'),
                "9" => Some('('),
                "0" => Some(')'),
                _ => None,
            };
        }
    }
    // US punctuation (Slash/? must not go through enigo layout keys)
    match code {
        "Space" => Some(' '),
        "Minus" => Some(if shift { '_' } else { '-' }),
        "Equal" => Some(if shift { '+' } else { '=' }),
        "BracketLeft" => Some(if shift { '{' } else { '[' }),
        "BracketRight" => Some(if shift { '}' } else { ']' }),
        "Backslash" => Some(if shift { '|' } else { '\\' }),
        "Semicolon" => Some(if shift { ':' } else { ';' }),
        "Quote" => Some(if shift { '"' } else { '\'' }),
        "Comma" => Some(if shift { '<' } else { ',' }),
        "Period" => Some(if shift { '>' } else { '.' }),
        "Slash" => Some(if shift { '?' } else { '/' }),
        "Backquote" => Some(if shift { '~' } else { '`' }),
        "NumpadDivide" if !shift => Some('/'),
        "NumpadMultiply" if !shift => Some('*'),
        "NumpadSubtract" if !shift => Some('-'),
        "NumpadAdd" if !shift => Some('+'),
        "NumpadDecimal" if !shift => Some('.'),
        _ => None,
    }
}

#[cfg(not(target_os = "macos"))]
fn map_button(b: i32) -> Button {
    match b {
        1 => Button::Middle,
        2 => Button::Right,
        _ => Button::Left,
    }
}

fn map_dom_key(code: &str) -> Option<Key> {
    Some(match code {
        "Enter" => Key::Return,
        "Escape" => Key::Escape,
        "Backspace" => Key::Backspace,
        "Tab" => Key::Tab,
        "Space" => Key::Space,
        "Delete" => Key::Delete,
        "Home" => Key::Home,
        "End" => Key::End,
        "PageUp" => Key::PageUp,
        "PageDown" => Key::PageDown,
        "ArrowLeft" => Key::LeftArrow,
        "ArrowRight" => Key::RightArrow,
        "ArrowUp" => Key::UpArrow,
        "ArrowDown" => Key::DownArrow,
        "MetaLeft" | "MetaRight" | "OSLeft" | "OSRight" => Key::Meta,
        "ControlLeft" | "ControlRight" => Key::Control,
        "AltLeft" | "AltRight" => Key::Alt,
        "ShiftLeft" | "ShiftRight" => Key::Shift,
        "CapsLock" => Key::CapsLock,
        "F1" => Key::F1,
        "F2" => Key::F2,
        "F3" => Key::F3,
        "F4" => Key::F4,
        "F5" => Key::F5,
        "F6" => Key::F6,
        "F7" => Key::F7,
        "F8" => Key::F8,
        "F9" => Key::F9,
        "F10" => Key::F10,
        "F11" => Key::F11,
        "F12" => Key::F12,
        c if c.starts_with("Key") && c.len() == 4 => {
            let ch = c.chars().nth(3)?.to_ascii_lowercase();
            Key::Unicode(ch)
        }
        c if c.starts_with("Digit") && c.len() == 6 => {
            let ch = c.chars().nth(5)?;
            Key::Unicode(ch)
        }
        "Numpad0" => Key::Unicode('0'),
        "Numpad1" => Key::Unicode('1'),
        "Numpad2" => Key::Unicode('2'),
        "Numpad3" => Key::Unicode('3'),
        "Numpad4" => Key::Unicode('4'),
        "Numpad5" => Key::Unicode('5'),
        "Numpad6" => Key::Unicode('6'),
        "Numpad7" => Key::Unicode('7'),
        "Numpad8" => Key::Unicode('8'),
        "Numpad9" => Key::Unicode('9'),
        "NumpadAdd" => Key::Unicode('+'),
        "NumpadSubtract" => Key::Unicode('-'),
        "NumpadMultiply" => Key::Unicode('*'),
        "NumpadDivide" => Key::Unicode('/'),
        "NumpadDecimal" => Key::Unicode('.'),
        "NumpadEnter" => Key::Return,
        "Minus" => Key::Unicode('-'),
        "Equal" => Key::Unicode('='),
        "BracketLeft" => Key::Unicode('['),
        "BracketRight" => Key::Unicode(']'),
        "Backslash" => Key::Unicode('\\'),
        "Semicolon" => Key::Unicode(';'),
        "Quote" => Key::Unicode('\''),
        "Comma" => Key::Unicode(','),
        "Period" => Key::Unicode('.'),
        "Slash" => Key::Unicode('/'),
        "Backquote" => Key::Unicode('`'),
        _ => return None,
    })
}
