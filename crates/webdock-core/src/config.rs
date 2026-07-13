use std::path::PathBuf;

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::tuning::DEFAULT_PORT;

/// Persistent WebDock settings (JSON; migrates from Swift `config.ini` once).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    /// Whether the embedded server should run. Default off.
    #[serde(default)]
    pub server_enabled: bool,
    #[serde(default = "default_port")]
    pub port: u16,
    /// Bind all interfaces when true; otherwise loopback only.
    #[serde(default)]
    pub allow_lan: bool,
    /// Shared secret. Empty = no token required.
    #[serde(default)]
    pub token: String,
    #[serde(default)]
    pub allowed_domains: Vec<String>,
    #[serde(default)]
    pub ip_allowlist_enabled: bool,
    #[serde(default)]
    pub allowed_ips: Vec<String>,
    /// When true, honor `X-Forwarded-For` / `X-Real-IP` / `CF-Connecting-IP`.
    /// Default **false** — only trust proxy headers behind a known reverse proxy.
    #[serde(default)]
    pub trust_proxy_headers: bool,
}

fn default_port() -> u16 {
    DEFAULT_PORT
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            server_enabled: false,
            port: DEFAULT_PORT,
            allow_lan: false,
            token: String::new(),
            allowed_domains: Vec::new(),
            ip_allowlist_enabled: false,
            allowed_ips: Vec::new(),
            trust_proxy_headers: false,
        }
    }
}

impl AppConfig {
    pub fn has_token(&self) -> bool {
        !self.token.is_empty()
    }

    pub fn generate_token(bytes: usize) -> String {
        let mut buf = vec![0u8; bytes];
        rand::thread_rng().fill_bytes(&mut buf);
        URL_SAFE_NO_PAD.encode(buf)
    }

    pub fn support_dir() -> PathBuf {
        dirs_support()
    }

    pub fn config_path() -> PathBuf {
        Self::support_dir().join("config.json")
    }

    pub fn legacy_ini_path() -> PathBuf {
        Self::support_dir().join("config.ini")
    }

    pub fn load_or_default() -> Self {
        match Self::load() {
            Ok(c) => c,
            Err(_) => {
                // Prefer WebRust INI/JSON; optionally seed once from legacy WebDock config.
                if let Some(c) = Self::try_migrate_ini() {
                    let _ = c.save();
                    return c;
                }
                if let Some(c) = Self::try_migrate_from_webdock() {
                    let _ = c.save();
                    return c;
                }
                Self::default()
            }
        }
    }

    /// One-shot copy of settings from Swift WebDock (token/LAN), without sharing TCC identity.
    fn try_migrate_from_webdock() -> Option<Self> {
        let path = legacy_webdock_config_json();
        let text = std::fs::read_to_string(path).ok()?;
        let mut cfg: Self = serde_json::from_str(&text).ok()?;
        // Avoid binding the same port as the Swift app (usually 8080).
        if cfg.port == 8080 {
            cfg.port = crate::tuning::DEFAULT_PORT;
        }
        cfg.server_enabled = true;
        Some(cfg)
    }

    pub fn load() -> Result<Self, ConfigError> {
        let path = Self::config_path();
        let text = std::fs::read_to_string(&path)?;
        let cfg: Self = serde_json::from_str(&text)?;
        Ok(cfg)
    }

    pub fn save(&self) -> Result<PathBuf, ConfigError> {
        let dir = Self::support_dir();
        std::fs::create_dir_all(&dir)?;
        let path = Self::config_path();
        let text = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, text)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
        }
        Ok(path)
    }

    /// Constant-time token compare when a token is configured.
    ///
    /// When lengths differ we still run a dummy compare so length is not an oracle.
    pub fn token_matches(&self, provided: Option<&str>) -> bool {
        if !self.has_token() {
            return true;
        }
        let Some(p) = provided else {
            return false;
        };
        use subtle::ConstantTimeEq;
        let a = self.token.as_bytes();
        let b = p.as_bytes();
        // Pad shorter side mentally: compare equal-length only; if lengths differ,
        // still ct_eq against token with a fixed dummy to avoid short-circuit leaks.
        if a.len() != b.len() {
            let _ = a.ct_eq(a);
            return false;
        }
        bool::from(a.ct_eq(b))
    }

    /// Origin/Host allowed for WebSocket (CSRF-ish). Loopback always ok.
    /// Empty `allowed_domains` → allow any host that passes Host parse (LAN use).
    pub fn is_origin_allowed(&self, origin: Option<&str>, host_header: Option<&str>) -> bool {
        let Some(origin) = origin else {
            // Non-browser clients may omit Origin — allow only when token is set
            // (shared secret) or loopback. Without token, require Origin.
            return self.has_token();
        };
        let origin = origin.trim();
        if origin.is_empty() || origin.eq_ignore_ascii_case("null") {
            return false;
        }
        // Parse origin host from e.g. https://example.com:443
        let host = origin
            .split("://")
            .nth(1)
            .unwrap_or(origin)
            .split('/')
            .next()
            .unwrap_or("")
            .split(':')
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();
        if host.is_empty() {
            return false;
        }
        if is_loopback_host(&host) {
            return true;
        }
        if let Some(hh) = host_header {
            let hh = hh.split(':').next().unwrap_or(hh).to_ascii_lowercase();
            if host == hh {
                return true;
            }
        }
        self.is_domain_allowed(&host)
    }

    pub fn is_ip_allowed(&self, ip: &str) -> bool {
        let n = normalize_ip(ip);
        if is_loopback(&n) {
            return true;
        }
        if !self.ip_allowlist_enabled {
            return true;
        }
        if self.allowed_ips.is_empty() {
            return false;
        }
        self.allowed_ips.iter().any(|a| normalize_ip(a) == n)
    }

    pub fn is_domain_allowed(&self, host: &str) -> bool {
        let h = host.to_ascii_lowercase();
        if is_loopback_host(&h) {
            return true;
        }
        if self.allowed_domains.is_empty() {
            return true;
        }
        self.allowed_domains
            .iter()
            .any(|d| d.to_ascii_lowercase() == h)
    }

    pub fn connection_urls(&self, lan_addresses: &[String]) -> Vec<String> {
        let mut urls = vec![format!("http://127.0.0.1:{}", self.port)];
        if self.allow_lan {
            for ip in lan_addresses {
                urls.push(format!("http://{}:{}", ip, self.port));
            }
        }
        urls
    }

    fn try_migrate_ini() -> Option<Self> {
        let path = Self::legacy_ini_path();
        let text = std::fs::read_to_string(path).ok()?;
        Some(parse_ini(&text))
    }
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
}

