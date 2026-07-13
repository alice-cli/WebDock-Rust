//! Process-wide host: config + optional HTTP server lifecycle (for GUI + CLI).

use std::path::PathBuf;
use std::sync::Arc;

use parking_lot::Mutex;
use tracing::{info, warn};
use webdock_core::AppConfig;
use webdock_platform;

use crate::{lan_addresses, start, ServerHandle, ServerOptions};

/// Owns config and the running remote-desktop server.
pub struct Host {
    rt: tokio::runtime::Runtime,
    config: Mutex<AppConfig>,
    handle: Mutex<Option<ServerHandle>>,
    webui_dir: Option<PathBuf>,
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
        })
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
        let port = self.local_port().unwrap_or(cfg.port);
        let mut urls = cfg.connection_urls(&lan_addresses());
        // If bound port differs (fallback), rewrite.
        if let Some(p) = self.local_port() {
            if p != cfg.port {
                urls = vec![format!("http://127.0.0.1:{p}")];
                if cfg.allow_lan {
                    for ip in lan_addresses() {
                        urls.push(format!("http://{ip}:{p}"));
                    }
                }
            }
        }
        serde_json::json!({
            "running": running,
            "port": port,
            "serverEnabled": cfg.server_enabled,
            "allowLan": cfg.allow_lan,
            "token": cfg.token,
            "hasToken": cfg.has_token(),
            "allowedDomains": cfg.allowed_domains,
            "ipAllowlistEnabled": cfg.ip_allowlist_enabled,
            "allowedIps": cfg.allowed_ips,
            "urls": urls,
            "configPath": AppConfig::config_path().display().to_string(),
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
        let mut cfg = self.config();
        let need_restart = self.is_running()
            && (cfg.port != port
                || cfg.allow_lan != allow_lan
                || cfg.token != token
                || cfg.allowed_domains != allowed_domains
                || cfg.ip_allowlist_enabled != ip_allowlist_enabled
                || cfg.allowed_ips != allowed_ips);

        cfg.server_enabled = server_enabled;
        cfg.port = port.max(1);
        cfg.allow_lan = allow_lan;
        cfg.token = token;
        cfg.allowed_domains = allowed_domains;
        cfg.ip_allowlist_enabled = ip_allowlist_enabled;
        cfg.allowed_ips = allowed_ips;
        self.save_config(cfg)?;

        if need_restart {
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
        let ports: Vec<u16> = {
            let mut v = vec![cfg.port];
            for p in 8090u16..=8100 {
                if !v.contains(&p) {
                    v.push(p);
                }
            }
            v
        };

        let mut last = None;
        for port in ports {
            let mut c = cfg.clone();
            c.port = port;
            c.server_enabled = true;
            match self.rt.block_on(start(ServerOptions {
                config: c.clone(),
                webui_dir: webui.clone(),
                platform: webdock_platform::current(),
            })) {
                Ok(h) => {
                    let addr = h.local_addr;
                    if addr.port() != cfg.port {
                        let mut saved = c;
                        saved.port = addr.port();
                        let _ = saved.save();
                        *self.config.lock() = saved;
                    } else {
                        let mut saved = self.config();
                        saved.server_enabled = true;
                        let _ = saved.save();
                        *self.config.lock() = saved;
                    }
                    *self.handle.lock() = Some(h);
                    info!(%addr, "server started");
                    return Ok(addr);
                }
                Err(e) => {
                    warn!(port, error = %e, "bind failed");
                    last = Some(e);
                }
            }
        }
        Err(last
            .map(|e| e.to_string())
            .unwrap_or_else(|| "bind failed".into()))
    }

    pub fn stop(&self) {
        let h = self.handle.lock().take();
        if let Some(h) = h {
            self.rt.block_on(h.stop());
            info!("server stopped");
        }
        let mut cfg = self.config();
        cfg.server_enabled = false;
        let _ = cfg.save();
        *self.config.lock() = cfg;
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
            // Token change requires restart to apply auth middleware.
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
        let _ = std::process::Command::new("open").arg(&url).spawn();
    }
}

pub type SharedHost = Arc<Host>;
