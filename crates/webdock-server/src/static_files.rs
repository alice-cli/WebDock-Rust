//! Authenticated static WebUI serving (filesystem or rust-embed).

use std::path::Path;

use axum::body::Body;
use axum::http::{header, HeaderMap, Request, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use axum::Form;
use serde::Deserialize;
use tracing::debug;

use crate::auth::{self, AuthFail};
use crate::state::SharedState;

#[derive(rust_embed::Embed)]
#[folder = "../../webui"]
#[prefix = ""]
pub struct EmbeddedWebUi;

#[derive(Debug, Deserialize, Default)]
pub struct TokenQuery {
    pub token: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct LoginForm {
    pub token: Option<String>,
}

/// Peer IP for access control. Proxy headers only when `trust_proxy_headers`.
fn peer_ip(headers: &HeaderMap, fallback: &str, trust_proxy: bool) -> String {
    if trust_proxy {
        if let Some(v) = headers
            .get("cf-connecting-ip")
            .or_else(|| headers.get("x-real-ip"))
            .and_then(|v| v.to_str().ok())
        {
            return v.split(',').next().unwrap_or(v).trim().to_string();
        }
        if let Some(v) = headers.get("x-forwarded-for").and_then(|v| v.to_str().ok()) {
            return v.split(',').next().unwrap_or(v).trim().to_string();
        }
    }
    fallback.to_string()
}

/// Simple login rate limit: max 10 attempts / 60s per socket IP.
fn login_rate_ok(state: &SharedState, ip: &str) -> bool {
    state.login_rate_check(ip)
}

pub fn content_type_for(path: &str) -> &'static str {
    match Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "html" => "text/html; charset=utf-8",
        "js" => "application/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "png" => "image/png",
        "ico" => "image/x-icon",
        "svg" => "image/svg+xml",
        "json" => "application/json",
        "woff2" => "font/woff2",
        _ => "application/octet-stream",
    }
}

/// HTML/JS/CSS must revalidate — stale i18n.js caused language-switch bugs after updates.
fn cache_control_for(rel: &str) -> &'static str {
    match Path::new(rel)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "html" | "js" | "css" => "no-cache",
        _ => "public, max-age=86400",
    }
}

/// Inject build version into index.html so script/link URLs bust browser caches.
fn inject_webui_version(bytes: Vec<u8>, rel: &str) -> Vec<u8> {
    if rel != "index.html" {
        return bytes;
    }
    match String::from_utf8(bytes) {
        Ok(s) => s
            .replace("__WEBRUST_VER__", env!("CARGO_PKG_VERSION"))
            .into_bytes(),
        Err(e) => e.into_bytes(),
    }
}

fn normalize_path(uri_path: &str) -> String {
    let p = uri_path.split('?').next().unwrap_or(uri_path);
    if p.is_empty() || p == "/" {
        return "index.html".into();
    }
    let p = p.trim_start_matches('/');
    if p.is_empty() {
        "index.html".into()
    } else {
        p.to_string()
    }
}

pub fn is_asset_public(rel: &str) -> bool {
    rel.starts_with("favicon") || rel == "apple-touch-icon.png" || rel == "apple-touch-icon"
}

/// Serve one file from disk or embed.
pub fn load_file(webui_dir: Option<&Path>, rel: &str) -> Option<(Vec<u8>, &'static str)> {
    let ct = content_type_for(rel);
    if let Some(dir) = webui_dir {
        let path = dir.join(rel);
        // Prevent path traversal
        let canon_dir = dir.canonicalize().ok()?;
        let canon_file = path.canonicalize().ok()?;
        if !canon_file.starts_with(&canon_dir) {
            return None;
        }
        if let Ok(bytes) = std::fs::read(&canon_file) {
            return Some((bytes, ct));
        }
    }
    EmbeddedWebUi::get(rel).map(|f| (f.data.into_owned(), ct))
}

