//! Embedded HTTP + WebSocket server + desktop host controller.

mod auth;
mod handlers;
mod host;
mod peer;
mod state;
mod static_files;

pub mod gui;
pub use host::{Host, SharedHost};

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use axum::routing::get;
use axum::Router;
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tracing::info;

use webdock_core::AppConfig;
use webdock_platform::PlatformServices;

pub use state::AppState;

/// Handle to a running server instance.
pub struct ServerHandle {
    pub local_addr: SocketAddr,
    shutdown: Option<oneshot::Sender<()>>,
    join: Option<tokio::task::JoinHandle<()>>,
    metrics_join: Option<tokio::task::JoinHandle<()>>,
}

impl ServerHandle {
    pub async fn stop(mut self) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
        if let Some(join) = self.metrics_join.take() {
            join.abort();
        }
        if let Some(join) = self.join.take() {
            let _ = join.await;
        }
    }
}

pub struct ServerOptions {
    pub config: AppConfig,
    pub webui_dir: Option<PathBuf>,
    pub platform: PlatformServices,
}

/// Start the server. Returns immediately with a handle.
pub async fn start(opts: ServerOptions) -> Result<ServerHandle, std::io::Error> {
    let bind: SocketAddr = if opts.config.allow_lan {
        ([0, 0, 0, 0], opts.config.port).into()
    } else {
        ([127, 0, 0, 1], opts.config.port).into()
    };

    let webui = opts.webui_dir.or_else(discover_webui);

    if let Some(ref dir) = webui {
        info!(path = %dir.display(), "WebUI directory");
    } else {
        info!("WebUI: embedded assets");
    }

    let state = Arc::new(AppState::new(opts.config, opts.platform, webui));

    // IMPORTANT: path `/` must allow GET (SPA) as well as POST (login).
    // Registering only `post(...)` makes browsers GET / return **HTTP 405**.
    let app = Router::new()
        .route("/health", get(handlers::health))
        .route("/ws", get(handlers::ws_handler))
        .route("/api/status", get(handlers::status_handler))
        .route(
            "/",
            get(static_files::serve_static).post(static_files::login_post),
        )
        .fallback(static_files::serve_static)
        .with_state(state.clone());

    let listener = TcpListener::bind(bind).await?;
    let local_addr = listener.local_addr()?;
    info!(%local_addr, "WebRust server listening");

    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

    // Metrics loop stops when shutdown fires (shared with serve).
    let (metrics_stop_tx, mut metrics_stop_rx) = oneshot::channel::<()>();
    let metrics_state = state.clone();
    let metrics_join = tokio::spawn(async move {
        let mut tick = tokio::time::interval(std::time::Duration::from_secs(2));
        loop {
            tokio::select! {
                _ = tick.tick() => {
                    metrics_state.broadcast_metrics();
                }
                _ = &mut metrics_stop_rx => break,
            }
        }
    });

    let join = tokio::spawn(async move {
        let serve = axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .with_graceful_shutdown(async {
            let _ = shutdown_rx.await;
            let _ = metrics_stop_tx.send(());
        });
        if let Err(e) = serve.await {
            tracing::error!(error = %e, "server error");
        }
    });

    Ok(ServerHandle {
        local_addr,
        shutdown: Some(shutdown_tx),
        join: Some(join),
        metrics_join: Some(metrics_join),
    })
}

fn discover_webui() -> Option<PathBuf> {
    let candidates = [
        PathBuf::from("webui"),
        PathBuf::from("../webui"),
        PathBuf::from("../../webui"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../webui"),
    ];
    candidates
        .into_iter()
        .find(|p| p.join("index.html").is_file())
}

/// LAN IPv4 addresses for UI display.
pub fn lan_addresses() -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(ifaces) = if_addrs::get_if_addrs() {
        for iface in ifaces {
            if iface.is_loopback() {
                continue;
            }
            if let if_addrs::IfAddr::V4(v4) = iface.addr {
                out.push(v4.ip.to_string());
            }
        }
    }
    out
}
