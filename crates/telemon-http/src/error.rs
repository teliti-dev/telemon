//! Pemetaan galat inti ke respons HTTP. Detail internal tidak pernah dikirim ke klien.

use axum::Json;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde_json::json;
use telemon_core::auth::AuthError;
use telemon_core::service::ServiceError;
use telemon_core::store::RouterError;
use telemon_core::Error as RouterApiError;

#[derive(Debug)]
pub struct ApiError {
    pub status: StatusCode,
    pub code: &'static str,
    pub message: String,
    pub retry_after: Option<u32>,
}

impl ApiError {
    pub fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self { status, code, message: message.into(), retry_after: None }
    }
    pub fn bad(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "invalid", message)
    }
    pub fn unauthenticated() -> Self {
        Self::new(StatusCode::UNAUTHORIZED, "unauthenticated", "Belum login")
    }
    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::new(StatusCode::FORBIDDEN, "forbidden", message)
    }
    pub fn not_found() -> Self {
        Self::new(StatusCode::NOT_FOUND, "not_found", "Tidak ditemukan")
    }
    pub fn too_many(secs: u32) -> Self {
        let mut e = Self::new(StatusCode::TOO_MANY_REQUESTS, "rate_limited", format!("Terlalu banyak permintaan. Coba lagi dalam {secs} detik"));
        e.retry_after = Some(secs);
        e
    }
    /// Kesalahan di sisi server: dicatat di stderr, klien hanya menerima pesan umum.
    pub fn internal(detail: impl std::fmt::Display) -> Self {
        eprintln!("telemon-http: galat internal: {detail}");
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, "internal", "Terjadi kesalahan di server")
    }
}

impl From<AuthError> for ApiError {
    fn from(e: AuthError) -> Self {
        let (status, retry) = match &e {
            AuthError::Invalid(_) => (StatusCode::BAD_REQUEST, None),
            AuthError::BadCredentials | AuthError::BadCode { .. } | AuthError::Unauthenticated => (StatusCode::UNAUTHORIZED, None),
            AuthError::Locked { minutes } => (StatusCode::TOO_MANY_REQUESTS, Some(minutes * 60)),
            AuthError::Disabled | AuthError::Forbidden(_) => (StatusCode::FORBIDDEN, None),
            AuthError::NotFound => (StatusCode::NOT_FOUND, None),
            AuthError::VaultLocked => (StatusCode::SERVICE_UNAVAILABLE, None),
            AuthError::Db(d) => return ApiError::internal(d),
        };
        ApiError { status, code: e.code(), message: e.to_string(), retry_after: retry }
    }
}

impl From<ServiceError> for ApiError {
    fn from(e: ServiceError) -> Self {
        match e {
            ServiceError::Router(RouterApiError::Connect(m) | RouterApiError::Fatal(m)) => ApiError::new(StatusCode::BAD_GATEWAY, "router_unreachable", format!("Router tidak terjangkau: {m}")),
            ServiceError::Router(RouterApiError::Trap(m)) => ApiError::new(StatusCode::UNPROCESSABLE_ENTITY, "router_rejected", format!("Router menolak: {m}")),
            ServiceError::Router(RouterApiError::Rejected(m)) => ApiError::new(StatusCode::UNPROCESSABLE_ENTITY, "rejected", m),
            ServiceError::Config(RouterError::Invalid(m)) => ApiError::bad(m),
            ServiceError::Config(RouterError::NotFound) | ServiceError::NotFound => ApiError::not_found(),
            ServiceError::Config(RouterError::Db(d)) | ServiceError::Db(d) => ApiError::internal(d),
            ServiceError::Rejected(m) => ApiError::forbidden(m),
            ServiceError::Password(p) => match p {
                telemon_core::PasswordError::Locked => ApiError::new(StatusCode::SERVICE_UNAVAILABLE, "vault_locked", "Penyimpanan rahasia terkunci"),
                other => ApiError::internal(other),
            },
            ServiceError::Secret(m) => ApiError::internal(format!("kredensial: {m}")),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let mut r = (self.status, Json(json!({ "error": { "code": self.code, "message": self.message } }))).into_response();
        if let Some(s) = self.retry_after
            && let Ok(v) = HeaderValue::from_str(&s.to_string())
        {
            r.headers_mut().insert(header::RETRY_AFTER, v);
        }
        r
    }
}

pub type ApiResult<T> = Result<T, ApiError>;