pub async fn login_post(
    axum::extract::State(state): axum::extract::State<SharedState>,
    axum::extract::ConnectInfo(addr): axum::extract::ConnectInfo<std::net::SocketAddr>,
    headers: HeaderMap,
    Form(form): Form<LoginForm>,
) -> Response {
    let cfg = state.config.lock().clone();
    // Always use socket IP for login rate-limit (ignore spoofable proxy headers).
    let socket_ip = addr.ip().to_string();
    if !login_rate_ok(&state, &socket_ip) {
        return (StatusCode::TOO_MANY_REQUESTS, "too many login attempts").into_response();
    }
    let ip = peer_ip(&headers, &socket_ip, cfg.trust_proxy_headers);
    let token = form.token.as_deref().unwrap_or("").trim();
    if !cfg.is_ip_allowed(&ip) {
        return AuthFail::ForbiddenIp.into_response();
    }
    if !cfg.token_matches(Some(token)) {
        return auth::gate_page(true).into_response();
    }
    // 303 to clean URL + Set-Cookie (matches Swift host).
    let mut res = Redirect::to("/").into_response();
    if let Ok(val) = header::HeaderValue::from_str(&auth::set_cookie_header(token)) {
        res.headers_mut().insert(header::SET_COOKIE, val);
    }
    res
}

/// Authenticated SPA / static asset handler.
pub async fn serve_static(
    axum::extract::State(state): axum::extract::State<SharedState>,
    axum::extract::ConnectInfo(addr): axum::extract::ConnectInfo<std::net::SocketAddr>,
    axum::extract::Query(q): axum::extract::Query<TokenQuery>,
    headers: HeaderMap,
    req: Request<Body>,
) -> Response {
    let path = req.uri().path();
    let rel = normalize_path(path);
    let cfg = state.config.lock().clone();
    let ip = peer_ip(&headers, &addr.ip().to_string(), cfg.trust_proxy_headers);

    // IP / domain always enforced.
    if !cfg.is_ip_allowed(&ip) {
        return AuthFail::ForbiddenIp.into_response();
    }
    if let Some(host) = headers
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .map(|h| h.split(':').next().unwrap_or(h))
    {
        if !cfg.is_domain_allowed(host) {
            return AuthFail::ForbiddenHost.into_response();
        }
    }

    let public_asset = is_asset_public(&rel);
    let is_index = rel == "index.html";

    if cfg.has_token() && !public_asset {
        let token = auth::extract_token(&headers, q.token.as_deref());
        if !cfg.token_matches(token.as_deref()) {
            if is_index {
                return auth::gate_page(token.is_some()).into_response();
            }
            return (StatusCode::UNAUTHORIZED, "unauthorized").into_response();
        }
        // Valid token via query on index → set cookie for subsequent WS/assets.
        if is_index {
            if let Some(t) = token {
                if let Some((bytes, ct)) = load_file(state.webui_dir(), &rel) {
                    let bytes = inject_webui_version(bytes, &rel);
                    let mut res = (
                        StatusCode::OK,
                        [
                            (header::CONTENT_TYPE, ct),
                            (header::CACHE_CONTROL, "no-cache"),
                        ],
                        bytes,
                    )
                        .into_response();
                    if let Ok(val) = header::HeaderValue::from_str(&auth::set_cookie_header(&t)) {
                        res.headers_mut().insert(header::SET_COOKIE, val);
                    }
                    return res;
                }
            }
        }
    }

    match load_file(state.webui_dir(), &rel) {
        Some((bytes, ct)) => {
            debug!(%rel, "static ok");
            let bytes = inject_webui_version(bytes, &rel);
            let cache = cache_control_for(&rel);
            (
                StatusCode::OK,
                [(header::CONTENT_TYPE, ct), (header::CACHE_CONTROL, cache)],
                bytes,
            )
                .into_response()
        }
        None => (StatusCode::NOT_FOUND, "not found").into_response(),
    }
}
