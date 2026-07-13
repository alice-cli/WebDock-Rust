//! Platform trait surface + **real** OS backends for deployment.
//!
//! Application code should only call [`current`] / [`PlatformServices`].
//! Mock backends exist solely for unit tests (`feature = "mock"`).

mod mock;
mod native;
mod route;
mod traits;
mod types;

pub use mock::MockPlatform;
pub use route::{
    display_id_from_route, display_route, is_display_route, window_id_from_route, window_route,
    DISPLAY_ROUTE_FLAG,
};
pub use traits::*;
pub use types::*;

/// Host clipboard helpers (remote → browser sync).
pub mod clipboard {
    #[cfg(not(feature = "mock"))]
    #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
    pub use crate::native::{
        clipboard_change_count as change_count, clipboard_read as read_string,
        clipboard_read_after_change as read_string_after_change,
    };

    #[cfg(any(
        feature = "mock",
        not(any(target_os = "macos", target_os = "windows", target_os = "linux"))
    ))]
    pub fn change_count() -> i64 {
        0
    }
    #[cfg(any(
        feature = "mock",
        not(any(target_os = "macos", target_os = "windows", target_os = "linux"))
    ))]
    pub fn read_string() -> String {
        String::new()
    }
    #[cfg(any(
        feature = "mock",
        not(any(target_os = "macos", target_os = "windows", target_os = "linux"))
    ))]
    pub fn read_string_after_change(_from: i64, _timeout_ms: u64) -> String {
        String::new()
    }
}

/// Session / lock-screen.
pub fn is_screen_locked() -> bool {
    #[cfg(not(feature = "mock"))]
    #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
    {
        return native::is_screen_locked();
    }
    #[cfg(any(
        feature = "mock",
        not(any(target_os = "macos", target_os = "windows", target_os = "linux"))
    ))]
    {
        false
    }
}

/// Resolve owning process id for a capture route (window list / close).
pub fn native_pid_for_route(id: webdock_protocol::RouteId) -> Option<i32> {
    #[cfg(feature = "mock")]
    {
        let _ = id;
        return None;
    }
    #[cfg(not(feature = "mock"))]
    {
        #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
        {
            return native::capture_pid(id);
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        {
            let _ = id;
            None
        }
    }
}

/// Bundle of all platform services used by the server.
pub struct PlatformServices {
    pub capture: std::sync::Arc<dyn CaptureBackend>,
    pub input: std::sync::Arc<dyn InputInjector>,
    pub windows: std::sync::Arc<dyn WindowControl>,
    pub apps: std::sync::Arc<dyn AppCatalog>,
    pub metrics: std::sync::Arc<dyn MetricsProvider>,
    pub ime: std::sync::Arc<dyn ImeControl>,
    pub power: std::sync::Arc<dyn PowerControl>,
    pub platform_name: &'static str,
}

/// Construct services for the current OS (**real backends** in production).
pub fn current() -> PlatformServices {
    #[cfg(feature = "mock")]
    {
        return MockPlatform::services();
    }
    #[cfg(not(feature = "mock"))]
    {
        #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
        {
            return native::services();
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        {
            tracing::error!("unsupported OS — falling back to mock (not for production)");
            MockPlatform::services()
        }
    }
}
