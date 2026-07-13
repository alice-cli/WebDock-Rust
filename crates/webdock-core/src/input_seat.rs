//! Exclusive input seat — one browser controls input at a time (with drag grace).

use std::collections::HashMap;
use std::time::{Duration, Instant};

use parking_lot::Mutex;

use crate::tuning::{DRAG_GRACE_SECS, INPUT_BUSY_NOTIFY_INTERVAL_SECS, INPUT_SEAT_TTL_SECS};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputKind {
    Move,
    Down,
    Up,
    Scroll,
    Key,
    Text,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SeatResult {
    Allowed,
    Busy { who: String },
}

struct SeatState {
    owner: Option<u64>,
    label: String,
    last: Instant,
    /// After down, owner may finish the gesture even if idle TTL would expire.
    drag_until: Option<Instant>,
    /// Rate-limit inputBusy spam per peer.
    last_busy_notify: HashMap<u64, Instant>,
}

/// Thread-safe exclusive input arbitration.
pub struct InputSeat {
    inner: Mutex<SeatState>,
    ttl: Duration,
    drag_grace: Duration,
    busy_interval: Duration,
}

impl Default for InputSeat {
    fn default() -> Self {
        Self::new()
    }
}

impl InputSeat {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(SeatState {
                owner: None,
                label: String::new(),
                last: Instant::now(),
                drag_until: None,
                last_busy_notify: HashMap::new(),
            }),
            ttl: Duration::from_secs_f64(INPUT_SEAT_TTL_SECS),
            drag_grace: Duration::from_secs_f64(DRAG_GRACE_SECS),
            busy_interval: Duration::from_secs_f64(INPUT_BUSY_NOTIFY_INTERVAL_SECS),
        }
    }

    pub fn acquire(&self, peer_id: u64, kind: InputKind, label: &str) -> SeatResult {
        let mut g = self.inner.lock();
        let now = Instant::now();

        // Expire idle seat (but keep seat during active drag grace).
        if let Some(owner) = g.owner {
            let in_drag = g.drag_until.map(|t| now <= t).unwrap_or(false);
            let idle = now.duration_since(g.last) > self.ttl;
            if idle && !in_drag {
                g.owner = None;
                g.drag_until = None;
            } else if owner != peer_id {
                return SeatResult::Busy {
                    who: g.label.clone(),
                };
            }
        }

        // Same owner (or free seat).
        g.owner = Some(peer_id);
        g.label = label.to_string();
        g.last = now;
        if kind == InputKind::Down {
            g.drag_until = Some(now + self.drag_grace);
        }
        if kind == InputKind::Up {
            g.drag_until = None;
        }
        SeatResult::Allowed
    }

    /// Whether we should notify this peer about a busy seat (rate-limited).
    pub fn should_notify_busy(&self, peer_id: u64) -> bool {
        let mut g = self.inner.lock();
        let now = Instant::now();
        if let Some(last) = g.last_busy_notify.get(&peer_id) {
            if now.duration_since(*last) < self.busy_interval {
                return false;
            }
        }
        g.last_busy_notify.insert(peer_id, now);
        true
    }

    pub fn release(&self, peer_id: u64) {
        let mut g = self.inner.lock();
        if g.owner == Some(peer_id) {
            g.owner = None;
            g.drag_until = None;
        }
        g.last_busy_notify.remove(&peer_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exclusive() {
        let seat = InputSeat::new();
        assert_eq!(seat.acquire(1, InputKind::Key, "a"), SeatResult::Allowed);
        assert!(matches!(
            seat.acquire(2, InputKind::Key, "b"),
            SeatResult::Busy { .. }
        ));
        seat.release(1);
        assert_eq!(seat.acquire(2, InputKind::Key, "b"), SeatResult::Allowed);
    }

    #[test]
    fn same_owner_drag_continues() {
        let seat = InputSeat::new();
        assert_eq!(seat.acquire(1, InputKind::Down, "a"), SeatResult::Allowed);
        assert_eq!(seat.acquire(1, InputKind::Move, "a"), SeatResult::Allowed);
        assert_eq!(seat.acquire(1, InputKind::Up, "a"), SeatResult::Allowed);
        assert!(matches!(
            seat.acquire(2, InputKind::Down, "b"),
            SeatResult::Busy { .. }
        ));
    }

    #[test]
    fn busy_notify_rate_limit() {
        let seat = InputSeat::new();
        assert!(seat.should_notify_busy(1));
        assert!(!seat.should_notify_busy(1));
    }
}
