use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Response};

use webdock_core::AppConfig;

/// Extract token from query, Authorization Bearer, or cookie `webdock_token`.
pub fn extract_token(headers: &HeaderMap, query_token: Option<&str>) -> Option<String> {
    if let Some(t) = query_token {
        if !t.is_empty() {
            return Some(t.to_string());
        }
    }
    if let Some(auth) = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
    {
        if let Some(rest) = auth.strip_prefix("Bearer ") {
            return Some(rest.trim().to_string());
        }
    }
    if let Some(cookie) = headers.get(header::COOKIE).and_then(|v| v.to_str().ok()) {
        for part in cookie.split(';') {
            let part = part.trim();
            // Accept both cookie names during migration from WebDock.
            if let Some(v) = part
                .strip_prefix("webrust_token=")
                .or_else(|| part.strip_prefix("webdock_token="))
            {
                let v = v.split(';').next().unwrap_or(v).trim();
                if !v.is_empty() {
                    // URL-decode basic percent encoding
                    return Some(urlencoding_decode(v));
                }
            }
        }
    }
    None
}

fn urlencoding_decode(s: &str) -> String {
    // Minimal decode for token chars; full percent-decode without extra crate.
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (from_hex(bytes[i + 1]), from_hex(bytes[i + 2])) {
                out.push((h << 4) | l);
                i += 3;
                continue;
            }
        }
        if bytes[i] == b'+' {
            out.push(b' ');
        } else {
            out.push(bytes[i]);
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn from_hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

pub fn check_access(
    config: &AppConfig,
    headers: &HeaderMap,
    query_token: Option<&str>,
    peer_ip: &str,
) -> Result<(), AuthFail> {
    if !config.is_ip_allowed(peer_ip) {
        return Err(AuthFail::ForbiddenIp);
    }
    if let Some(host) = headers
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .map(|h| h.split(':').next().unwrap_or(h))
    {
        if !config.is_domain_allowed(host) {
            return Err(AuthFail::ForbiddenHost);
        }
    }
    let token = extract_token(headers, query_token);
    if !config.token_matches(token.as_deref()) {
        return Err(AuthFail::Unauthorized {
            wrong: token.is_some(),
        });
    }
    Ok(())
}

/// WebSocket-only: Origin must match Host or allowed domains (blocks drive-by WS).
pub fn check_ws_origin(config: &AppConfig, headers: &HeaderMap) -> Result<(), AuthFail> {
    let origin = headers.get(header::ORIGIN).and_then(|v| v.to_str().ok());
    let host = headers.get(header::HOST).and_then(|v| v.to_str().ok());
    if config.is_origin_allowed(origin, host) {
        Ok(())
    } else {
        Err(AuthFail::ForbiddenOrigin)
    }
}

#[derive(Debug)]
pub enum AuthFail {
    Unauthorized { wrong: bool },
    ForbiddenIp,
    ForbiddenHost,
    ForbiddenOrigin,
}

impl AuthFail {
    pub fn into_response(self) -> Response {
        match self {
            AuthFail::Unauthorized { wrong } => gate_page(wrong).into_response(),
            AuthFail::ForbiddenIp | AuthFail::ForbiddenHost | AuthFail::ForbiddenOrigin => {
                (StatusCode::FORBIDDEN, "forbidden").into_response()
            }
        }
    }

    /// For WebSocket upgrade failures (no HTML redirect).
    pub fn into_ws_response(self) -> Response {
        match self {
            AuthFail::Unauthorized { .. } => {
                (StatusCode::UNAUTHORIZED, "unauthorized").into_response()
            }
            AuthFail::ForbiddenIp | AuthFail::ForbiddenHost | AuthFail::ForbiddenOrigin => {
                (StatusCode::FORBIDDEN, "forbidden").into_response()
            }
        }
    }
}

pub fn gate_page(wrong_token: bool) -> Html<String> {
    let message = if wrong_token {
        "토큰이 올바르지 않습니다. 다시 입력하세요."
    } else {
        "이 서버는 토큰이 필요합니다."
    };
    Html(format!(
        r#"<!doctype html><html lang="ko"><head>
<meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>WebRust — 접근 거부</title>
<link rel="icon" type="image/png" href="/favicon.png">
<style>
  body{{font-family:-apple-system,system-ui,sans-serif;background:#0a0b0d;color:#e8eef6;
       display:flex;min-height:100vh;align-items:center;justify-content:center;margin:0}}
  .card{{background:#14161a;border:1px solid #2a2e36;border-radius:12px;padding:28px 24px;width:min(360px,92vw)}}
  h1{{font-size:1.1rem;margin:0 0 8px}} p{{color:#9aa3b2;font-size:.9rem;margin:0 0 16px;line-height:1.45}}
  input{{width:100%;box-sizing:border-box;padding:10px 12px;border-radius:8px;border:1px solid #2a2e36;
        background:#0a0b0d;color:#e8eef6;font-size:1rem;margin-bottom:12px}}
  button{{width:100%;padding:10px;border:0;border-radius:8px;background:#3b82f6;color:#fff;font-weight:600;cursor:pointer}}
  button:hover{{filter:brightness(1.08)}}
</style></head><body><div class="card">
<h1>WebRust</h1>
<p>{message}</p>
<form method="POST" action="/" autocomplete="off">
  <input type="password" name="token" placeholder="토큰" required autofocus autocomplete="off">
  <button type="submit">접속</button>
</form>
</div></body></html>"#
    ))
}

pub fn set_cookie_header(token: &str) -> String {
    format!("webrust_token={token}; Path=/; SameSite=Strict; Max-Age=604800")
}
