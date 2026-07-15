//! IME control — macOS Korean / Latin keyboard input-source switching.
//!
//! Mirrors WebDock `Sources/Input/InputSource.swift`:
//! - Carbon `TISSelectInputSource` (not Ctrl+Space / not `defaults read`)
//! - Prefer absolute setKorean(want) with retry
//! - Never inject Escape (chat apps treat it as close)
//! - Work on main queue

use crate::traits::*;
use parking_lot::Mutex;
use std::sync::OnceLock;
use tracing::{debug, warn};

pub struct NativeIme;

/// Last client-desired Korean mode (for logging / future heal).
fn desired_korean() -> &'static Mutex<Option<bool>> {
    static D: OnceLock<Mutex<Option<bool>>> = OnceLock::new();
    D.get_or_init(|| Mutex::new(None))
}

impl ImeControl for NativeIme {
    fn current_korean(&self) -> Option<(bool, String)> {
        #[cfg(target_os = "macos")]
        {
            return Some(macos::current_state());
        }
        #[cfg(not(target_os = "macos"))]
        {
            None
        }
    }

    fn set_korean(&self, korean: bool) -> Result<(bool, String), PlatformError> {
        *desired_korean().lock() = Some(korean);
        #[cfg(target_os = "macos")]
        {
            return Ok(macos::set_korean(korean));
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
        if cfg!(target_os = "macos") {
            caps.push("ime-korean".into());
        }
        caps
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use super::*;
    use core_foundation::base::TCFType;
    use core_foundation::string::CFString;
    use core_foundation_sys::base::CFTypeRef;
    use core_foundation_sys::string::CFStringRef;
    use std::ffi::c_void;
    use std::thread;
    use std::time::Duration;

    type TISInputSourceRef = *mut c_void;
    type OSStatus = i32;

    #[link(name = "Carbon", kind = "framework")]
    unsafe extern "C" {
        fn TISCopyCurrentKeyboardInputSource() -> TISInputSourceRef;
        fn TISCopyCurrentASCIICapableKeyboardInputSource() -> TISInputSourceRef;
        fn TISCreateInputSourceList(
            properties: *const c_void,
            include_all_installed: u8,
        ) -> *mut c_void; // CFArrayRef
        fn TISGetInputSourceProperty(
            source: TISInputSourceRef,
            property_key: CFStringRef,
        ) -> *mut c_void;
        fn TISSelectInputSource(source: TISInputSourceRef) -> OSStatus;
        fn TISEnableInputSource(source: TISInputSourceRef) -> OSStatus;

        static kTISPropertyInputSourceID: CFStringRef;
        static kTISPropertyInputSourceCategory: CFStringRef;
        static kTISCategoryKeyboardInputSource: CFStringRef;
    }

    use core_foundation_sys::array::{CFArrayGetCount, CFArrayGetValueAtIndex, CFArrayRef};
    use core_foundation_sys::base::CFRelease;

    pub fn current_state() -> (bool, String) {
        let korean = run_on_main(|| is_korean());
        (korean, if korean { "한".into() } else { "A".into() })
    }

    /// Absolute set — WebDock `InputSource.setKorean`.
    pub fn set_korean(want_korean: bool) -> (bool, String) {
        run_on_main(move || {
            // Already correct → do nothing (re-select mid-type can reset compose).
            if is_korean() == want_korean {
                debug!(want_korean, "IME already correct");
                return (
                    want_korean,
                    if want_korean {
                        "한".into()
                    } else {
                        "A".into()
                    },
                );
            }

            let ok = if want_korean {
                select_korean()
            } else {
                select_latin()
            };
            if !ok {
                warn!(want_korean, "IME first select failed");
            }
            thread::sleep(Duration::from_millis(12));

            let mut st = is_korean();
            if st != want_korean {
                // Retry once (WebDock does the same).
                let ok2 = if want_korean {
                    select_korean()
                } else {
                    select_latin()
                };
                if !ok2 {
                    warn!(want_korean, "IME retry select failed");
                }
                thread::sleep(Duration::from_millis(12));
                st = is_korean();
            }

            if st != want_korean {
                // Hard heal: flip opposite then desired (WebDock hardHeal, no Escape).
                debug!(want_korean, "IME hard-heal flip");
                if want_korean {
                    let _ = select_latin();
                    thread::sleep(Duration::from_millis(12));
                    let _ = select_korean();
                } else {
                    let _ = select_korean();
                    thread::sleep(Duration::from_millis(12));
                    let _ = select_latin();
                }
                thread::sleep(Duration::from_millis(12));
                st = is_korean();
            }

            if st != want_korean {
                warn!(
                    want_korean,
                    actual = st,
                    "IME state mismatch after select — check Input Sources in System Settings"
                );
            } else {
                debug!(want_korean, "IME switched OK");
            }

            (st, if st { "한".into() } else { "A".into() })
        })
    }

    fn run_on_main<F, R>(f: F) -> R
    where
        F: FnOnce() -> R + Send,
        R: Send,
    {
        dispatch2::run_on_main(move |_mtm| f())
    }

    fn is_korean() -> bool {
        unsafe {
            let src = TISCopyCurrentKeyboardInputSource();
            if src.is_null() {
                return false;
            }
            let id = source_id(src);
            CFRelease(src as CFTypeRef);
            is_korean_source_id(&id)
        }
    }

    fn select_korean() -> bool {
        let preferred = [
            "com.apple.inputmethod.Korean.2SetKorean",
            "com.apple.inputmethod.Korean.3SetKorean",
            "com.apple.inputmethod.Korean",
            "com.apple.inputmethod.Korean.390Sebulshik",
        ];
        if select_by_ids(&preferred) {
            return true;
        }
        let sources = all_keyboard_sources();
        let mut ok = false;
        for src in &sources {
            let id = unsafe { source_id(*src) };
            if is_korean_source_id(&id) && select_source(*src) {
                ok = true;
                break;
            }
        }
        release_sources(sources);
        if !ok {
            warn!("IME: no Korean input source found");
        }
        ok
    }

    fn select_latin() -> bool {
        let preferred = [
            "com.apple.keylayout.ABC",
            "com.apple.keylayout.US",
            "com.apple.keylayout.British",
        ];
        if select_by_ids(&preferred) {
            return true;
        }
        unsafe {
            let ascii = TISCopyCurrentASCIICapableKeyboardInputSource();
            if !ascii.is_null() {
                let id = source_id(ascii);
                let ok = !is_korean_source_id(&id) && select_source(ascii);
                CFRelease(ascii as CFTypeRef);
                if ok {
                    return true;
                }
            }
        }
        let sources = all_keyboard_sources();
        let mut ok = false;
        for src in &sources {
            let id = unsafe { source_id(*src) };
            if is_latin_source_id(&id) && !is_korean_source_id(&id) && select_source(*src) {
                ok = true;
                break;
            }
        }
        release_sources(sources);
        ok
    }

    fn select_by_ids(ids: &[&str]) -> bool {
        let sources = all_keyboard_sources();
        let mut ok = false;
        'outer: for want in ids {
            for src in &sources {
                let id = unsafe { source_id(*src) };
                if id == *want || id.starts_with(want) {
                    if select_source(*src) {
                        ok = true;
                        break 'outer;
                    }
                }
            }
        }
        release_sources(sources);
        ok
    }

    fn select_source(source: TISInputSourceRef) -> bool {
        if source.is_null() {
            return false;
        }
        unsafe {
            let _ = TISEnableInputSource(source);
            let err = TISSelectInputSource(source);
            if err != 0 {
                warn!(err, id = %source_id(source), "TISSelectInputSource failed");
                return false;
            }
            true
        }
    }

    fn all_keyboard_sources() -> Vec<TISInputSourceRef> {
        unsafe {
            // includeAllInstalled = true (WebDock). Filter keyboard category in-process.
            let list = TISCreateInputSourceList(std::ptr::null(), 1);
            if list.is_null() {
                return Vec::new();
            }
            let arr = list as CFArrayRef;
            let n = CFArrayGetCount(arr);
            let want_cat =
                CFString::wrap_under_get_rule(kTISCategoryKeyboardInputSource).to_string();
            let mut out = Vec::with_capacity(n as usize);
            for i in 0..n {
                let item = CFArrayGetValueAtIndex(arr, i) as TISInputSourceRef;
                if item.is_null() {
                    continue;
                }
                let cat_ptr =
                    TISGetInputSourceProperty(item, kTISPropertyInputSourceCategory) as CFStringRef;
                if cat_ptr.is_null() {
                    continue;
                }
                let cat = CFString::wrap_under_get_rule(cat_ptr).to_string();
                if cat == want_cat {
                    // Retain: list is released below; sources must outlive the list.
                    use core_foundation_sys::base::CFRetain;
                    CFRetain(item as CFTypeRef);
                    out.push(item);
                }
            }
            CFRelease(list as CFTypeRef);
            out
        }
    }

    /// Drop retained sources after selection attempts.
    fn release_sources(sources: Vec<TISInputSourceRef>) {
        for s in sources {
            if !s.is_null() {
                unsafe { CFRelease(s as CFTypeRef) };
            }
        }
    }

    unsafe fn source_id(source: TISInputSourceRef) -> String {
        let ptr = TISGetInputSourceProperty(source, kTISPropertyInputSourceID);
        if ptr.is_null() {
            return String::new();
        }
        CFString::wrap_under_get_rule(ptr as CFStringRef).to_string()
    }

    fn is_korean_source_id(id: &str) -> bool {
        let lower = id.to_ascii_lowercase();
        lower.contains("korean")
            || lower.contains("hangul")
            || lower.contains("2set")
            || lower.contains("3set")
            || lower.contains("dubeolsik")
            || lower.contains("sebeolsik")
    }

    fn is_latin_source_id(id: &str) -> bool {
        id.starts_with("com.apple.keylayout.")
            || id.contains("ABC")
            || id.contains(".US")
            || id.contains("British")
    }
}
