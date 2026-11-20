//! Rute AI (`/api/ai/*`, ADR 0007 dan 0024). Semua rute memakai izin yang sama dengan menu terkait:
//! pertanyaan dan riwayat butuh izin **AI chat**; dasbor dan provider mengikuti **Pengaturan** (Lihat/Ubah, dan tulis
//! lewat akses publik mengikuti `allow_public_write`). Kunci API tidak pernah dikirim ke klien.

use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::{Arc, Mutex};

use axum::Json;
use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{StatusCode, header};
use axum::response::Response;
use axum::routing::{get, post, put};
use serde::Deserialize;
use serde_json::{Value, json};
use telemon_ai::{AiBudgetInput, AiChat, AiError, AiMessage, AiProviderInput, AiService, AiStreamEvent, Kind, Lang};
use telemon_core::Connector;
use telemon_core::auth::{Level, Resource};
use tokio::sync::mpsc;

use crate::error::ApiError;
use crate::guard::Caller;
use crate::state::AppState;

type St<C> = State<Arc<AppState<C>>>;
type ApiResult<T> = Result<T, ApiError>;

/// Tempat layanan AI di `AppState` plus penghitung pertanyaan yang sedang berjalan per akun.
#[derive(Default)]
pub struct AiSlot {
    svc: Mutex<Option<Arc<AiService>>>,
    inflight: Mutex<HashMap<String, u32>>,
}

/// Pertanyaan bersamaan per akun; sisanya menunggu (429) agar satu akun tidak menghabiskan anggaran.
const MAX_INFLIGHT: u32 = 2;
pub(crate) const MAX_QUESTIONS_PER_MINUTE: u32 = 20;
const MAX_MESSAGES: usize = 40;
const MAX_TEXT: usize = 20_000;

impl<C: Connector> AppState<C> {
    /// Pasang layanan AI. Tanpa layanan, `features.ai` selalu false dan rute `/api/ai/*` menjawab 501.
    pub fn set_ai(&self, svc: Arc<AiService>) {
        *self.ai.svc.lock().expect("lock") = Some(svc);
    }

    /// Pasang layanan AI dengan klien model `llm` dan jalankan pembersihan riwayat chat per jam (retensi di Pengaturan).
    /// Harus dipanggil dari dalam runtime tokio.
    pub fn enable_ai(self: &Arc<Self>, llm: Arc<dyn telemon_ai::Llm>) -> Arc<AiService> {
        let svc = Arc::new(AiService::new(self.telemon.store(), self.telemon.secrets().clone(), self.auth.clone(), llm));
        self.set_ai(svc.clone());
        let (weak_st, task_svc) = (Arc::downgrade(self), svc.clone());
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(std::time::Duration::from_secs(3600));
            loop {
                tick.tick().await;
                let Some(st) = weak_st.upgrade() else { break };
                let days = st.telemon.with_store(|s| s.get_settings()).ok().and_then(|s| s.retention.ai_chat_days);
                let _ = task_svc.purge_chats(days);
            }
        });
        svc
    }

    /// Layanan AI terpasang (untuk menyiapkan data awal, mis. provider demo).
    pub fn ai_service(&self) -> Option<Arc<AiService>> {
        self.ai()
    }

    pub(crate) fn ai(&self) -> Option<Arc<AiService>> {
        self.ai.svc.lock().expect("lock").clone()
    }

    /// Untuk `features.ai` pada status berbagi: layanan terpasang, dinyalakan pemilik, dan ada provider berkunci.
    pub(crate) fn ai_ready(&self) -> bool {
        self.ai().is_some_and(|s| s.ready())
    }
}

pub fn routes<C: Connector>() -> axum::Router<Arc<AppState<C>>> {
    axum::Router::new()
        .route("/api/ai/dashboard", get(dashboard::<C>))
        .route("/api/ai/providers", post(provider_create::<C>))
        .route("/api/ai/providers/{id}", put(provider_update::<C>).delete(provider_delete::<C>))
        .route("/api/ai/providers/{id}/default", post(provider_default::<C>))
        .route("/api/ai/providers/{id}/test", post(provider_test::<C>))
        .route("/api/ai/budget", put(budget_put::<C>))
        .route("/api/ai/ask", post(ask::<C>))
        .route("/api/ai/chats", get(chats_list::<C>))
        .route("/api/ai/chats/{id}", get(chat_get::<C>).put(chat_put::<C>).delete(chat_delete::<C>))
}

fn svc<C: Connector>(st: &AppState<C>) -> ApiResult<Arc<AiService>> {
    st.ai().ok_or_else(|| ApiError::new(StatusCode::NOT_IMPLEMENTED, "not_implemented", "AI belum tersedia di aplikasi ini"))
}

fn need<C: Connector>(st: &AppState<C>, c: &Caller, r: Resource, l: Level) -> ApiResult<()> {
    st.auth.authorize(Some(&c.session), r, l).map_err(Into::into)
}

fn account(c: &Caller) -> String {
    if c.desktop { "desktop".into() } else { c.session.account_id.clone() }
}

