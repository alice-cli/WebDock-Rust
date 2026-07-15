//! Production platform backends (xcap + enigo + OS helpers).

mod apps;
#[cfg(target_os = "macos")]
mod ax_close;
mod capture;
#[cfg(target_os = "macos")]
mod cg_mouse;
#[cfg(target_os = "macos")]
mod cg_type;
mod clipboard;
mod icons;
mod ime;
mod input;
mod metrics;
mod permissions;
mod power;
mod session;
mod window_ctl;

pub use clipboard::{
    change_count as clipboard_change_count, read_string as clipboard_read,
    read_string_after_change as clipboard_read_after_change,
};
pub use session::is_screen_locked;

use std::sync::Arc;

use crate::PlatformServices;

pub use capture::NativeCapture;
pub use input::NativeInput;
pub use window_ctl::NativeWindowControl;

/// PID for a window route (used by server close handler).
pub fn capture_pid(id: webdock_protocol::RouteId) -> Option<i32> {
    capture::pid_for_route(id)
}

/// Build real OS services for deployment.
pub fn services() -> PlatformServices {
    permissions::log_requirements();

    let capture = Arc::new(NativeCapture::new());
    let windows = Arc::new(NativeWindowControl::new());
    let input = Arc::new(NativeInput::new(windows.clone()));

    PlatformServices {
        capture,
        input,
        windows,
        apps: Arc::new(apps::NativeApps),
        metrics: Arc::new(metrics::NativeMetrics::new()),
        ime: Arc::new(ime::NativeIme),
        power: Arc::new(power::NativePower::new()),
        platform_name: platform_name(),
    }
}

fn platform_name() -> &'static str {
    #[cfg(target_os = "macos")]
    {
        "macos"
    }
    #[cfg(target_os = "windows")]
    {
        "windows"
    }
    #[cfg(target_os = "linux")]
    {
        "linux"
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        "unknown"
    }
}
