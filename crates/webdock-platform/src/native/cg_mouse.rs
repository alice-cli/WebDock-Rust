//! CGEvent mouse injection (macOS).
//!
//! Matches original Swift `MouseInjection`: private event source, clickState,
//! pressure, and move-only-when-needed so multi-click works.

#![cfg(target_os = "macos")]

use std::sync::Mutex;

use core_graphics::event::{CGEvent, CGEventTapLocation, CGEventType, CGMouseButton, EventField};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use core_graphics::geometry::CGPoint;
use tracing::debug;

use crate::traits::InputError;
use crate::types::MousePhase;

static LAST_CURSOR: Mutex<(f64, f64)> = Mutex::new((-1.0, -1.0));
const CURSOR_EPS: f64 = 0.5;

/// Inject mouse at absolute screen coords (top-left origin, CGEvent space).
pub fn inject(
    phase: MousePhase,
    x: f64,
    y: f64,
    button: i32,
    click_count: i32,
) -> Result<(), InputError> {
    let is_right = button == 2;
    let is_middle = button == 1;
    let count = click_count.clamp(1, 3) as i64;
    let point = CGPoint::new(x, y);

    let source = CGEventSource::new(CGEventSourceStateID::Private)
        .map_err(|_| InputError::Other("CGEventSource (private) failed".into()))?;

    let (etype, mouse_btn) = event_type(phase, is_right, is_middle);

    // Place cursor before down only when it actually moved (multi-click).
    if matches!(phase, MousePhase::Down) {
        let need_move = {
            let last = LAST_CURSOR.lock().unwrap_or_else(|e| e.into_inner());
            let dx = (point.x - last.0).abs();
            let dy = (point.y - last.1).abs();
            last.0 < 0.0 || dx > CURSOR_EPS || dy > CURSOR_EPS
        };
        if need_move {
            let moved =
                CGEvent::new_mouse_event(source.clone(), CGEventType::MouseMoved, point, mouse_btn)
                    .map_err(|_| InputError::Other("CGEvent mouseMoved failed".into()))?;
            moved.post(CGEventTapLocation::HID);
        }
    }

    let event = CGEvent::new_mouse_event(source, etype, point, mouse_btn)
        .map_err(|_| InputError::Other("CGEvent mouse event failed".into()))?;

    let btn_num: i64 = if is_right {
        1
    } else if is_middle {
        2
    } else {
        0
    };
    event.set_integer_value_field(EventField::MOUSE_EVENT_BUTTON_NUMBER, btn_num);
    event.set_integer_value_field(EventField::MOUSE_EVENT_CLICK_STATE, count);
    event.set_double_value_field(
        EventField::MOUSE_EVENT_PRESSURE,
        if matches!(phase, MousePhase::Up) {
            0.0
        } else {
            1.0
        },
    );

    event.post(CGEventTapLocation::HID);

    if let Ok(mut last) = LAST_CURSOR.lock() {
        *last = (point.x, point.y);
    }

    debug!(?phase, x, y, button, count, "CGEvent mouse");
    Ok(())
}

fn event_type(phase: MousePhase, is_right: bool, is_middle: bool) -> (CGEventType, CGMouseButton) {
    if is_middle {
        let t = match phase {
            MousePhase::Down => CGEventType::OtherMouseDown,
            MousePhase::Up => CGEventType::OtherMouseUp,
            MousePhase::Move => CGEventType::OtherMouseDragged,
        };
        return (t, CGMouseButton::Center);
    }
    if is_right {
        let t = match phase {
            MousePhase::Down => CGEventType::RightMouseDown,
            MousePhase::Up => CGEventType::RightMouseUp,
            MousePhase::Move => CGEventType::RightMouseDragged,
        };
        return (t, CGMouseButton::Right);
    }
    let t = match phase {
        MousePhase::Down => CGEventType::LeftMouseDown,
        MousePhase::Up => CGEventType::LeftMouseUp,
        MousePhase::Move => CGEventType::LeftMouseDragged,
    };
    (t, CGMouseButton::Left)
}