fn dirs_support() -> PathBuf {
    // Product folder is **WebRust** (separate from Swift WebDock) so config + TCC stay distinct.
    // Do **not** require HOME on Windows (it is often unset).
    #[cfg(target_os = "macos")]
    {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join("Library/Application Support/WebRust");
        }
    }
    #[cfg(target_os = "windows")]
    {
        if let Some(ad) = std::env::var_os("APPDATA") {
            return PathBuf::from(ad).join("WebRust");
        }
        if let Some(up) = std::env::var_os("USERPROFILE") {
            return PathBuf::from(up)
                .join("AppData")
                .join("Roaming")
                .join("WebRust");
        }
        if let Some(proj) = directories::ProjectDirs::from("app", "WebRust", "WebRust") {
            return proj.config_dir().to_path_buf();
        }
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
            return PathBuf::from(xdg).join("webrust");
        }
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(".config/webrust");
        }
        if let Some(proj) = directories::ProjectDirs::from("app", "WebRust", "WebRust") {
            return proj.config_dir().to_path_buf();
        }
    }
    PathBuf::from(".").join("webrust-config")
}

/// Swift WebDock config path (optional one-shot migration source).
fn legacy_webdock_config_json() -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join("Library/Application Support/WebDock/config.json");
        }
    }
    #[cfg(target_os = "windows")]
    {
        if let Some(ad) = std::env::var_os("APPDATA") {
            return PathBuf::from(ad).join("WebDock").join("config.json");
        }
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(".config/webdock/config.json");
        }
    }
    PathBuf::from("webdock-config.json")
}

fn normalize_ip(ip: &str) -> String {
    ip.trim().to_ascii_lowercase()
}

fn is_loopback(ip: &str) -> bool {
    ip == "127.0.0.1" || ip == "::1" || ip == "localhost" || ip.starts_with("127.")
}

fn is_loopback_host(h: &str) -> bool {
    h == "localhost" || h == "127.0.0.1" || h == "::1"
}

/// Minimal INI parser for Swift `config.ini` migration.
pub fn parse_ini(text: &str) -> AppConfig {
    let mut cfg = AppConfig::default();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        let k = k.trim().to_ascii_lowercase();
        let v = v.trim();
        match k.as_str() {
            "serverenabled" | "server_enabled" => cfg.server_enabled = parse_bool(v),
            "port" => {
                if let Ok(p) = v.parse() {
                    cfg.port = p;
                }
            }
            "allowlan" | "allow_lan" => cfg.allow_lan = parse_bool(v),
            "token" => cfg.token = v.to_string(),
            "alloweddomains" | "allowed_domains" => {
                cfg.allowed_domains = split_list(v);
            }
            "ipallowlistenabled" | "ip_allowlist_enabled" => {
                cfg.ip_allowlist_enabled = parse_bool(v);
            }
            "allowedips" | "allowed_ips" => cfg.allowed_ips = split_list(v),
            _ => {}
        }
    }
    cfg
}

fn parse_bool(v: &str) -> bool {
    matches!(v.to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on")
}

fn split_list(v: &str) -> Vec<String> {
    v.split([',', ' ', ';'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_match() {
        let mut c = AppConfig::default();
        assert!(c.token_matches(None));
        c.token = "secret".into();
        assert!(!c.token_matches(None));
        assert!(c.token_matches(Some("secret")));
        assert!(!c.token_matches(Some("Secret")));
    }

    #[test]
    fn parse_ini_sample() {
        let ini = r#"
serverEnabled=true
port=9090
allowLAN=1
token=abc
allowedIPs=10.0.0.1, 10.0.0.2
ipAllowlistEnabled=true
"#;
        let c = parse_ini(ini);
        assert!(c.server_enabled);
        assert_eq!(c.port, 9090);
        assert!(c.allow_lan);
        assert_eq!(c.token, "abc");
        assert_eq!(c.allowed_ips.len(), 2);
        assert!(c.ip_allowlist_enabled);
    }
}
