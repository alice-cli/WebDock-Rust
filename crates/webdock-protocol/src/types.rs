use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Window / display route id (CGWindowID-compatible, may exceed i32 for display routes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RouteId(pub i64);

impl RouteId {
    pub fn as_i64(self) -> i64 {
        self.0
    }
}

/// Normalized coordinate in `0.0..=1.0` relative to target bounds.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Fraction(pub f64);

impl Fraction {
    pub fn clamp(self) -> Self {
        Self(self.0.clamp(0.0, 1.0))
    }

    pub fn get(self) -> f64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(transparent)]
pub struct MouseButton(pub i32);

#[derive(Debug, Error)]
#[allow(dead_code)]
pub enum ProtocolError {
    #[error("invalid json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("unknown message type")]
    UnknownType,
}
