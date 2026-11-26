//! Terminal SSH ke router untuk extension (ADR 0029).
//!
//! - **Pin kunci host** per router: `GET/PUT /api/routers/{id}/ssh` (izin Router Ubah). Pemasangan pin hanya bila
//!   sidik jari yang dikirim klien sama dengan yang router tawarkan saat itu (admin sudah memeriksanya).
//! - **Sesi**: `GET /api/ext-session/{ext}/{session}?router=&cols=&rows=` (WebSocket). Syarat, semuanya wajib:
//!   extension aktif dan official, sesi dideklarasikan manifest, toggle `feature.router-ssh` menyala, peran punya
//!   level Ubah dan aksi sesi pada extension, peran punya Router Ubah (jadi 2FA berlaku), bukan lewat akses Publik,
//!   router `can_write`, kunci host sudah disematkan. SSH memberi hak penuh akun router, jadi tidak ada mode baca-saja.
//! - Batas: 3 sesi per akun, 10 total, diam 10 menit, maksimum 4 jam. Pembukaan, penutupan, dan baris perintah yang
//!   diketik (usaha terbaik, bukan bukti) masuk riwayat audit.
//! - Frame klien→server: biner = masukan mentah; teks = JSON `{"type":"resize","cols","rows"}`. Server→klien: biner =
//!   keluaran; teks `{"type":"end","reason"}` sebelum ditutup.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{ConnectInfo, Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{Value, json};
use telemon_core::auth::{Level, Resource, Via};
use telemon_core::{AuditEntry, AuditOutcome, Change, Connector, ServiceError};
use telemon_ext::valid_id;
use telemon_ssh::{SshError, Target};

use crate::error::{ApiError, ApiResult};
use crate::extbackend::{granted_of, usable};
use crate::extensions::{effective, lvl};
use crate::guard::{Caller, Surface, origin_ok, resolve_caller};
use crate::state::AppState;

type St<C> = Arc<AppState<C>>;

const PER_USER: usize = 3;
const TOTAL: usize = 10;
const IDLE: Duration = Duration::from_secs(600);
const MAX_SESSION: Duration = Duration::from_secs(4 * 3600);
const RECHECK: Duration = Duration::from_secs(30);
const MAX_LINES: usize = 50;

pub fn routes<C: Connector>() -> Router<St<C>> {
    Router::new().route("/api/routers/{id}/ssh", get(pin_get::<C>).put(pin_put::<C>)).route("/api/ext-session/{ext}/{session}", get(session::<C>))
}

fn audit<C: Connector>(st: &St<C>, actor: &str, router: &str, action: &str, target: &str, title: String, changes: Vec<Change>) {
    let e = AuditEntry { at_ms: st.now(), actor: actor.into(), router_id: router.into(), action: action.into(), target: target.into(), title, changes, outcome: AuditOutcome::Ok };
    let _ = st.telemon.with_store(|s| s.record_audit(&e));
}

fn router_host<C: Connector>(st: &St<C>, id: &str) -> ApiResult<(String, u16, Option<String>, String)> {
    let rec = st.telemon.with_store(|s| s.get_router(id)).map_err(ApiError::internal)?.ok_or_else(ApiError::not_found)?;
    let (port, pin) = st.telemon.with_store(|s| s.router_ssh_get(id)).map_err(ApiError::internal)?;
    Ok((rec.host, port, pin, rec.name))
}

#[derive(Deserialize)]
struct PortQ {
    /// Port lain untuk diperiksa sebelum disematkan (bawaan: port tersimpan).
    port: Option<u16>,
}

async fn pin_get<C: Connector>(State(st): State<St<C>>, c: Caller, Path(id): Path<String>, Query(q): Query<PortQ>) -> ApiResult<Json<Value>> {
    st.auth.authorize(Some(&c.session), Resource::Router, Level::Update).map_err(ApiError::from)?;
    let (host, saved_port, pin, _) = router_host(&st, &id)?;
    let port = q.port.filter(|p| *p > 0).unwrap_or(saved_port);
    let presented = telemon_ssh::host_key_fingerprint(&host, port).await;
    Ok(Json(json!({ "port": port, "savedPort": saved_port, "pinned": pin, "presented": presented.as_ref().ok(), "error": presented.err().map(|e| e.to_string()) })))
}

