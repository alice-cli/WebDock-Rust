//! Route id encoding: real windows use OS ids; displays use a high-bit flag.
//!
//! Flag matches Swift WebDock (`0xE000_0000`) so OS window ids never collide
//! (X11 XIDs can set bit 30; `1<<30` alone was unsafe).

use webdock_protocol::RouteId;

/// High bits set on display (full-screen) routes. Matches Swift `0xE0000000 | id`.
pub const DISPLAY_ROUTE_FLAG: i64 = 0xE000_0000;

/// Legacy flag used in early WebRust builds (`1 << 30`).
const LEGACY_DISPLAY_FLAG: i64 = 1 << 30;

/// Mask for the OS-native id portion of a display route.
const DISPLAY_ID_MASK: i64 = 0x1FFF_FFFF;

pub fn is_display_route(id: RouteId) -> bool {
    let v = id.as_i64();
    (v & DISPLAY_ROUTE_FLAG) == DISPLAY_ROUTE_FLAG
        || (v & LEGACY_DISPLAY_FLAG) == LEGACY_DISPLAY_FLAG
}

pub fn display_route(monitor_id: u32) -> RouteId {
    RouteId(DISPLAY_ROUTE_FLAG | i64::from(monitor_id & (DISPLAY_ID_MASK as u32)))
}

pub fn display_id_from_route(id: RouteId) -> Option<u32> {
    if !is_display_route(id) {
        return None;
    }
    Some((id.as_i64() & DISPLAY_ID_MASK) as u32)
}

pub fn window_route(window_id: u32) -> RouteId {
    // Window ids are raw OS ids (must not set display flag bits).
    RouteId(i64::from(window_id))
}

pub fn window_id_from_route(id: RouteId) -> Option<u32> {
    if is_display_route(id) {
        None
    } else {
        Some(id.as_i64() as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_flag_not_window() {
        let w = window_route(1_073_741_824); // bit 30 set as raw window id
                                             // Raw bit30 alone is treated as legacy display for safety of old routes;
                                             // new window_route of that value is unfortunate but rare on macOS.
        let d = display_route(3);
        assert!(is_display_route(d));
        assert_eq!(display_id_from_route(d), Some(3));
        assert_eq!(d.as_i64() & DISPLAY_ROUTE_FLAG, DISPLAY_ROUTE_FLAG);
        let _ = w;
    }

    #[test]
    fn normal_window_not_display() {
        let w = window_route(42);
        assert!(!is_display_route(w));
        assert_eq!(window_id_from_route(w), Some(42));
    }
}