impl From<AiError> for ApiError {
    fn from(e: AiError) -> Self {
        let (status, code) = match e.kind {
            Kind::Invalid => (StatusCode::BAD_REQUEST, "invalid"),
            Kind::NotFound => (StatusCode::NOT_FOUND, "not_found"),
            Kind::Forbidden => (StatusCode::FORBIDDEN, "forbidden"),
            Kind::Unavailable => (StatusCode::CONFLICT, "ai_unavailable"),
            Kind::Upstream => (StatusCode::BAD_GATEWAY, "ai_upstream"),
        };
        ApiError::new(status, code, e.message)
    }
}

#[derive(Deserialize)]
struct DaysQ {
    days: Option<i64>,
}

async fn dashboard<C: Connector>(State(st): St<C>, c: Caller, Query(q): Query<DaysQ>) -> ApiResult<Json<Value>> {
    need(&st, &c, Resource::Pengaturan, Level::Read)?;
    let d = svc(&st)?.dashboard(q.days.unwrap_or(30))?;
    Ok(Json(serde_json::to_value(d).map_err(ApiError::internal)?))
}

/// URL dasar kustom (9Router) membuat server mengirim permintaan ke alamat itu; membiarkan akun jarak jauh
/// mengaturnya membuka celah SSRF ke jaringan dalam. Karena itu hanya aplikasi desktop (pemilik) yang boleh
/// mengubahnya; akun jarak jauh boleh menyimpan ulang provider selama URL-nya tidak berubah.
fn guard_base_url<C: Connector>(st: &AppState<C>, c: &Caller, existing_id: Option<&str>, input: &AiProviderInput) -> ApiResult<()> {
    if c.desktop {
        return Ok(());
    }
    let wanted = input.base_url.as_deref().map(str::trim).map(|u| u.trim_end_matches('/')).filter(|u| !u.is_empty());
    let Some(wanted) = wanted else { return Ok(()) };
    let current = existing_id.and_then(|id| svc(st).ok()?.providers().ok()?.into_iter().find(|p| p.id == id)).and_then(|p| p.base_url);
    if current.as_deref() == Some(wanted) {
        return Ok(());
    }
    Err(ApiError::forbidden("URL dasar provider hanya bisa diatur dari aplikasi desktop"))
}

async fn provider_create<C: Connector>(State(st): St<C>, c: Caller, Json(input): Json<AiProviderInput>) -> ApiResult<Json<Value>> {
    need(&st, &c, Resource::Pengaturan, Level::Update)?;
    guard_base_url(&st, &c, None, &input)?;
    let p = svc(&st)?.save_provider(None, input)?;
    Ok(Json(serde_json::to_value(p).map_err(ApiError::internal)?))
}

async fn provider_update<C: Connector>(State(st): St<C>, c: Caller, Path(id): Path<String>, Json(input): Json<AiProviderInput>) -> ApiResult<Json<Value>> {
    need(&st, &c, Resource::Pengaturan, Level::Update)?;
    guard_base_url(&st, &c, Some(&id), &input)?;
    let p = svc(&st)?.save_provider(Some(&id), input)?;
    Ok(Json(serde_json::to_value(p).map_err(ApiError::internal)?))
}

async fn provider_delete<C: Connector>(State(st): St<C>, c: Caller, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    need(&st, &c, Resource::Pengaturan, Level::Update)?;
    svc(&st)?.remove_provider(&id)?;
    Ok(Json(json!({ "ok": true })))
}

async fn provider_default<C: Connector>(State(st): St<C>, c: Caller, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    need(&st, &c, Resource::Pengaturan, Level::Update)?;
    svc(&st)?.set_default(&id)?;
    Ok(Json(json!({ "ok": true })))
}

async fn provider_test<C: Connector>(State(st): St<C>, c: Caller, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    need(&st, &c, Resource::Pengaturan, Level::Update)?;
    let p = svc(&st)?.test_provider(&id).await?;
    Ok(Json(serde_json::to_value(p).map_err(ApiError::internal)?))
}

async fn budget_put<C: Connector>(State(st): St<C>, c: Caller, Json(input): Json<AiBudgetInput>) -> ApiResult<Json<Value>> {
    need(&st, &c, Resource::Pengaturan, Level::Update)?;
    svc(&st)?.save_budget(input)?;
    Ok(Json(json!({ "ok": true })))
}

// ---------------------------------------------------------------- pertanyaan (NDJSON)

#[derive(Deserialize)]
struct AskReq {
    messages: Vec<AiMessage>,
    #[serde(default)]
    lang: Option<String>,
}

enum Item {
    Event(AiStreamEvent),
    Done(Result<(), AiError>),
}

/// Melepas hitungan "sedang berjalan" saat pertanyaan selesai, apa pun hasilnya.
struct Slot {
    st: Arc<dyn Fn() + Send + Sync>,
}

impl Drop for Slot {
    fn drop(&mut self) {
        (self.st)();
    }
}

