//! Display wake / no-sleep for remote sessions.
//!
//! Policy (matches Swift WebDock):
//! - Server idle → monitor may sleep.
//! - ≥1 authenticated WS client → hold no-sleep assertion.
//! - Last client leaves → release.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;

use tracing::{info, warn};

use crate::traits::*;

pub struct NativePower {
    hold: AtomicU32,
    /// Long-running `caffeinate` child while sessions active (macOS fallback path).
    child: Mutex<Option<std::process::Child>>,
    #[cfg(target_os = "macos")]
    assertion_id: Mutex<u32>,
}

impl NativePower {
    pub fn new() -> Self {
        Self {
            hold: AtomicU32::new(0),
            child: Mutex::new(None),
            #[cfg(target_os = "macos")]
            assertion_id: Mutex::new(0),
        }
    }
}

impl Default for NativePower {
    fn default() -> Self {
        Self::new()
    }
}

impl PowerControl for NativePower {
    fn wake_display(&self) -> Result<(), PlatformError> {
        #[cfg(target_os = "macos")]
        {
            let _ = std::process::Command::new("caffeinate")
                .args(["-u", "-t", "3"])
                .spawn();
            return Ok(());
        }
        #[cfg(target_os = "windows")]
        {
            // ES_DISPLAY_REQUIRED pulse via SetThreadExecutionState would be better;
            // powercfg change is heavy-handed — use caffeinate-equivalent soft nudge.
            let _ = std::process::Command::new("powershell")
                .args([
                    "-NoProfile",
                    "-Command",
                    "[void][System.Reflection.Assembly]::LoadWithPartialName('System.Windows.Forms'); [System.Windows.Forms.Cursor]::Position = [System.Windows.Forms.Cursor]::Position",
                ])
                .status();
            return Ok(());
        }
        #[cfg(target_os = "linux")]
        {
            let _ = std::process::Command::new("xset")
                .args(["dpms", "force", "on"])
                .status();
            return Ok(());
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        {
            Ok(())
        }
    }

    fn keep_awake(&self, enable: bool) -> Result<(), PlatformError> {
        if enable {
            let prev = self.hold.fetch_add(1, Ordering::SeqCst);
            if prev == 0 {
                self.acquire_hold()?;
                info!("display: session retain (clients active)");
                let _ = self.wake_display();
            }
        } else {
            loop {
                let cur = self.hold.load(Ordering::SeqCst);
                if cur == 0 {
                    break;
                }
                match self
                    .hold
                    .compare_exchange(cur, cur - 1, Ordering::SeqCst, Ordering::SeqCst)
                {
                    Ok(1) => {
                        self.release_hold();
                        info!("display: session release (no clients — sleep allowed)");
                        break;
                    }
                    Ok(_) => break,
                    Err(_) => continue,
                }
            }
        }
        Ok(())
    }
}

impl NativePower {
    fn acquire_hold(&self) -> Result<(), PlatformError> {
        #[cfg(target_os = "macos")]
        {
            if macos_create_assertion(&self.assertion_id) {
                return Ok(());
            }
            // Fallback: long-running caffeinate -dims
            let mut g = self.child.lock().unwrap_or_else(|e| e.into_inner());
            if g.is_none() {
                match std::process::Command::new("caffeinate")
                    .args(["-dims"])
                    .spawn()
                {
                    Ok(c) => {
                        info!("display: caffeinate -dims started");
                        *g = Some(c);
                    }
                    Err(e) => warn!(error = %e, "caffeinate spawn failed"),
                }
            }
            return Ok(());
        }
        #[cfg(target_os = "windows")]
        {
            // ES_CONTINUOUS | ES_DISPLAY_REQUIRED | ES_SYSTEM_REQUIRED
            #[link(name = "kernel32")]
            extern "system" {
                fn SetThreadExecutionState(es_flags: u32) -> u32;
            }
            const ES_CONTINUOUS: u32 = 0x8000_0000;
            const ES_SYSTEM_REQUIRED: u32 = 0x0000_0001;
            const ES_DISPLAY_REQUIRED: u32 = 0x0000_0002;
            unsafe {
                SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED | ES_DISPLAY_REQUIRED);
            }
            return Ok(());
        }
        #[cfg(target_os = "linux")]
        {
            let mut g = self.child.lock().unwrap_or_else(|e| e.into_inner());
            if g.is_none() {
                // Prefer systemd-inhibit if available
                if let Ok(c) = std::process::Command::new("systemd-inhibit")
                    .args([
                        "--what=idle:sleep:handle-lid-switch",
                        "--who=WebRust",
                        "--why=Remote session",
                        "--mode=block",
                        "sleep",
                        "infinity",
                    ])
                    .spawn()
                {
                    *g = Some(c);
                }
            }
            return Ok(());
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        {
            Ok(())
        }
    }

