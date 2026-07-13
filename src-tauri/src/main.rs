//! WebDock desktop entry.
//!
//! Default: headless server (same as `webdock-server`).
//! With `--features tauri-shell`: Tauri tray + settings window (P4).

use tracing_subscriber::EnvFilter;
use webdock_core::AppConfig;
use webdock_server::{lan_addresses, start, ServerOptions};

#[cfg(not(feature = "tauri-shell"))]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env().add_directive("webdock_server=info".parse()?),
        )
        .init();

    let mut cfg = AppConfig::load_or_default();
    // Desktop app defaults to starting the server when launched (override via config).
    if std::env::args().any(|a| a == "--enable") {
        cfg.server_enabled = true;
    }
    if !cfg.server_enabled {
        eprintln!(
            "serverEnabled=false in config; pass --enable or set serverEnabled in config.json"
        );
        eprintln!("config path: {:?}", AppConfig::config_path());
        // Still start for dev convenience when no config file exists.
        if !AppConfig::config_path().exists() {
            cfg.server_enabled = true;
            cfg.port = 8080;
        } else {
            return Ok(());
        }
    }

    for u in cfg.connection_urls(&lan_addresses()) {
        println!("WebDock: {u}");
    }

    let handle = start(ServerOptions {
        config: cfg,
        webui_dir: None,
        platform: webdock_platform::current(),
    })
    .await?;

    println!("listening on http://{}/", handle.local_addr);
    tokio::signal::ctrl_c().await?;
    handle.stop().await;
    Ok(())
}

#[cfg(feature = "tauri-shell")]
fn main() {
    // Full Tauri bootstrap lands in P4. For now, feature compiles the dependency graph.
    eprintln!("tauri-shell: use tray UI (P4) — falling back to note");
    eprintln!("Run without --features tauri-shell for the embedded server.");
}
