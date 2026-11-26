//! Backend WASM extension (ADR 0028): rute `/api/ext/<id>/...`, host API, dan worker terjadwal.
//!
//! Modul dijalankan oleh `telemon_ext::Sandbox` (interpreter, bahan bakar, batas memori). Semua yang dilakukan modul
//! lewat `ExtHost`, yang menegakkan **izin extension** (toggle yang diberikan admin) dan, untuk panggilan atas nama
//! pengguna, **izin peran pengguna itu**. Worker berjalan hanya dengan izin extension.

use std::collections::HashMap;
use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::{Path, RawQuery, State};
use axum::http::{Method, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{any, post};
use axum::{Json, Router};
use serde::Serialize;
use serde_json::{Value, json};
use telemon_core::auth::{Level, Resource, Session, Via};
use telemon_core::{AuditEntry, AuditOutcome, Change, Connector, DbValue, EventQuery, RouterApi};
use telemon_ext::{Host, Manifest, Package, RunLimits, RuntimeError, Sandbox, namespace, valid_id};

use crate::error::{ApiError, ApiResult};
use crate::extensions::{all_rows, blocked_by, effective, lvl, manage, manifest_of, path_of, status};
use crate::guard::Caller;
use crate::state::AppState;

type St<C> = Arc<AppState<C>>;

pub fn routes<C: Connector>() -> Router<St<C>> {
    Router::new()
        .route("/api/ext/{id}/{*path}", any(route::<C>))
        .route("/api/extensions/{id}/logs", axum::routing::get(logs::<C>))
        .route("/api/extensions/{id}/workers/{worker}/run", post(run_worker_now::<C>))
}

/// Jalur `print` yang boleh dibaca extension (tanpa kata sandi atau rahasia). Pencocokan persis.
const PRINT_PATHS: &[&str] = &["/system/resource", "/system/health", "/system/identity", "/system/routerboard", "/system/clock", "/system/package", "/interface", "/interface/ethernet", "/ip/address", "/ip/route", "/ppp/active"];

const LOG_LINES: usize = 200;

pub(crate) fn push_log<C: Connector>(st: &St<C>, id: &str, line: String) {
    let mut g = st.ext_logs.lock().expect("lock");
    let q = g.entry(id.to_string()).or_default();
    if q.len() >= LOG_LINES {
        q.pop_front();
    }
    q.push_back(format!("{} {}", st.now(), line.chars().take(500).collect::<String>()));
}

// ---------------- host API ----------------

struct ExtHost<C: Connector> {
    st: St<C>,
    id: String,
    manifest: Arc<Manifest>,
    granted: Vec<String>,
    caller: Option<Session>,
    rt: tokio::runtime::Handle,
}

fn ok(v: Value) -> Vec<u8> {
    json!({ "ok": v }).to_string().into_bytes()
}

fn err(code: &str, message: impl Into<String>) -> Vec<u8> {
    json!({ "error": { "code": code, "message": message.into() } }).to_string().into_bytes()
}

fn db_args(v: Option<&Value>) -> Result<Vec<DbValue>, String> {
    let Some(a) = v else { return Ok(vec![]) };
    let a = a.as_array().ok_or("params harus berupa array")?;
    if a.len() > 32 {
        return Err("Terlalu banyak parameter".into());
    }
    a.iter()
        .map(|x| match x {
            Value::Null => Ok(DbValue::Null),
            Value::Bool(b) => Ok(DbValue::Int(*b as i64)),
            Value::Number(n) => Ok(n.as_i64().map(DbValue::Int).or_else(|| n.as_f64().map(DbValue::Real)).unwrap_or(DbValue::Null)),
            Value::String(s) => Ok(DbValue::Text(s.clone())),
            _ => Err("Parameter harus null, boolean, angka, atau teks".to_string()),
        })
        .collect()
}

fn db_json(v: &DbValue) -> Value {
    match v {
        DbValue::Null => Value::Null,
        DbValue::Int(i) => json!(i),
        DbValue::Real(f) => json!(f),
        DbValue::Text(t) => json!(t),
        DbValue::Blob(n) => json!({ "blob": n }),
    }
}

impl<C: Connector> ExtHost<C> {
    /// Izin extension (toggle) dan, bila ada pengguna, izin peran pengguna pada sumber datanya.
    fn need(&self, perm: &str, resource: Resource) -> Result<(), Vec<u8>> {
        if !self.manifest.permissions.iter().any(|p| p.id == perm) || !self.granted.iter().any(|g| g == perm) {
            return Err(err("permission_denied", format!("Izin {perm} tidak diberikan ke extension ini")));
        }
        if let Some(s) = &self.caller {
            if self.st.auth.authorize(Some(s), resource, Level::Read).is_err() {
                return Err(err("permission_denied", "Peran pengguna tidak boleh membaca data ini"));
            }
        }
        Ok(())
    }

    fn dispatch(&mut self, op: &str, input: &Value) -> Result<Value, Vec<u8>> {
        let s = |k: &str| input.get(k).and_then(Value::as_str);
        let ns = namespace(&self.id);
        match op {
            "time.now" => Ok(json!(self.st.now())),
            "log" => {
                push_log(&self.st, &self.id, s("message").unwrap_or("").to_string());
                Ok(Value::Null)
            }
            "kv.get" => {
                let key = s("key").ok_or_else(|| err("bad_request", "key wajib"))?;
                let v = self.st.telemon.with_store(|d| d.ext_kv_get(&self.id, key)).map_err(|x| err("db", x.to_string()))?;
                Ok(v.map_or(Value::Null, Value::String))
            }
            "kv.set" => {
                let (key, value) = (s("key").ok_or_else(|| err("bad_request", "key wajib"))?, s("value").ok_or_else(|| err("bad_request", "value (teks) wajib"))?);
                let now = self.st.now();
                self.st.telemon.with_store(|d| d.ext_kv_set(&self.id, key, value, now)).map_err(|m| err("quota", m))?;
                Ok(Value::Null)
            }
            "kv.delete" => {
                let key = s("key").ok_or_else(|| err("bad_request", "key wajib"))?;
                let r = self.st.telemon.with_store(|d| d.ext_kv_delete(&self.id, key)).map_err(|x| err("db", x.to_string()))?;
                Ok(json!(r))
            }
            "kv.list" => {
                let limit = input.get("limit").and_then(Value::as_u64).unwrap_or(100) as usize;
                let keys = self.st.telemon.with_store(|d| d.ext_kv_list(&self.id, s("prefix").unwrap_or(""), limit)).map_err(|x| err("db", x.to_string()))?;
                Ok(json!(keys))
            }
            "db.query" => {
                let sql = s("sql").ok_or_else(|| err("bad_request", "sql wajib"))?;
                let args = db_args(input.get("params")).map_err(|m| err("bad_request", m))?;
                let limit = (input.get("limit").and_then(Value::as_u64).unwrap_or(200) as usize).clamp(1, 1000);
                let r = self.st.telemon.with_store(|d| d.ext_db_query(&ns, sql, &args, limit)).map_err(|x| err("db", x.to_string()))?;
                Ok(json!({ "columns": r.columns, "rows": r.rows.iter().map(|row| row.iter().map(db_json).collect::<Vec<_>>()).collect::<Vec<_>>() }))
            }
            "db.exec" => {
                let sql = s("sql").ok_or_else(|| err("bad_request", "sql wajib"))?;
                let args = db_args(input.get("params")).map_err(|m| err("bad_request", m))?;
                let n = self.st.telemon.with_store(|d| d.ext_db_exec(&ns, sql, &args)).map_err(|x| err("db", x.to_string()))?;
                Ok(json!({ "changes": n }))
            }
            "routers.list" => {
                self.need("data.routers", Resource::Router)?;
                let rows = self.st.telemon.with_store(|d| d.list_routers()).map_err(|x| err("db", x.to_string()))?;
                Ok(json!(rows.iter().map(|r| json!({ "id": r.id, "name": r.name, "model": r.model })).collect::<Vec<_>>()))
            }
            "router.print" => {
                self.need("data.routers", Resource::Router)?;
                let (router, path) = (s("router").ok_or_else(|| err("bad_request", "router wajib"))?, s("path").ok_or_else(|| err("bad_request", "path wajib"))?);
                if !PRINT_PATHS.contains(&path) {
                    return Err(err("path_not_allowed", format!("Jalur {path} tidak boleh dibaca extension")));
                }
                let client = self.st.telemon.client(router).map_err(|x| err("router", x.to_string()))?;
                let rows = self
                    .rt
                    .block_on(async { tokio::time::timeout(std::time::Duration::from_secs(10), client.print(path)).await })
                    .map_err(|_| err("timeout", "Router tidak menjawab dalam 10 detik"))?
                    .map_err(|x| err("router", x.to_string()))?;
                let clean = |k: &str| !(k.contains("password") || k.contains("secret") || k.contains("private"));
                Ok(json!(rows.iter().map(|r| r.iter().filter(|(k, _)| clean(k)).collect::<HashMap<_, _>>()).collect::<Vec<_>>()))
            }
            "events.list" => {
                self.need("data.events", Resource::Event)?;
                let kinds: Vec<telemon_core::EventKind> = input.get("kinds").and_then(Value::as_array).map(|a| a.iter().filter_map(|k| k.as_str().and_then(telemon_core::EventKind::parse)).collect()).unwrap_or_default();
                let q = EventQuery { router_id: s("router").map(str::to_string), kinds, since_ms: input.get("sinceMs").and_then(Value::as_i64), limit: (input.get("limit").and_then(Value::as_u64).unwrap_or(50) as usize).clamp(1, 200), ..Default::default() };
                let rows = self.st.telemon.with_store(|d| d.list_events(&q)).map_err(|x| err("db", x.to_string()))?;
                Ok(json!(rows.iter().map(|r| json!({ "id": r.id, "router": r.router_id, "kind": r.event.kind.as_str(), "at": r.event.at_ms, "username": r.event.username, "message": r.event.message })).collect::<Vec<_>>()))
            }
            other => Err(err("unknown_op", format!("Operasi host tidak dikenal: {other}"))),
        }
    }
}

impl<C: Connector> Host for ExtHost<C> {
    fn call(&mut self, op: &str, input: &[u8]) -> Vec<u8> {
        let Ok(v) = serde_json::from_slice::<Value>(if input.is_empty() { b"{}" } else { input }) else { return err("bad_request", "Masukan bukan JSON") };
        match self.dispatch(op, &v) {
            Ok(r) => ok(r),
            Err(e) => e,
        }
    }
}

// ---------------- menjalankan modul ----------------

/// Muat modul extension (diverifikasi ulang dari disk, di-cache per versi).
pub(crate) fn sandbox<C: Connector>(st: &St<C>, id: &str, version: &str) -> Option<Arc<Sandbox>> {
    if let Some((v, s)) = st.ext_wasm.lock().expect("lock").get(id)
        && v == version
    {
        return Some(s.clone());
    }
    let cfg = st.cfg.extensions.as_ref()?;
    let bytes = std::fs::read(path_of(&cfg.dir, id)).ok()?;
    let p = Package::open(&bytes, &cfg.keys, &telemon_ext::Limits::default()).ok()?;
    if p.tier != telemon_ext::Tier::Official || p.manifest.id != id || p.manifest.version != version {
        return None;
    }
    let sb = Arc::new(Sandbox::new(p.file(&p.manifest.backend.as_ref()?.entry)?).ok()?);
    st.ext_wasm.lock().expect("lock").insert(id.to_string(), (version.to_string(), sb.clone()));
    Some(sb)
}

enum Fail {
    Timeout,
    Backend(String),
}

/// Jalankan satu pemanggilan di thread blokir dan kembalikan jawaban JSON modul.
async fn invoke<C: Connector>(st: &St<C>, row: &telemon_core::StoredExtension, manifest: Arc<Manifest>, granted: Vec<String>, caller: Option<Session>, request: Value) -> Result<Value, Fail> {
    let (st2, st3, id, version) = (st.clone(), st.clone(), row.id.clone(), row.version.clone());
    let rt = tokio::runtime::Handle::current();
    let req = request.to_string().into_bytes();
    let res = tokio::task::spawn_blocking(move || -> Result<Vec<u8>, RuntimeError> {
        let sb = sandbox(&st2, &id, &version).ok_or_else(|| RuntimeError::Invalid("modul backend tidak bisa dimuat".into()))?;
        let host = ExtHost { st: st2, id, manifest, granted, caller, rt };
        let mut limits = RunLimits::default();
        if let Some(f) = st3.cfg.extensions.as_ref().map(|c| c.fuel).filter(|f| *f > 0) {
            limits.fuel = f;
        }
        sb.invoke(Box::new(host), &req, &limits)
    })
    .await
    .map_err(|x| Fail::Backend(x.to_string()))?;
    match res {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(|_| Fail::Backend("jawaban backend bukan JSON".into())),
        Err(RuntimeError::OutOfFuel) => Err(Fail::Timeout),
        Err(x) => Err(Fail::Backend(x.to_string())),
    }
}

pub(crate) fn granted_of<C: Connector>(st: &St<C>, id: &str) -> Vec<String> {
    st.telemon.with_store(|d| d.ext_perms(id)).unwrap_or_default().into_iter().filter(|(_, g)| *g).map(|(k, _)| k).collect()
}

/// Extension yang boleh dijalankan: aktif, official, punya backend, syarat terpenuhi, versi Telemon cocok.
fn runnable<C: Connector>(st: &St<C>, id: &str) -> Option<(telemon_core::StoredExtension, Arc<Manifest>)> {
    usable(st, id, true)
}

/// Seperti `runnable`, dengan `need_backend = false` untuk extension yang hanya punya UI dan sesi.
pub(crate) fn usable<C: Connector>(st: &St<C>, id: &str, need_backend: bool) -> Option<(telemon_core::StoredExtension, Arc<Manifest>)> {
    let rows = all_rows(st).ok()?;
    let row = rows.iter().find(|r| r.id == id)?.clone();
    let m = manifest_of(&row).ok()?;
    let perms = st.telemon.with_store(|d| d.ext_perms(id)).ok()?;
    let ok = row.enabled && row.tier == "official" && (m.backend.is_some() || !need_backend) && matches!(status(&row, &m, &perms, &blocked_by(&m, &rows), &st.cfg.host_version), "ok");
    ok.then(|| (row, Arc::new(m)))
}

// ---------------- rute HTTP ----------------

fn parse_query(q: Option<String>) -> Value {
    let mut m = serde_json::Map::new();
    for kv in q.unwrap_or_default().split('&').filter(|s| !s.is_empty()).take(50) {
        let (k, v) = kv.split_once('=').unwrap_or((kv, ""));
        let dec = |s: &str| {
            let b = s.as_bytes();
            let (mut out, mut i) = (Vec::new(), 0);
            while i < b.len() {
                match b[i] {
                    b'+' => out.push(b' '),
                    b'%' if i + 2 < b.len() => match u8::from_str_radix(&s[i + 1..i + 3], 16) {
                        Ok(x) => {
                            out.push(x);
                            i += 2
                        }
                        Err(_) => out.push(b'%'),
                    },
                    c => out.push(c),
                }
                i += 1;
            }
            String::from_utf8_lossy(&out).chars().take(500).collect::<String>()
        };
        m.insert(dec(k), Value::String(dec(v)));
    }
    Value::Object(m)
}

async fn route<C: Connector>(State(st): State<St<C>>, c: Caller, method: Method, Path((id, path)): Path<(String, String)>, RawQuery(query): RawQuery, body: Bytes) -> ApiResult<Response> {
    if st.cfg.extensions.is_none() || !valid_id(&id) {
        return Err(ApiError::not_found());
    }
    let (row, m) = runnable(&st, &id).ok_or_else(ApiError::not_found)?;
    let full = format!("/{path}");
    let r = m.contributes.routes.iter().find(|r| r.method == method.as_str() && r.path == full).ok_or_else(ApiError::not_found)?.clone();
    let (level, actions) = effective(&st, &c, &id, &m);
    if level < lvl(&r.level) || level == 0 {
        return Err(ApiError::forbidden("Peran Anda tidak punya akses ke fitur ini"));
    }
    if let Some(a) = &r.action {
        if !actions.contains(a) {
            return Err(ApiError::forbidden("Peran Anda tidak boleh melakukan aksi ini"));
        }
    }
    if r.level == "update" && c.session.via == Via::Publik {
        return Err(ApiError::forbidden("Aksi ini tidak tersedia lewat akses Publik"));
    }
    let body_json = if body.is_empty() { Value::Null } else { serde_json::from_slice(&body).map_err(|_| ApiError::bad("Badan permintaan bukan JSON"))? };
    let request = json!({
        "kind": "http", "route": r.id, "method": r.method, "path": r.path, "query": parse_query(query), "body": body_json,
        "user": if c.desktop { Value::Null } else { json!({ "id": c.session.account_id, "username": c.session.username, "displayName": c.session.display_name, "roleId": c.session.role_id }) },
    });
    let granted = granted_of(&st, &id);
    let caller = Some(c.session.clone());
    let out = invoke(&st, &row, m.clone(), granted, caller, request).await.map_err(|f| match f {
        Fail::Timeout => ApiError::new(StatusCode::GATEWAY_TIMEOUT, "backend_timeout", "Backend extension melewati batas waktu"),
        Fail::Backend(msg) => {
            push_log(&st, &id, format!("galat backend: {msg}"));
            ApiError::new(StatusCode::BAD_GATEWAY, "backend_error", "Backend extension gagal")
        }
    })?;
    if r.method != "GET" {
        let entry = AuditEntry { at_ms: st.now(), actor: c.session.username.clone(), router_id: String::new(), action: "extension.call".into(), target: id.clone(), title: format!("Panggil {} {} pada extension {}", r.method, r.path, m.name), changes: Vec::<Change>::new(), outcome: AuditOutcome::Ok };
        let _ = st.telemon.with_store(|d| d.record_audit(&entry));
    }
    let status = out.get("status").and_then(Value::as_u64).and_then(|s| StatusCode::from_u16(s as u16).ok()).unwrap_or(StatusCode::OK);
    if !(status.is_success() || status.is_client_error()) {
        return Err(ApiError::new(StatusCode::BAD_GATEWAY, "backend_bad_response", "Backend extension memberi status tidak valid"));
    }
    Ok((status, Json(out.get("body").cloned().unwrap_or(Value::Null))).into_response())
}

async fn logs<C: Connector>(State(st): State<St<C>>, c: Caller, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    manage(&st, &c, Level::Read)?;
    let lines: Vec<String> = st.ext_logs.lock().expect("lock").get(&id).map(|q| q.iter().cloned().collect()).unwrap_or_default();
    Ok(Json(json!({ "lines": lines })))
}

// ---------------- worker terjadwal ----------------

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkerState {
    pub next_run: i64,
    pub last_run: Option<i64>,
    pub last_ok: Option<bool>,
    pub last_error: Option<String>,
    pub failures: u32,
    pub paused: bool,
}

const MAX_FAILURES: u32 = 5;

/// Status worker satu extension untuk tampilan.
pub(crate) fn worker_view<C: Connector>(st: &St<C>, m: &Manifest) -> Vec<Value> {
    let g = st.ext_workers.lock().expect("lock");
    m.contributes.workers.iter().map(|w| json!({ "id": w.id, "everySeconds": w.every_seconds, "state": g.get(&format!("{}/{}", m.id, w.id)).cloned().unwrap_or_default() })).collect()
}

/// Jalankan satu worker sekarang; memperbarui keadaan (gagal berulang menjeda worker).
async fn run_worker<C: Connector>(st: &St<C>, id: &str, worker: &str) -> Result<(), String> {
    let (row, m) = runnable(st, id).ok_or("extension tidak aktif")?;
    let w = m.contributes.workers.iter().find(|w| w.id == worker).ok_or("worker tidak dikenal")?.clone();
    let key = format!("{id}/{worker}");
    let now = st.now();
    let granted = granted_of(st, id);
    let result = invoke(st, &row, m.clone(), granted, None, json!({ "kind": "worker", "worker": worker, "now": now })).await;
    let mut g = st.ext_workers.lock().expect("lock");
    let s = g.entry(key).or_default();
    s.last_run = Some(now);
    match result {
        Ok(_) => {
            s.last_ok = Some(true);
            s.last_error = None;
            s.failures = 0;
            s.next_run = now + w.every_seconds as i64 * 1000;
            Ok(())
        }
        Err(f) => {
            let msg = match f {
                Fail::Timeout => "melewati batas waktu CPU".to_string(),
                Fail::Backend(m) => m,
            };
            s.last_ok = Some(false);
            s.last_error = Some(msg.clone());
            s.failures += 1;
            s.next_run = now + w.every_seconds as i64 * 1000 * s.failures.min(6) as i64;
            if s.failures >= MAX_FAILURES {
                s.paused = true;
            }
            drop(g);
            push_log(st, id, format!("worker {worker} gagal ({msg})"));
            Err(msg)
        }
    }
}

/// Jalankan semua worker yang jatuh tempo (dipanggil penjadwal tiap beberapa detik; dipanggil langsung oleh tes).
pub async fn run_due_workers<C: Connector>(st: &St<C>) {
    let Ok(rows) = all_rows(st) else { return };
    let now = st.now();
    for row in rows {
        let Some((_, m)) = runnable(st, &row.id) else { continue };
        for w in &m.contributes.workers {
            let due = {
                let g = st.ext_workers.lock().expect("lock");
                g.get(&format!("{}/{}", row.id, w.id)).is_none_or(|s| !s.paused && s.next_run <= now)
            };
            if due {
                let _ = run_worker(st, &row.id, &w.id).await;
            }
        }
    }
}

/// Penjadwal latar: memeriksa worker jatuh tempo tiap 5 detik. Dipanggil sekali oleh aplikasi (desktop, server).
pub fn spawn_scheduler<C: Connector>(st: St<C>) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(std::time::Duration::from_secs(5));
        loop {
            tick.tick().await;
            run_due_workers(&st).await;
        }
    })
}

async fn run_worker_now<C: Connector>(State(st): State<St<C>>, c: Caller, Path((id, worker)): Path<(String, String)>) -> ApiResult<Json<Value>> {
    manage(&st, &c, Level::Update)?;
    if !valid_id(&id) {
        return Err(ApiError::not_found());
    }
    let row = all_rows(&st)?.into_iter().find(|r| r.id == id).ok_or_else(ApiError::not_found)?;
    let m = manifest_of(&row)?;
    if !m.contributes.workers.iter().any(|w| w.id == worker) {
        return Err(ApiError::not_found());
    }
    // Menjalankan manual membuka jeda.
    if let Some(s) = st.ext_workers.lock().expect("lock").get_mut(&format!("{id}/{worker}")) {
        s.paused = false;
        s.failures = 0;
    }
    let r = run_worker(&st, &id, &worker).await;
    Ok(Json(json!({ "ok": r.is_ok(), "error": r.err(), "workers": worker_view(&st, &m) })))
}