    fn release_hold(&self) {
        #[cfg(target_os = "macos")]
        {
            macos_release_assertion(&self.assertion_id);
        }
        #[cfg(target_os = "windows")]
        {
            #[link(name = "kernel32")]
            extern "system" {
                fn SetThreadExecutionState(es_flags: u32) -> u32;
            }
            const ES_CONTINUOUS: u32 = 0x8000_0000;
            unsafe {
                SetThreadExecutionState(ES_CONTINUOUS);
            }
        }
        let mut g = self.child.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(mut c) = g.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }
}

#[cfg(target_os = "macos")]
fn macos_create_assertion(slot: &Mutex<u32>) -> bool {
    // IOPMAssertionCreateWithName — NoDisplaySleep
    type IOPMAssertionID = u32;
    #[link(name = "IOKit", kind = "framework")]
    extern "C" {
        fn IOPMAssertionCreateWithName(
            assertion_type: *const std::ffi::c_void, // CFStringRef
            assertion_level: u32,
            assertion_name: *const std::ffi::c_void, // CFStringRef
            assertion_id: *mut IOPMAssertionID,
        ) -> i32;
    }
    use core_foundation::base::TCFType;
    use core_foundation::string::CFString;

    const K_IOPM_ASSERTION_LEVEL_ON: u32 = 255;
    // kIOPMAssertionTypeNoDisplaySleep
    let ty = CFString::from_static_string("NoDisplaySleepAssertion");
    let name = CFString::from_static_string("WebRust remote session");
    let mut id: u32 = 0;
    let kr = unsafe {
        IOPMAssertionCreateWithName(
            ty.as_concrete_TypeRef() as *const _,
            K_IOPM_ASSERTION_LEVEL_ON,
            name.as_concrete_TypeRef() as *const _,
            &mut id,
        )
    };
    // kIOReturnSuccess == 0
    if kr == 0 && id != 0 {
        *slot.lock().unwrap_or_else(|e| e.into_inner()) = id;
        info!(id, "display: NoDisplaySleep assertion on");
        true
    } else {
        // Try PreventUserIdleDisplaySleep
        let ty2 = CFString::from_static_string("PreventUserIdleDisplaySleep");
        let mut id2: u32 = 0;
        let kr2 = unsafe {
            IOPMAssertionCreateWithName(
                ty2.as_concrete_TypeRef() as *const _,
                K_IOPM_ASSERTION_LEVEL_ON,
                name.as_concrete_TypeRef() as *const _,
                &mut id2,
            )
        };
        if kr2 == 0 && id2 != 0 {
            *slot.lock().unwrap_or_else(|e| e.into_inner()) = id2;
            info!(id = id2, "display: PreventUserIdleDisplaySleep on");
            true
        } else {
            warn!(kr, kr2, "IOPM assertion failed — will try caffeinate");
            false
        }
    }
}

#[cfg(target_os = "macos")]
fn macos_release_assertion(slot: &Mutex<u32>) {
    #[link(name = "IOKit", kind = "framework")]
    extern "C" {
        fn IOPMAssertionRelease(assertion_id: u32) -> i32;
    }
    let mut g = slot.lock().unwrap_or_else(|e| e.into_inner());
    if *g != 0 {
        unsafe {
            IOPMAssertionRelease(*g);
        }
        info!(id = *g, "display: assertion released");
        *g = 0;
    }
}
