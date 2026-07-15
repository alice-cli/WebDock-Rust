//! Process-wide host: config + optional HTTP server lifecycle (for GUI + CLI).

use std::path::PathBuf;
use std::sync::Arc;

use parking_lot::Mutex;
use tracing::{info, warn};
use webdock_core::AppConfig;
use webdock_platform;

use crate::updater::{self, UpdateInfo, UpdateProgress};
use crate::{lan_addresses, start, ServerHandle, ServerOptions};

/// Owns config and the running remote-desktop server.
pub struct Host {
    rt: tokio::runtime::Runtime,
    config: Mutex<AppConfig>,
    handle: Mutex<Option<ServerHandle>>,
    webui_dir: Option<PathBuf>,
    update: Mutex<Option<UpdateInfo>>,
    update_progress: Mutex<UpdateProgress>,
    /// Set when install asked us to exit after kicking installer.
    pending_exit: Mutex<bool>,
}

impl Host {
    pub fn new(webui_dir: Option<PathBuf>) -> Result<Self, String> {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .thread_name("webrust-server")
            .build()
            .map_err(|e| e.to_string())?;
        let config = AppConfig::load_or_default();
        Ok(Self {
            rt,
            config: Mutex::new(config),
            handle: Mutex::new(None),
            webui_dir,
            update: Mutex::new(None),
            update_progress: Mutex::new(UpdateProgress::idle()),
            pending_exit: Mutex::new(false),
        })
    }

    pub fn set_update_info(&self, info: Option<UpdateInfo>) {
        *self.update.lock() = info;
    }

    pub fn update_info(&self) -> Option<UpdateInfo> {
        self.update.lock().clone()
    }

    pub fn update_progress(&self) -> UpdateProgress {
        self.update_progress.lock().clone()
    }

    pub fn set_update_progress(&self, p: UpdateProgress) {
        *self.update_progress.lock() = p;
    }

    pub fn take_pending_exit(&self) -> bool {
        let mut g = self.pending_exit.lock();
        let v = *g;
        *g = false;
        v
    }

    /// Download + install latest (or cached) update. May set pending_exit.
    pub fn install_update(&self) -> Result<String, String> {
        let info = match self.update_info() {
            Some(i) if i.update_available && i.download_url.is_some() => i,
            _ => {
                self.set_update_progress(UpdateProgress {
                    phase: "checking".into(),
                    percent: 0,
                    message: "Checking for updates…".into(),
                    downloaded: 0,
                    total: 0,
                });
                let i = updater::check_for_update()?;
                self.set_update_info(Some(i.clone()));
                if !i.update_available {
                    self.set_update_progress(UpdateProgress::idle());
                    return Ok(format!("Up to date (v{})", i.current_version));
                }
                if i.download_url.is_none() {
                    self.set_update_progress(UpdateProgress::idle());
                    return Err("No installable asset for this platform".into());
                }
                i
            }
        };

        // Stop embedded server so installers can replace the binary.
        self.stop();

        match updater::apply_update(&info, |p| {
            self.set_update_progress(p);
        }) {
            Ok(outcome) => {
                if outcome.should_exit {
                    *self.pending_exit.lock() = true;
                }
                Ok(outcome.message)
            }
            Err(e) => {
                self.set_update_progress(UpdateProgress {
                    phase: "error".into(),
                    percent: 0,
                    message: e.clone(),
                    downloaded: 0,
                    total: 0,
                });
                Err(e)
            }
        }
    }

    pub fn config(&self) -> AppConfig {
        self.config.lock().clone()
    }

    pub fn is_running(&self) -> bool {
        self.handle.lock().is_some()
    }

    pub fn local_port(&self) -> Option<u16> {
        self.handle.lock().as_ref().map(|h| h.local_addr.port())
    }

    pub fn urls(&self) -> Vec<String> {
        let cfg = self.config();
        cfg.connection_urls(&lan_addresses())
    }

    pub fn status_json(&self) -> serde_json::Value {
        let cfg = self.config();
        let running = self.is_running();
        // Always the user-configured port from config.json — never a fallback.
        let port = cfg.port;
        let bound = self.local_port();
        let urls = if running {
            cfg.connection_urls(&lan_addresses())
        } else {
            Vec::new()
        };
        let update = self.update.lock().clone();
        let update_progress = self.update_progress.lock().clone();
        serde_json::json!({
            "running": running,
            "port": port,
            "boundPort": bound,
            "serverEnabled": cfg.server_enabled,
            "allowLan": cfg.allow_lan,
            "token": cfg.token,
            "hasToken": cfg.has_token(),
            "allowedDomains": cfg.allowed_domains,
            "ipAllowlistEnabled": cfg.ip_allowlist_enabled,
            "allowedIps": cfg.allowed_ips,
            "urls": urls,
            "configPath": AppConfig::config_path().display().to_string(),
            "version": env!("CARGO_PKG_VERSION"),
            "update": update,
            "updateProgress": update_progress,
        })
    }