#[derive(Deserialize)]
struct PinReq {
    port: Option<u16>,
    /// `null` menghapus pin; selain itu harus sama dengan sidik jari yang router tawarkan saat ini.
    fingerprint: Option<String>,
}

async fn pin_put<C: Connector>(State(st): State<St<C>>, c: Caller, Path(id): Path<String>, Json(req): Json<PinReq>) -> ApiResult<Json<Value>> {
    st.auth.authorize(Some(&c.session), Resource::Router, Level::Update).map_err(ApiError::from)?;
    let (host, cur_port, cur_pin, name) = router_host(&st, &id)?;
    let port = req.port.unwrap_or(cur_port);
    if port == 0 {
        return Err(ApiError::bad("Port SSH tidak valid"));
    }
    let now = st.now();
    let new_pin = match &req.fingerprint {
        None => None,
        Some(fp) => {
            let presented = telemon_ssh::host_key_fingerprint(&host, port).await.map_err(|e| ApiError::new(StatusCode::BAD_GATEWAY, "ssh_unreachable", e.to_string()))?;
            if !presented.eq_ignore_ascii_case(fp.trim()) {
                return Err(ApiError::new(StatusCode::CONFLICT, "ssh_host_key_changed", format!("Router menawarkan kunci {presented}, bukan {fp}; periksa ulang sebelum menyematkan")));
            }
            Some(presented)
        }
    };
    st.telemon.with_store(|s| s.router_ssh_set(&id, port, new_pin.as_deref(), now)).map_err(ApiError::internal)?;
    audit(&st, &c.session.username, &id, "router.ssh_pin", &id, format!("Sematkan kunci host SSH router {name}"), vec![Change { field: "kunci host".into(), from: cur_pin, to: new_pin.clone() }, Change { field: "port".into(), from: Some(cur_port.to_string()), to: Some(port.to_string()) }]);
    Ok(Json(json!({ "port": port, "pinned": new_pin })))
}

#[derive(Deserialize)]
struct SessQ {
    router: String,
    cols: Option<u32>,
    rows: Option<u32>,
}

/// Menghitung sesi terbuka; berkurang otomatis saat sesi berakhir (termasuk bila tugas dibatalkan).
struct Slot<C: Connector> {
    st: St<C>,
    user: String,
}

impl<C: Connector> Drop for Slot<C> {
    fn drop(&mut self) {
        let mut g = self.st.ssh_active.lock().expect("lock");
        if let Some(n) = g.0.get_mut(&self.user) {
            *n = n.saturating_sub(1);
            if *n == 0 {
                g.0.remove(&self.user);
            }
        }
        g.1 = g.1.saturating_sub(1);
    }
}

fn take_slot<C: Connector>(st: &St<C>, user: &str) -> Option<Slot<C>> {
    let mut g = st.ssh_active.lock().expect("lock");
    if g.1 >= TOTAL || g.0.get(user).copied().unwrap_or(0) >= PER_USER {
        return None;
    }
    *g.0.entry(user.to_string()).or_default() += 1;
    g.1 += 1;
    Some(Slot { st: st.clone(), user: user.to_string() })
}

