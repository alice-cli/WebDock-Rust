//! Shared WebSocket JSON message types and binary frame layout.
//!
//! Contract is frozen to match the Swift WebDock client (`AppJS`).
//! Keep `#[serde(rename_all = "camelCase")]` and `type` tags stable.

mod binary;
mod client;
mod server;
mod types;

pub use binary::{
    FrameError, FrameHeader, FrameType, H264Header, FRAME_HEADER_LEN, H264_HEADER_LEN,
    H264_TYPE_BYTE,
};
pub use client::ClientMessage;
pub use server::{
    AppInfo, ClientInfo, H264Config, ImePayload, MetricsPayload, ServerMessage, WindowInfo,
};
pub use types::{Fraction, MouseButton, RouteId};

/// Protocol version advertised in capability negotiation.
pub const PROTOCOL_VERSION: u32 = 1;
