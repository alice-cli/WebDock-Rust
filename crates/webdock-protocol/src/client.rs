use serde::{Deserialize, Serialize};

use crate::types::{Fraction, MouseButton, RouteId};

/// Messages sent from the browser client → host.
///
/// Field names match the Swift/JS client (`type` tag, camelCase where used).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ClientMessage {
    #[serde(rename = "select")]
    Select { id: RouteId },

    #[serde(rename = "down")]
    Down {
        x: Fraction,
        y: Fraction,
        #[serde(default)]
        button: MouseButton,
        #[serde(default = "default_count")]
        count: i32,
    },

    #[serde(rename = "move")]
    Move {
        x: Fraction,
        y: Fraction,
        #[serde(default)]
        button: MouseButton,
        #[serde(default = "default_count")]
        count: i32,
    },

    #[serde(rename = "up")]
    Up {
        x: Fraction,
        y: Fraction,
        #[serde(default)]
        button: MouseButton,
        #[serde(default = "default_count")]
        count: i32,
    },

    #[serde(rename = "click")]
    Click { x: Fraction, y: Fraction },

    #[serde(rename = "scroll")]
    Scroll {
        x: Fraction,
        y: Fraction,
        #[serde(default)]
        dx: f64,
        #[serde(default)]
        dy: f64,
    },

    #[serde(rename = "key")]
    Key {
        code: String,
        #[serde(default)]
        meta: bool,
        #[serde(default)]
        ctrl: bool,
        #[serde(default)]
        shift: bool,
        #[serde(default)]
        alt: bool,
        /// Client-expected IME (Korean) — UI tracking only.
        #[serde(default, rename = "ime")]
        ime: Option<bool>,
    },

    #[serde(rename = "text")]
    Text {
        value: String,
        #[serde(default)]
        replace: u32,
    },

    #[serde(rename = "ime")]
    Ime {
        #[serde(default)]
        korean: Option<bool>,
    },

    #[serde(rename = "imeState")]
    ImeState,

    #[serde(rename = "imeHeal")]
    ImeHeal {
        #[serde(default)]
        korean: Option<bool>,
        #[serde(default)]
        hard: bool,
    },

    #[serde(rename = "resize")]
    Resize { w: i32, h: i32 },

    #[serde(rename = "quality")]
    Quality { value: f64 },

    #[serde(rename = "format")]
    Format { value: String },

    #[serde(rename = "preset")]
    Preset { value: String },

    #[serde(rename = "keyframe")]
    Keyframe { id: RouteId },

    #[serde(rename = "stats")]
    Stats {
        #[serde(default)]
        fps: Option<f64>,
        #[serde(default)]
        queue: Option<i32>,
        #[serde(default)]
        drops: Option<i32>,
        #[serde(default)]
        pressure: Option<i32>,
    },

    #[serde(rename = "fps")]
    Fps { value: i32 },

    #[serde(rename = "apps")]
    Apps,

    #[serde(rename = "launch")]
    Launch {
        path: String,
        #[serde(default, rename = "newInstance")]
        new_instance: bool,
    },

    #[serde(rename = "close")]
    Close {
        id: RouteId,
        #[serde(default)]
        pid: Option<i32>,
        #[serde(default)]
        title: Option<String>,
    },

    #[serde(rename = "quit")]
    Quit { pid: i32 },

    #[serde(rename = "refresh")]
    Refresh,

    #[serde(rename = "clipAuto")]
    ClipAuto { value: bool },

    #[serde(rename = "clipboardGet")]
    ClipboardGet,
}

fn default_count() -> i32 {
    1
}

impl ClientMessage {
    pub fn from_json(s: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(s)
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_select() {
        let m = ClientMessage::from_json(r#"{"type":"select","id":42}"#).unwrap();
        match m {
            ClientMessage::Select { id } => assert_eq!(id.0, 42),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn parse_key() {
        let m = ClientMessage::from_json(
            r#"{"type":"key","code":"KeyA","meta":true,"ctrl":false,"shift":false,"alt":false}"#,
        )
        .unwrap();
        match m {
            ClientMessage::Key { code, meta, .. } => {
                assert_eq!(code, "KeyA");
                assert!(meta);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn parse_move() {
        let m = ClientMessage::from_json(r#"{"type":"move","x":0.5,"y":0.25,"button":0}"#).unwrap();
        match m {
            ClientMessage::Move { x, y, .. } => {
                assert!((x.get() - 0.5).abs() < 1e-9);
                assert!((y.get() - 0.25).abs() < 1e-9);
            }
            _ => panic!("wrong variant"),
        }
    }
}