    pub fn save_config(&self, cfg: AppConfig) -> Result<(), String> {
        cfg.save().map_err(|e| e.to_string())?;
        *self.config.lock() = cfg;
        Ok(())
    }

    pub fn set_from_status_fields(
        &self,
        server_enabled: bool,
        port: u16,
        allow_lan: bool,
        token: String,
        allowed_domains: Vec<String>,
        ip_allowlist_enabled: bool,
        allowed_ips: Vec<String>,
    ) -> Result<(), String> {
        if !(1..=65535).contains(&port) {
            return Err("port must be between 1 and 65535".into());
        }

        let prev = self.config();
        let port_changed = prev.port != port;
        let bind_related_changed = port_changed
            || prev.allow_lan != allow_lan
            || prev.token != token
            || prev.allowed_domains != allowed_domains
            || prev.ip_allowlist_enabled != ip_allowlist_enabled
            || prev.allowed_ips != allowed_ips;
        let was_running = self.is_running();

        let mut cfg = prev;
        cfg.server_enabled = server_enabled;
        cfg.port = port;
        cfg.allow_lan = allow_lan;
        cfg.token = token;
        cfg.allowed_domains = allowed_domains;
        cfg.ip_allowlist_enabled = ip_allowlist_enabled;
        cfg.allowed_ips = allowed_ips;
        // Persist user intent first — never lose a port edit if bind fails later.
        self.save_config(cfg)?;

        if was_running && bind_related_changed {
            // Restart on the new settings. stop() does NOT rewrite config.
            self.stop();
            if server_enabled {
                self.start()?;
            }
        } else if server_enabled && !self.is_running() {
            self.start()?;
        } else if !server_enabled && self.is_running() {
            self.stop();
        }
        Ok(())
    }

    pub fn start(&self) -> Result<std::net::SocketAddr, String> {
        if self.is_running() {
            return Ok(self.handle.lock().as_ref().unwrap().local_addr);
        }
        let cfg = self.config();
        let webui = self.webui_dir.clone();
        // Exact user port only. No range scan, no random, no rewrite of config.port.
        let port = cfg.port;
        if port == 0 {
            return Err("port must be between 1 and 65535".into());
        }
        let mut c = cfg.clone();
        c.port = port;
        c.server_enabled = true;
        match self.rt.block_on(start(ServerOptions {
            config: c,
            webui_dir: webui,
            platform: webdock_platform::current(),
        })) {
            Ok(h) => {
                let addr = h.local_addr;
                // Only flip server_enabled — never touch port.
                let mut saved = self.config();
                saved.server_enabled = true;
                let _ = saved.save();
                *self.config.lock() = saved;
                *self.handle.lock() = Some(h);
                info!(%addr, "server started");
                Ok(addr)
            }
            Err(e) => {
                warn!(port, error = %e, "bind failed");
                Err(format!(
                    "port {port} is unavailable ({e}). Choose another port and Apply."
                ))
            }
        }
    }

    /// Stop the listening server. Does **not** change `config.json` (port or flags).
    pub fn stop(&self) {
        let h = self.handle.lock().take();
        if let Some(h) = h {
            self.rt.block_on(h.stop());
            info!("server stopped");
        }
    }

    /// Stop and set `server_enabled = false` in config (user turned server off).
    pub fn disable(&self) {
        self.stop();
        let mut cfg = self.config();
        if cfg.server_enabled {
            cfg.server_enabled = false;
            let _ = cfg.save();
            *self.config.lock() = cfg;
        }
    }

    pub fn start_if_enabled(&self) -> Result<(), String> {
        if self.config().server_enabled {
            self.start().map(|_| ())
        } else {
            Ok(())
        }
    }

    pub fn gen_token(&self) -> Result<String, String> {
        let mut cfg = self.config();
        cfg.token = AppConfig::generate_token(24);
        let t = cfg.token.clone();
        self.save_config(cfg)?;
        if self.is_running() {
            self.stop();
            let mut c = self.config();
            c.server_enabled = true;
            self.save_config(c)?;
            self.start()?;
        }
        Ok(t)
    }

    pub fn open_remote_ui(&self) {
        let cfg = self.config();
        let port = self.local_port().unwrap_or(cfg.port);
        let url = if cfg.has_token() {
            format!("http://127.0.0.1:{port}/?token={}", cfg.token)
        } else {
            format!("http://127.0.0.1:{port}/")
        };
        crate::util::open_url(&url);
    }
}

pub type SharedHost = Arc<Host>;
