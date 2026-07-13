use serde::{Deserialize, Serialize};

use crate::types::RouteId;

/// Messages sent from host → browser client.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ServerMessage {
    #[serde(rename = "windows")]
    Windows { list: Vec<WindowInfo> },

    #[serde(rename = "apps")]
    Apps { list: Vec<AppInfo> },

    #[serde(rename = "clients")]
    Clients { list: Vec<ClientInfo> },

    #[serde(rename = "metrics")]
    Metrics {
        #[serde(flatten)]
        payload: MetricsPayload,
    },

    #[serde(rename = "ime")]
    Ime {
        #[serde(flatten)]
        payload: ImePayload,
    },

    #[serde(rename = "clipboard")]
    Clipboard {
        value: String,
        empty: bool,
        force: bool,
    },

    #[serde(rename = "inputBusy")]
    InputBusy { message: String, who: String },

    #[serde(rename = "h264config")]
    H264Config {
        #[serde(flatten)]
        config: H264Config,
    },

    /// Capability advertisement (protocol extension; clients may ignore).
    #[serde(rename = "hello")]
    Hello {
        version: u32,
        #[serde(default)]
        capabilities: Vec<String>,
        platform: String,
    },
}

/// Window row for the browser sidebar.
/// Field names match Swift WebDock / `app.js` (`name`, `path`, `icon`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowInfo {
    pub id: RouteId,
    pub pid: i32,
    /// App name (client field is `name`, not `app`).
    pub name: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub w: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub h: Option<i32>,
    /// `data:image/png;base64,...` — only sent when new to the peer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub name: String,
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientInfo {
    pub id: String,
    pub ip: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub viewing: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_id: Option<RouteId>,
}

/// Host metrics for the web UI header — field names match Swift + `app.js` (`applyMetrics`).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MetricsPayload {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ram: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disk: Option<f64>,
    /// Used RAM in GB (number — client `fmtGB` / tooltips).
    #[serde(default, rename = "ramUsedGB", skip_serializing_if = "Option::is_none")]
    pub ram_used_gb: Option<f64>,
    #[serde(
        default,
        rename = "ramTotalGB",
        skip_serializing_if = "Option::is_none"
    )]
    pub ram_total_gb: Option<f64>,
    #[serde(
        default,
        rename = "diskUsedGB",
        skip_serializing_if = "Option::is_none"
    )]
    pub disk_used_gb: Option<f64>,
    #[serde(
        default,
        rename = "diskTotalGB",
        skip_serializing_if = "Option::is_none"
    )]
    pub disk_total_gb: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImePayload {
    pub korean: bool,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct H264Config {
    pub codec: String,
    pub width: u32,
    pub height: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl ServerMessage {
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    pub fn from_json(s: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_use_name_field() {
        let msg = ServerMessage::Windows {
            list: vec![WindowInfo {
                id: RouteId(1),
                pid: 100,
                name: "Safari".into(),
                title: "Home".into(),
                path: Some("/Applications/Safari.app".into()),
                w: Some(800),
                h: Some(600),
                icon: Some("data:image/png;base64,xx".into()),
                icon_key: Some("/Applications/Safari.app".into()),
            }],
        };
        let s = msg.to_json().unwrap();
        assert!(s.contains(r#""name":"Safari""#));
        assert!(s.contains(r#""path":"/Applications/Safari.app""#));
        assert!(s.contains(r#""icon":"#));
        assert!(!s.contains(r#""app":"#));
    }
}
