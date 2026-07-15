//! WebRust — remote desktop host.
//!
//! Default: **settings GUI + system tray** on macOS / Windows / Linux.
//! Headless: `WebRust --cli --port 8090`
// GUI subsystem on Windows: no console window on double-click.
// CLI/terminal output is restored via AttachConsole below.
#![cfg_attr(windows, windows_subsystem = "windows")]

use std::fs::OpenOptions;
use std::path::PathBuf;

use tracing_subscriber::EnvFilter;
use webdock_core::AppConfig;
use webdock_platform;
use webdock_server::{lan_addresses, start, ServerOptions};

/// With `windows_subsystem = "windows"` the process has no console, so
/// `println!`/`eprintln!` go nowhere even when launched from cmd/PowerShell.
/// Attaching to the parent's console (if any) restores terminal output for
/// `--cli`, `--help`, `--version`, etc. Must run before the first print.
#[cfg(windows)]
fn attach_parent_console() {
    use windows::Win32::System::Console::{AttachConsole, ATTACH_PARENT_PROCESS};
    unsafe {
        let _ = AttachConsole(ATTACH_PARENT_PROCESS);
    }
}

fn main() {
    #[cfg(windows)]
    attach_parent_console();

    let args: Vec<String> = std::env::args().skip(1).collect();
    let want_cli = args.iter().any(|a| a == "--cli" || a == "--headless");

    if args.iter().any(|a| a == "-h" || a == "--help") {
        println!(
            "WebRust — remote desktop host v{}\n\n\
             Default: settings window + system tray (Win/macOS/Linux)\n\
             CLI:  WebRust --cli [--port N] [--lan] [--token T] [--webui DIR]\n\
             Other:\n\
               WebRust --gen-token\n\
               WebRust --check-update\n\
               WebRust --install-update\n\
               WebRust --version",
            env!("CARGO_PKG_VERSION")
        );
        return;
    }

    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("WebRust {}", env!("CARGO_PKG_VERSION"));
        return;
    }

    if args.iter().any(|a| a == "--check-update") {
        match webdock_server::updater::check_for_update() {
            Ok(info) => {
                println!("current: {}", info.current_version);
                println!("latest:  {}", info.latest_version);
                if info.update_available {
                    println!("update:  AVAILABLE");
                    if let Some(n) = &info.asset_name {
                        println!("asset:   {n}");
                    }
                    println!("kind:    {:?}", info.install_kind);
                    if let Some(u) = &info.download_url {
                        println!("download:{u}");
                    }
                    if let Some(u) = &info.html_url {
                        println!("page:    {u}");
                    }
                    std::process::exit(0);
                } else {
                    println!("update:  up to date");
                    std::process::exit(0);
                }
            }
            Err(e) => {
                eprintln!("check-update failed: {e}");
                std::process::exit(1);
            }
        }
    }

    if args.iter().any(|a| a == "--install-update") {
        match webdock_server::updater::check_for_update() {
            Ok(info) if !info.update_available => {
                println!("already up to date (v{})", info.current_version);
                std::process::exit(0);
            }
            Ok(info) => {
                println!(
                    "installing v{} ({})…",
                    info.latest_version,
                    info.asset_name.as_deref().unwrap_or("?")
                );
                match webdock_server::updater::apply_update(&info, |p| {
                    eprintln!("[{}] {}%", p.phase, p.percent);
                }) {
                    Ok(out) => {
                        println!("{}", out.message);
                        if out.should_exit {
                            std::process::exit(0);
                        }
                        std::process::exit(0);
                    }
                    Err(e) => {
                        eprintln!("install-update failed: {e}");
                        std::process::exit(1);
                    }
                }
            }
            Err(e) => {
                eprintln!("check-update failed: {e}");
                std::process::exit(1);
            }
        }
    }

    init_tracing();

    if !want_cli {
        if let Err(e) = webdock_server::gui::run() {
            eprintln!("GUI error: {e}");
            eprintln!("Hint: use headless mode: WebRust --cli --port 8090");
            std::process::exit(1);
        }
        return;
    }

    if let Err(e) = run_cli(args) {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn init_tracing() {
    let log_path = AppConfig::support_dir().join("webrust.log");
    let _ = std::fs::create_dir_all(AppConfig::support_dir());
    let filter = EnvFilter::from_default_env()
        .add_directive("webdock_server=info".parse().unwrap_or_default());
    if std::io::IsTerminal::is_terminal(&std::io::stderr()) {
        tracing_subscriber::fmt().with_env_filter(filter).init();
    } else if let Ok(file) = OpenOptions::new().create(true).append(true).open(&log_path) {
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_writer(std::sync::Mutex::new(file))
            .init();
    } else {
        tracing_subscriber::fmt().with_env_filter(filter).init();
    }
}

#[tokio::main]
async fn run_cli(args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let mut cfg = AppConfig::load_or_default();
    cfg.server_enabled = true;

    let mut webui_dir: Option<PathBuf> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--cli" | "--headless" => {}
            "--port" => {
                i += 1;
                if let Some(p) = args.get(i) {
                    cfg.port = p.parse()?;
                }
            }
            "--lan" => cfg.allow_lan = true,
            "--token" => {
                i += 1;
                if let Some(t) = args.get(i) {
                    cfg.token = t.clone();
                }
            }
            "--webui" => {
                i += 1;
                if let Some(p) = args.get(i) {
                    webui_dir = Some(PathBuf::from(p));
                }
            }
            "--gen-token" => {
                cfg.token = AppConfig::generate_token(24);
                let _ = cfg.save();
                println!("token={}", cfg.token);
                println!("saved {:?}", AppConfig::config_path());
                return Ok(());
            }
            other => eprintln!("unknown arg: {other}"),
        }
        i += 1;
    }

    if webui_dir.is_none() {
        webui_dir = discover_bundled_webui();
    }

    // Exact configured / --port only. No port scanning or config rewrite.
    let port = cfg.port;
    if port == 0 {
        return Err("port must be between 1 and 65535".into());
    }
    let handle = match start(ServerOptions {
        config: cfg.clone(),
        webui_dir: webui_dir.clone(),
        platform: webdock_platform::current(),
    })
    .await
    {
        Ok(h) => h,
        Err(e) => {
            return Err(
                format!("port {port} is unavailable ({e}). Use --port N to pick another.").into(),
            );
        }
    };

    // Persist only when CLI mutated config (e.g. --lan / --token); keep user's port.
    let _ = cfg.save();

    for u in cfg.connection_urls(&lan_addresses()) {
        println!("WebRust: {u}");
        if cfg.has_token() {
            println!("  token query: {u}/?token={}", cfg.token);
        }
    }
    println!("listening on http://{}/", handle.local_addr);
    println!("config: {:?}", AppConfig::config_path());
    println!("press Ctrl+C to stop");

    tokio::signal::ctrl_c().await?;
    handle.stop().await;
    Ok(())
}

fn discover_bundled_webui() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let exe_dir = exe.parent()?;

    if let Some(contents) = exe_dir.parent() {
        let bundled = contents.join("Resources").join("webui");
        if bundled.join("index.html").is_file() {
            return Some(bundled);
        }
    }
    let beside = exe_dir.join("webui");
    if beside.join("index.html").is_file() {
        return Some(beside);
    }
    for rel in ["webui", "../webui", "../../webui"] {
        let p = PathBuf::from(rel);
        if p.join("index.html").is_file() {
            return Some(p);
        }
    }
    None
}