#[allow(clippy::too_many_arguments)]
async fn session<C: Connector>(
    State(st): State<St<C>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    axum::Extension(surface): axum::Extension<Surface>,
    headers: HeaderMap,
    Path((ext, sid)): Path<(String, String)>,
    Query(q): Query<SessQ>,
    ws: WebSocketUpgrade,
) -> Result<Response, ApiError> {
    if !origin_ok(&headers) {
        return Err(ApiError::forbidden("Asal permintaan tidak diizinkan"));
    }
    let caller = resolve_caller(&st, &surface, &headers, peer).await?;
    if st.cfg.extensions.is_none() || !valid_id(&ext) {
        return Err(ApiError::not_found());
    }
    let (_, m) = usable(&st, &ext, false).ok_or_else(ApiError::not_found)?;
    let decl = m.contributes.sessions.iter().find(|s| s.id == sid).ok_or_else(ApiError::not_found)?.clone();
    if !granted_of(&st, &ext).iter().any(|g| g == "feature.router-ssh") {
        return Err(ApiError::forbidden("Izin feature.router-ssh belum diberikan ke extension ini"));
    }
    let (level, actions) = effective(&st, &caller, &ext, &m);
    if level < lvl("update") || !actions.contains(&decl.action) {
        return Err(ApiError::forbidden("Peran Anda tidak boleh membuka terminal ini"));
    }
    if caller.session.via == Via::Publik {
        return Err(ApiError::forbidden("Terminal SSH tidak tersedia lewat akses Publik"));
    }
    st.auth.authorize(Some(&caller.session), Resource::Router, Level::Update).map_err(ApiError::from)?;
    let creds = st.telemon.ssh_credentials(&q.router).map_err(|e| match e {
        ServiceError::NotFound => ApiError::not_found(),
        ServiceError::Rejected(m) => ApiError::forbidden(m),
        other => ApiError::internal(other),
    })?;
    let Some(pin) = creds.host_key.clone() else {
        return Err(ApiError::new(StatusCode::CONFLICT, "ssh_pin_required", "Kunci host SSH router belum disematkan; periksa dan sematkan dulu"));
    };
    let user = caller.session.username.clone();
    let slot = take_slot(&st, &user).ok_or_else(|| ApiError::new(StatusCode::TOO_MANY_REQUESTS, "too_many_sessions", "Terlalu banyak sesi terminal terbuka"))?;
    let (cols, rows) = (q.cols.unwrap_or(100).clamp(10, 500), q.rows.unwrap_or(30).clamp(3, 200));
    let target = Target { host: creds.host.clone(), port: creds.port, username: creds.username.clone(), password: creds.password.clone() };
    let session = telemon_ssh::open(&target, &pin, cols, rows).await.map_err(|e| match e {
        SshError::HostKeyMismatch { .. } => ApiError::new(StatusCode::CONFLICT, "ssh_host_key_changed", e.to_string()),
        SshError::Auth => ApiError::new(StatusCode::BAD_GATEWAY, "ssh_auth", e.to_string()),
        other => ApiError::new(StatusCode::BAD_GATEWAY, "ssh_unreachable", other.to_string()),
    })?;
    let name = st.telemon.with_store(|s| s.get_router(&q.router)).ok().flatten().map(|r| r.name).unwrap_or_default();
    audit(&st, &user, &q.router, "extension.ssh_open", &ext, format!("Buka terminal SSH ke router {name}"), vec![]);
    let router = q.router.clone();
    Ok(ws.on_upgrade(move |socket| bridge(st, socket, session, slot, surface, headers, peer, user, ext, router, name)))
}

/// Teks yang diketik pengguna per baris, tanpa urutan escape dan karakter kontrol (usaha terbaik untuk audit).
#[derive(Default)]
struct LineCapture {
    cur: Vec<char>,
    lines: Vec<String>,
    esc: u8,
}

impl LineCapture {
    fn feed(&mut self, data: &[u8]) {
        for &b in data {
            if self.esc > 0 {
                // ESC [ ... huruf akhir, atau ESC + satu karakter.
                if self.esc == 1 {
                    self.esc = if b == b'[' || b == b'O' { 2 } else { 0 };
                } else if (0x40..=0x7e).contains(&b) {
                    self.esc = 0;
                }
                continue;
            }
            match b {
                0x1b => self.esc = 1,
                b'\r' | b'\n' => self.flush(),
                0x7f | 0x08 => {
                    self.cur.pop();
                }
                0x03 | 0x15 => self.cur.clear(),
                0x20..=0x7e => {
                    if self.cur.len() < 200 {
                        self.cur.push(b as char)
                    }
                }
                _ => {}
            }
        }
    }

