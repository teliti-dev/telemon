//! Pengaman permintaan: cookie sesi, asal klien (LAN atau Publik), pemeriksaan Origin (CSRF),
//! dan header keamanan.

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use axum::extract::{ConnectInfo, FromRequestParts, Request, State};
use axum::http::header::{self, HeaderMap, HeaderValue};
use axum::http::request::Parts;
use axum::http::Method;
use axum::middleware::Next;
use axum::response::Response;
use telemon_core::auth::{Session, Via};
use telemon_core::Connector;

use crate::error::ApiError;
use crate::state::AppState;

pub const COOKIE: &str = "telemon_session";

pub fn cookie_value(h: &HeaderMap, name: &str) -> Option<String> {
    h.get_all(header::COOKIE).iter().filter_map(|v| v.to_str().ok()).flat_map(|v| v.split(';')).find_map(|kv| {
        let (k, v) = kv.trim().split_once('=')?;
        (k == name && !v.is_empty()).then(|| v.to_string())
    })
}

#[derive(Debug, Clone)]
pub struct ClientInfo {
    pub ip: String,
    pub via: Via,
}

/// Akses lewat Cloudflare Quick Tunnel tiba dari sidecar di komputer ini (loopback) dengan header
/// `CF-Connecting-IP`. Header itu hanya dipercaya dari loopback; dari alamat lain diabaikan
/// sehingga tidak bisa dipalsukan oleh klien LAN.
pub fn client_info(h: &HeaderMap, peer: SocketAddr) -> ClientInfo {
    if peer.ip().is_loopback()
        && let Some(ip) = h.get("cf-connecting-ip").and_then(|v| v.to_str().ok()).and_then(|v| v.trim().parse::<IpAddr>().ok())
    {
        return ClientInfo { ip: ip.to_string(), via: Via::Publik };
    }
    ClientInfo { ip: peer.ip().to_string(), via: Via::Lan }
}

/// Permintaan dari halaman web lain (lintas-situs) ditolak; tanpa Origin (klien non-browser) diterima.
pub fn origin_ok(h: &HeaderMap) -> bool {
    if h.get("sec-fetch-site").and_then(|v| v.to_str().ok()) == Some("cross-site") {
        return false;
    }
    let Some(origin) = h.get(header::ORIGIN).and_then(|v| v.to_str().ok()) else { return true };
    let host = h.get(header::HOST).and_then(|v| v.to_str().ok()).unwrap_or("");
    let origin_host = origin.split_once("://").map_or(origin, |(_, rest)| rest);
    !host.is_empty() && origin_host.eq_ignore_ascii_case(host)
}

/// Tolak metode yang mengubah keadaan bila Origin tidak sama dengan Host.
pub async fn csrf<C: Connector>(State(_): State<Arc<AppState<C>>>, req: Request, next: Next) -> Result<Response, ApiError> {
    let safe = matches!(*req.method(), Method::GET | Method::HEAD | Method::OPTIONS);
    if !safe && !origin_ok(req.headers()) {
        return Err(ApiError::forbidden("Asal permintaan tidak diizinkan"));
    }
    Ok(next.run(req).await)
}

/// Header keamanan; respons API tidak boleh di-cache, halaman UI mendapat CSP.
pub async fn security_headers<C: Connector>(State(st): State<Arc<AppState<C>>>, req: Request, next: Next) -> Response {
    let is_api = req.uri().path().starts_with("/api/");
    let mut res = next.run(req).await;
    let h = res.headers_mut();
    h.insert(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    h.insert(header::REFERRER_POLICY, HeaderValue::from_static("no-referrer"));
    h.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    if is_api {
        h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    } else if let Some(csp) = st.cfg.csp.as_deref().and_then(|c| HeaderValue::from_str(c).ok()) {
        h.insert(header::CONTENT_SECURITY_POLICY, csp);
    }
    res
}

/// Pembuat cookie sesi. `Secure` untuk akses publik (HTTPS lewat Cloudflare) atau bila dipaksa.
pub fn session_cookie(token: &str, max_age_secs: u32, secure: bool) -> HeaderValue {
    let mut c = format!("{COOKIE}={token}; HttpOnly; SameSite=Strict; Path=/; Max-Age={max_age_secs}");
    if secure {
        c.push_str("; Secure");
    }
    HeaderValue::from_str(&c).expect("cookie valid")
}

pub fn clear_cookie(secure: bool) -> HeaderValue {
    let mut c = format!("{COOKIE}=deleted; HttpOnly; SameSite=Strict; Path=/; Max-Age=0");
    if secure {
        c.push_str("; Secure");
    }
    HeaderValue::from_str(&c).expect("cookie valid")
}

/// Pemanggil terautentikasi: sesi dari cookie (izin terkini dibaca ulang tiap permintaan).
pub struct Caller {
    pub session: Session,
    pub token: String,
    pub info: ClientInfo,
}

impl<C: Connector> FromRequestParts<Arc<AppState<C>>> for Caller {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, st: &Arc<AppState<C>>) -> Result<Self, ApiError> {
        let peer = parts.extensions.get::<ConnectInfo<SocketAddr>>().map(|c| c.0).ok_or_else(|| ApiError::internal("alamat klien tidak tersedia"))?;
        let token = cookie_value(&parts.headers, COOKIE).ok_or_else(ApiError::unauthenticated)?;
        let auth = st.auth.clone();
        let t = token.clone();
        let mut session = tokio::task::spawn_blocking(move || auth.authenticate(&t)).await.map_err(|e| ApiError::internal(e))??;
        let info = client_info(&parts.headers, peer);
        // Aturan akses publik mengikuti jalur permintaan ini, bukan jalur saat login: cookie yang dipakai
        // lewat tunnel publik tidak mendapat hak LAN.
        session.via = info.via;
        Ok(Caller { session, token, info })
    }
}

/// Alamat klien tanpa autentikasi (untuk login dan pembatasan laju).
pub struct Client(pub ClientInfo);

impl<S: Send + Sync> FromRequestParts<S> for Client {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, ApiError> {
        let peer = parts.extensions.get::<ConnectInfo<SocketAddr>>().map(|c| c.0).ok_or_else(|| ApiError::internal("alamat klien tidak tersedia"))?;
        Ok(Client(client_info(&parts.headers, peer)))
    }
}