/// Jawaban dikirim sebagai satu objek JSON per baris (`application/x-ndjson`). Galat yang terjadi sebelum
/// jawaban dimulai (izin, AI mati, anggaran) menjadi respons HTTP biasa; galat sesudahnya menjadi event `error`.
async fn ask<C: Connector>(State(st): St<C>, c: Caller, Json(req): Json<AskReq>) -> ApiResult<Response> {
    need(&st, &c, Resource::Ai, Level::Read)?;
    let svc = svc(&st)?;
    if req.messages.is_empty() || req.messages.len() > MAX_MESSAGES || req.messages.iter().map(|m| m.text.chars().count()).sum::<usize>() > MAX_TEXT {
        return Err(ApiError::bad("Pertanyaan kosong atau terlalu panjang"));
    }
    let acc = account(&c);
    if let Err(secs) = st.ai_rate.check(&acc, st.now()) {
        return Err(ApiError::too_many(secs));
    }
    {
        let mut m = st.ai.inflight.lock().expect("lock");
        let n = m.entry(acc.clone()).or_insert(0);
        if *n >= MAX_INFLIGHT {
            return Err(ApiError::new(StatusCode::TOO_MANY_REQUESTS, "rate_limited", "Masih ada pertanyaan yang sedang dijawab; tunggu sebentar"));
        }
        *n += 1;
    }
    let st2 = st.clone();
    let acc2 = acc.clone();
    let slot = Slot { st: Arc::new(move || {
        let mut m = st2.ai.inflight.lock().expect("lock");
        if let Some(n) = m.get_mut(&acc2) {
            *n = n.saturating_sub(1);
            if *n == 0 {
                m.remove(&acc2);
            }
        }
    }) };

    let (tx, mut rx) = mpsc::unbounded_channel::<Item>();
    let session = c.session.clone();
    let desktop = c.desktop;
    let lang = Lang::parse(req.lang.as_deref());
    tokio::spawn(async move {
        let _slot = slot;
        let caller = if desktop { None } else { Some(&session) };
        let tx_emit = tx.clone();
        let r = svc.ask(caller, &acc, &req.messages, lang, &mut |e| {
            let _ = tx_emit.send(Item::Event(e));
        })
        .await;
        let _ = tx.send(Item::Done(r));
    });

    // Galat sebelum event pertama = respons HTTP biasa.
    let first = rx.recv().await;
    let first_line = match first {
        Some(Item::Done(Err(e))) => return Err(e.into()),
        Some(Item::Event(e)) => Some(line(&e)),
        _ => None,
    };
    let stream = Lines { first: first_line, rx };
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/x-ndjson")
        .header(header::CACHE_CONTROL, "no-store")
        .header("x-accel-buffering", "no")
        .body(Body::from_stream(stream))
        .map_err(ApiError::internal)
}

fn line(e: &AiStreamEvent) -> String {
    format!("{}\n", serde_json::to_string(e).unwrap_or_else(|_| "{}".into()))
}

/// Aliran baris NDJSON dari saluran; galat di tengah jalan menjadi event `error` lalu aliran berakhir.
struct Lines {
    first: Option<String>,
    rx: mpsc::UnboundedReceiver<Item>,
}

impl futures_core::Stream for Lines {
    type Item = Result<String, Infallible>;

    fn poll_next(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Option<Self::Item>> {
        use std::task::Poll;
        if let Some(l) = self.first.take() {
            return Poll::Ready(Some(Ok(l)));
        }
        match self.rx.poll_recv(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Some(Item::Event(e))) => Poll::Ready(Some(Ok(line(&e)))),
            Poll::Ready(Some(Item::Done(Err(e)))) => {
                self.rx.close();
                Poll::Ready(Some(Ok(line(&AiStreamEvent::Error { message: e.message }))))
            }
            Poll::Ready(Some(Item::Done(Ok(())))) | Poll::Ready(None) => Poll::Ready(None),
        }
    }
}

// ---------------------------------------------------------------- riwayat chat

async fn chats_list<C: Connector>(State(st): St<C>, c: Caller) -> ApiResult<Json<Value>> {
    need(&st, &c, Resource::Ai, Level::Read)?;
    Ok(Json(serde_json::to_value(svc(&st)?.chats(&account(&c))?).map_err(ApiError::internal)?))
}

async fn chat_get<C: Connector>(State(st): St<C>, c: Caller, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    need(&st, &c, Resource::Ai, Level::Read)?;
    Ok(Json(serde_json::to_value(svc(&st)?.chat(&account(&c), &id)?).map_err(ApiError::internal)?))
}

async fn chat_put<C: Connector>(State(st): St<C>, c: Caller, Path(id): Path<String>, Json(mut chat): Json<AiChat>) -> ApiResult<Json<Value>> {
    need(&st, &c, Resource::Ai, Level::Read)?;
    chat.id = id;
    Ok(Json(serde_json::to_value(svc(&st)?.save_chat(&account(&c), &chat)?).map_err(ApiError::internal)?))
}

async fn chat_delete<C: Connector>(State(st): St<C>, c: Caller, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    need(&st, &c, Resource::Ai, Level::Read)?;
    svc(&st)?.delete_chat(&account(&c), &id)?;
    Ok(Json(json!({ "ok": true })))
}
