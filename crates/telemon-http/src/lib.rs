//! Server HTTP Telemon untuk mode Serve (LAN) dan Publik: API JSON, login bercookie, WebSocket event,
//! dan penyajian UI. Semua logika bisnis ada di `telemon-core`; crate ini hanya membungkusnya.

mod ai;
mod api;
mod dashboard;
mod dto;
mod error;
mod extensions;
mod guard;
mod share;
mod state;
mod traffic;
mod update;
mod ws;

use std::future::Future;
use std::net::SocketAddr;
use std::sync::Arc;

pub use axum::Router;
pub use telemon_ai::{AiService, HttpLlm, Llm};
use axum::extract::DefaultBodyLimit;
use axum::http::StatusCode;
use axum::middleware::{from_fn_with_state};
use axum::routing::get;
use telemon_core::Connector;
use tower_http::services::{ServeDir, ServeFile};

pub use error::ApiError;
pub use guard::{COOKIE, DESKTOP_COOKIE, client_info, origin_ok};
pub use share::{DEFAULT_LAN_PORT, Tunnel};
pub use update::{KEEP_BACKUPS, UpdateInfo, Updater, prune_backups};
pub use state::{AppState, ExtConfig, HttpConfig, PlanCache, RateLimiter};

/// Router axum lengkap (API + UI) untuk akses jarak jauh (LAN/Publik; login akun).
/// Sambungkan dengan `into_make_service_with_connect_info::<SocketAddr>()`.
pub fn app<C: Connector>(st: Arc<AppState<C>>) -> Router {
    build(st, guard::Surface::Remote)
}

/// Router untuk aplikasi desktop: **hanya untuk loopback**. Tanpa login akun; pemanggil memegang token
/// per peluncuran (dipasang sebagai cookie lewat `/desktop-entry?t=<token>`), dan menjadi pemilik
/// (semua izin, termasuk pengaturan Berbagi).
pub fn app_desktop<C: Connector>(st: Arc<AppState<C>>, token: &str) -> Router {
    build(st, guard::Surface::Desktop(token.into()))
}

fn build<C: Connector>(st: Arc<AppState<C>>, surface: guard::Surface) -> Router {
    let mut r: Router<Arc<AppState<C>>> = api::routes::<C>().route("/api/ws", get(ws::handler::<C>));
    if matches!(surface, guard::Surface::Desktop(_)) {
        r = r.route("/desktop-entry", get(api::desktop_entry::<C>));
    }
    // Jalur /api yang tidak dikenal: 404 JSON, bukan halaman UI.
    r = r.route("/api/{*rest}", axum::routing::any(|| async { ApiError::not_found() }));
    if let Some(dir) = st.cfg.static_dir.clone() {
        let index = dir.join("index.html");
        r = r.fallback_service(ServeDir::new(dir).fallback(ServeFile::new(index)));
    } else {
        r = r.fallback(|| async { StatusCode::NOT_FOUND });
    }
    r.layer(DefaultBodyLimit::max(st.cfg.max_body))
        .layer(from_fn_with_state(st.clone(), guard::csrf::<C>))
        .layer(from_fn_with_state(st.clone(), guard::security_headers::<C>))
        .layer(axum::Extension(surface))
        .with_state(st)
}

/// Jalankan server pada `addr` sampai `shutdown` selesai.
pub async fn serve<C: Connector>(st: Arc<AppState<C>>, addr: SocketAddr, shutdown: impl Future<Output = ()> + Send + 'static) -> std::io::Result<()> {
    let listener = tokio::net::TcpListener::bind(addr).await?;
    serve_on(st, listener, shutdown).await
}

/// Jalankan router yang sudah jadi (mis. `app_desktop`) pada listener yang sudah diikat.
pub async fn serve_router(listener: tokio::net::TcpListener, app: Router, shutdown: impl Future<Output = ()> + Send + 'static) -> std::io::Result<()> {
    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>()).with_graceful_shutdown(shutdown).await
}

pub async fn serve_on<C: Connector>(st: Arc<AppState<C>>, listener: tokio::net::TcpListener, shutdown: impl Future<Output = ()> + Send + 'static) -> std::io::Result<()> {
    axum::serve(listener, app(st).into_make_service_with_connect_info::<SocketAddr>()).with_graceful_shutdown(shutdown).await
}
