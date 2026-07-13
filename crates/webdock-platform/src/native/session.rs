//! Login session / lock-screen helpers (macOS).

/// True when the GUI session shows the lock screen (user still logged in).
pub fn is_screen_locked() -> bool {
    #[cfg(target_os = "macos")]
    {
        return macos_is_screen_locked();
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

#[cfg(target_os = "macos")]
fn macos_is_screen_locked() -> bool {
    use core_foundation::base::{CFType, TCFType};
    use core_foundation::boolean::CFBoolean;
    use core_foundation::dictionary::CFDictionary;
    use core_foundation::number::CFNumber;
    use core_foundation::string::CFString;
    use std::os::raw::c_void;

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGSessionCopyCurrentDictionary() -> *const c_void;
    }

    unsafe {
        let raw = CGSessionCopyCurrentDictionary();
        if raw.is_null() {
            return false;
        }
        let dict = CFDictionary::<CFString, CFType>::wrap_under_create_rule(raw as *const _);
        let key = CFString::from_static_string("CGSSessionScreenIsLocked");
        let Some(val) = dict.find(&key) else {
            return false;
        };
        // Bool as CFBoolean
        if let Some(b) = val.clone().downcast_into::<CFBoolean>() {
            return b.into();
        }
        // Sometimes Int/NSNumber
        if let Some(n) = val.clone().downcast_into::<CFNumber>() {
            if let Some(i) = n.to_i64() {
                return i != 0;
            }
        }
        false
    }
}