    fn flush(&mut self) {
        let line: String = self.cur.drain(..).collect();
        if !line.trim().is_empty() && self.lines.len() < MAX_LINES {
            self.lines.push(line);
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn bridge<C: Connector>(st: St<C>, mut socket: WebSocket, mut s: telemon_ssh::Session, _slot: Slot<C>, surface: Surface, headers: HeaderMap, peer: SocketAddr, user: String, ext: String, router: String, name: String) {
    let started = Instant::now();
    let mut last_input = Instant::now();
    let (mut bytes_in, mut bytes_out) = (0u64, 0u64);
    let mut cap = LineCapture::default();
    let mut tick = tokio::time::interval(RECHECK);
    tick.tick().await;
    let mut reason = "ditutup".to_string();
    loop {
        tokio::select! {
            out = s.reader.read() => match out {
                Some(b) => {
                    bytes_out += b.len() as u64;
                    if socket.send(Message::Binary(b.into())).await.is_err() {
                        reason = "klien terputus".into();
                        break;
                    }
                }
                None => {
                    reason = "router menutup sesi".into();
                    break;
                }
            },
            msg = socket.recv() => match msg {
                Some(Ok(Message::Binary(b))) => {
                    last_input = Instant::now();
                    bytes_in += b.len() as u64;
                    cap.feed(&b);
                    if s.writer.write(&b).await.is_err() {
                        reason = "gagal menulis ke router".into();
                        break;
                    }
                }
                Some(Ok(Message::Text(t))) => {
                    if let Ok(v) = serde_json::from_str::<Value>(&t)
                        && v.get("type").and_then(Value::as_str) == Some("resize")
                    {
                        let cols = v.get("cols").and_then(Value::as_u64).unwrap_or(100).clamp(10, 500) as u32;
                        let rows = v.get("rows").and_then(Value::as_u64).unwrap_or(30).clamp(3, 200) as u32;
                        let _ = s.writer.resize(cols, rows).await;
                    }
                }
                Some(Ok(Message::Close(_))) | None | Some(Err(_)) => {
                    reason = "klien menutup".into();
                    break;
                }
                Some(Ok(_)) => {}
            },
            _ = tick.tick() => {
                let end = if last_input.elapsed() > IDLE {
                    Some("diam terlalu lama")
                } else if started.elapsed() > MAX_SESSION {
                    Some("melewati batas waktu sesi")
                } else if !resolve_caller(&st, &surface, &headers, peer).await.is_ok_and(|c| c.session.permissions.get(Resource::Router) >= Level::Update && c.session.via != Via::Publik) {
                    Some("izin dicabut atau sesi login berakhir")
                } else {
                    None
                };
                if let Some(r) = end {
                    reason = r.to_string();
                    let _ = socket.send(Message::Text(json!({ "type": "end", "reason": r }).to_string().into())).await;
                    break;
                }
            }
        }
    }
    s.writer.close().await;
    let _ = socket.send(Message::Close(None)).await;
    cap.flush();
    let mut changes = vec![
        Change { field: "durasi (detik)".into(), from: None, to: Some(started.elapsed().as_secs().to_string()) },
        Change { field: "byte masuk/keluar".into(), from: None, to: Some(format!("{bytes_in}/{bytes_out}")) },
        Change { field: "alasan akhir".into(), from: None, to: Some(reason) },
    ];
    changes.extend(cap.lines.into_iter().map(|l| Change { field: "perintah".into(), from: None, to: Some(l) }));
    audit(&st, &user, &router, "extension.ssh", &ext, format!("Tutup terminal SSH ke router {name}"), changes);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_capture_keeps_typed_lines_without_control_sequences() {
        let mut c = LineCapture::default();
        c.feed(b"/ip address print\r");
        c.feed(b"/sys\x1b[Ao\x7f\x7ftem reboot");
        c.feed(b"\x03");
        c.feed(b"  \r");
        c.feed(b"/ping 8.8.8.8 count=2\n");
        c.feed(&[0x00, 0x01, b'x', b'\r']);
        c.flush();
        assert_eq!(c.lines, vec!["/ip address print".to_string(), "/ping 8.8.8.8 count=2".to_string(), "x".to_string()]);
        let mut many = LineCapture::default();
        for i in 0..80 {
            many.feed(format!("cmd{i}\r").as_bytes());
        }
        assert_eq!(many.lines.len(), MAX_LINES, "batas baris per sesi");
        let mut long = LineCapture::default();
        long.feed(&vec![b'a'; 500]);
        long.feed(b"\r");
        assert_eq!(long.lines[0].len(), 200);
    }
}
