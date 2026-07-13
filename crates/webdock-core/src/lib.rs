//! Platform-independent domain logic for WebDock.

pub mod config;
pub mod input_seat;
pub mod tuning;

pub use config::AppConfig;
pub use input_seat::{InputKind, InputSeat, SeatResult};
