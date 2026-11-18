//! Server HTTP Telemon untuk mode Serve (LAN) dan Publik: API JSON, login bercookie, WebSocket event,
//! dan penyajian UI. Semua logika bisnis ada di `telemon-core`; crate ini hanya membungkusnya.

mod api;
mod dto;
mod error;
mod guard;
mod state;
mod ws;

use std::future::Future;
use std::net::SocketAddr;
use std::sync::Arc;

use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::http::StatusCode;
use axum::middleware::{from_fn_with_state};
use axum::routing::get;
use telemon_core::Connector;
use tower_http::services::{ServeDir, ServeFile};

pub use error::ApiError;
pub use guard::{COOKIE, client_info, origin_ok};
pub use state::{AppState, HttpConfig, PlanCache, RateLimiter};

/// Router axum lengkap (API + UI). Sambungkan dengan `into_make_service_with_connect_info::<SocketAddr>()`.
pub fn app<C: Connector>(st: Arc<AppState<C>>) -> Router {
    let mut r: Router<Arc<AppState<C>>> = api::routes::<C>().route("/api/ws", get(ws::handler::<C>));
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
        .with_state(st)
}

/// Jalankan server pada `addr` sampai `shutdown` selesai.
pub async fn serve<C: Connector>(st: Arc<AppState<C>>, addr: SocketAddr, shutdown: impl Future<Output = ()> + Send + 'static) -> std::io::Result<()> {
    let listener = tokio::net::TcpListener::bind(addr).await?;
    serve_on(st, listener, shutdown).await
}

pub async fn serve_on<C: Connector>(st: Arc<AppState<C>>, listener: tokio::net::TcpListener, shutdown: impl Future<Output = ()> + Send + 'static) -> std::io::Result<()> {
    axum::serve(listener, app(st).into_make_service_with_connect_info::<SocketAddr>()).with_graceful_shutdown(shutdown).await
}
