//! Close a single macOS window without quitting the app.
//!
//! Mirrors WebDock `Sources/Input/WindowClose.swift`:
//! 1. Accessibility close button on the exact `CGWindowID` (via private `_AXUIElementGetWindow`)
//! 2. Fallback: raise + Cmd+W via CGEvent

#![cfg(target_os = "macos")]

use std::ffi::c_void;
use std::ptr;

use core_foundation::base::TCFType;
use core_foundation::string::CFString;
use core_foundation_sys::array::{CFArrayGetCount, CFArrayGetValueAtIndex, CFArrayRef};
use core_foundation_sys::base::{CFRelease, CFRetain, CFTypeRef};
use core_foundation_sys::string::CFStringRef;
use core_graphics::event::{CGEvent, CGEventFlags, CGEventTapLocation, CGKeyCode};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use tracing::{debug, info, warn};

type AXUIElementRef = *mut c_void;
type AXError = i32;

const AX_OK: AXError = 0;
/// ANSI W (kVK_ANSI_W) — Cmd+W close shortcut.
const KEY_W: CGKeyCode = 0x0D;

/// AX attribute/action name held for the duration of a call.
/// Linking `kAX*Attribute` C statics fails on some arm64 linkers; string form
/// matches the public Accessibility API values WebDock uses.
struct AxName(CFString);
impl AxName {
    fn new(s: &'static str) -> Self {
        Self(CFString::from_static_string(s))
    }
    fn r(&self) -> CFStringRef {
        self.0.as_concrete_TypeRef()
    }
}

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXUIElementCreateApplication(pid: i32) -> AXUIElementRef;
    fn AXUIElementCopyAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: *mut CFTypeRef,
    ) -> AXError;
    fn AXUIElementPerformAction(element: AXUIElementRef, action: CFStringRef) -> AXError;

    /// Private HIServices symbol (same as WebDock AccessibilityHelpers).
    fn _AXUIElementGetWindow(element: AXUIElementRef, out_wid: *mut u32) -> AXError;
}

/// Close one window of `pid`. Prefer CGWindowID match; title is a fallback.
///
/// Work is forced onto the main queue (AX + CGEvent requirement, matching WebDock).
pub fn close_window(pid: i32, window_id: u32, title: Option<&str>) -> bool {
    if pid <= 0 {
        return false;
    }
    let title_owned = title.map(|s| s.to_string());
    dispatch2::run_on_main(move |_mtm| {
        if close_via_ax(pid, window_id, title_owned.as_deref()) {
            info!(pid, window_id, "window closed via AX close button");
            return true;
        }
        debug!(pid, window_id, "AX close failed — trying Cmd+W");
        if close_via_cmd_w(pid, window_id, title_owned.as_deref()) {
            info!(pid, window_id, "window closed via Cmd+W");
            return true;
        }
        warn!(
            pid,
            window_id, "window close failed — grant Accessibility to WebRust"
        );
        false
    })
}

fn close_via_ax(pid: i32, window_id: u32, title: Option<&str>) -> bool {
    let Some(window) = find_ax_window(pid, window_id, title) else {
        return false;
    };
    let raise = AxName::new("AXRaise");
    let close_attr = AxName::new("AXCloseButton");
    let press = AxName::new("AXPress");
    unsafe {
        // Raise target first so the close button is live.
        let _ = AXUIElementPerformAction(window, raise.r());

        let mut close_ref: CFTypeRef = ptr::null();
        let err = AXUIElementCopyAttributeValue(window, close_attr.r(), &mut close_ref);
        if err != AX_OK || close_ref.is_null() {
            CFRelease(window as CFTypeRef);
            return false;
        }
        let close_btn = close_ref as AXUIElementRef;
        let ok = AXUIElementPerformAction(close_btn, press.r()) == AX_OK;
        CFRelease(close_ref);
        CFRelease(window as CFTypeRef);
        ok
    }
}

fn close_via_cmd_w(pid: i32, window_id: u32, title: Option<&str>) -> bool {
    let raise = AxName::new("AXRaise");
    // Raise the exact window when possible, else front the process.
    if let Some(window) = find_ax_window(pid, window_id, title) {
        unsafe {
            let _ = AXUIElementPerformAction(window, raise.r());
            CFRelease(window as CFTypeRef);
        }
    } else {
        unsafe {
            let app = AXUIElementCreateApplication(pid);
            if !app.is_null() {
                let _ = AXUIElementPerformAction(app, raise.r());
                CFRelease(app as CFTypeRef);
            }
        }
    }
    std::thread::sleep(std::time::Duration::from_millis(40));
    inject_cmd_w()
}

fn inject_cmd_w() -> bool {
    let Ok(source) = CGEventSource::new(CGEventSourceStateID::HIDSystemState) else {
        return false;
    };
    let Ok(down) = CGEvent::new_keyboard_event(source.clone(), KEY_W, true) else {
        return false;
    };
    down.set_flags(CGEventFlags::CGEventFlagCommand);
    down.post(CGEventTapLocation::HID);

    let Ok(up) = CGEvent::new_keyboard_event(source, KEY_W, false) else {
        return false;
    };
    up.set_flags(CGEventFlags::CGEventFlagCommand);
    up.post(CGEventTapLocation::HID);
    true
}

/// Find AX window: exact CGWindowID first, then title. Never returns a random window
/// (same rule as WebDock AccessibilityHelpers.findWindow).
///
/// Returned ref is +1 retained — caller must `CFRelease`.
fn find_ax_window(pid: i32, window_id: u32, title: Option<&str>) -> Option<AXUIElementRef> {
    unsafe {
        let app = AXUIElementCreateApplication(pid);
        if app.is_null() {
            return None;
        }

        let windows_attr = AxName::new("AXWindows");
        let mut windows_ref: CFTypeRef = ptr::null();
        let err = AXUIElementCopyAttributeValue(app, windows_attr.r(), &mut windows_ref);
        CFRelease(app as CFTypeRef);
        if err != AX_OK || windows_ref.is_null() {
            return None;
        }

        let array = windows_ref as CFArrayRef;
        let count = CFArrayGetCount(array);
        let mut by_title: Option<AXUIElementRef> = None;
        let title_attr = AxName::new("AXTitle");

        for i in 0..count {
            let item = CFArrayGetValueAtIndex(array, i) as AXUIElementRef;
            if item.is_null() {
                continue;
            }
            // Match CGWindowID via private API (most reliable).
            let mut wid: u32 = 0;
            if _AXUIElementGetWindow(item, &mut wid) == AX_OK && wid == window_id && window_id != 0
            {
                let retained = CFRetain(item as CFTypeRef) as AXUIElementRef;
                CFRelease(windows_ref);
                return Some(retained);
            }
            if by_title.is_none() {
                if let Some(t) = title {
                    if !t.is_empty() && ax_window_title(item, title_attr.r()).as_deref() == Some(t)
                    {
                        by_title = Some(CFRetain(item as CFTypeRef) as AXUIElementRef);
                    }
                }
            }
        }

        CFRelease(windows_ref);
        by_title
    }
}

fn ax_window_title(element: AXUIElementRef, title_attr: CFStringRef) -> Option<String> {
    unsafe {
        let mut title_ref: CFTypeRef = ptr::null();
        let err = AXUIElementCopyAttributeValue(element, title_attr, &mut title_ref);
        if err != AX_OK || title_ref.is_null() {
            return None;
        }
        // wrap_under_create_rule: we own the +1 from CopyAttributeValue
        let s = CFString::wrap_under_create_rule(title_ref as *const _);
        Some(s.to_string())
    }
}
