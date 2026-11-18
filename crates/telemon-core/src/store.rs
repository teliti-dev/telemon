//! Penyimpanan SQLite: event, salinan user/profile dari router (Mikrotik = sumber kebenaran,
//! ADR 0004), catatan lokal, dan retensi (ADR 0006). Password belum disimpan di sini; itu
//! butuh enkripsi dengan kunci keyring (ADR 0009) dan dikerjakan terpisah.

use std::path::Path;

use rusqlite::types::Value;
use rusqlite::{Connection, OptionalExtension, params, params_from_iter};

use crate::events::{Event, EventKind};
use crate::model::{PppActive, PppProfile, PppSecret};
use crate::poller::{UserStatus, user_status};
use crate::vault::{Vault, VaultError};
use crate::write::Change;

pub type Result<T> = rusqlite::Result<T>;

const DAY_MS: i64 = 86_400_000;

/// Migrasi berurutan; `PRAGMA user_version` menyimpan jumlah yang sudah dijalankan.
const MIGRATIONS: &[&str] = &["
CREATE TABLE events (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  at_ms INTEGER NOT NULL,
  router_id TEXT NOT NULL,
  kind TEXT NOT NULL,
  username TEXT,
  address TEXT,
  caller_id TEXT,
  message TEXT NOT NULL
);
CREATE INDEX events_router_at ON events(router_id, at_ms);
CREATE INDEX events_at ON events(at_ms);
CREATE INDEX events_user ON events(router_id, username);

CREATE TABLE users (
  id TEXT PRIMARY KEY,
  router_id TEXT NOT NULL,
  username TEXT NOT NULL,
  secret_id TEXT NOT NULL DEFAULT '',
  profile TEXT NOT NULL DEFAULT '',
  status TEXT NOT NULL,
  address TEXT,
  caller_id TEXT,
  connected_at INTEGER,
  last_seen_at INTEGER,
  last_event TEXT,
  last_event_at INTEGER,
  comment TEXT,
  static_address TEXT,
  deleted_at INTEGER,
  deleted_reason TEXT
);
CREATE INDEX users_router ON users(router_id);

CREATE TABLE profiles (
  id TEXT PRIMARY KEY,
  router_id TEXT NOT NULL,
  name TEXT NOT NULL,
  rate_limit TEXT,
  local_address TEXT,
  remote_address TEXT,
  only_one TEXT,
  comment TEXT,
  deleted_at INTEGER,
  deleted_reason TEXT
);
CREATE INDEX profiles_router ON profiles(router_id);

CREATE TABLE user_meta (
  user_id TEXT PRIMARY KEY,
  customer_name TEXT,
  phone TEXT,
  note TEXT,
  tags TEXT NOT NULL DEFAULT '',
  updated_at INTEGER NOT NULL
);
", "
CREATE TABLE audit (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  at_ms INTEGER NOT NULL,
  actor TEXT NOT NULL,
  router_id TEXT NOT NULL,
  action TEXT NOT NULL,
  target TEXT NOT NULL,
  title TEXT NOT NULL,
  changes TEXT NOT NULL DEFAULT '',
  ok INTEGER NOT NULL,
  steps_done INTEGER NOT NULL DEFAULT 0,
  error TEXT
);
CREATE INDEX audit_at ON audit(at_ms);
CREATE INDEX audit_router ON audit(router_id, at_ms);
", "
CREATE TABLE user_secrets (
  user_id TEXT PRIMARY KEY,
  enc BLOB NOT NULL,
  updated_at INTEGER NOT NULL
);
CREATE TABLE vault_meta (
  k TEXT PRIMARY KEY,
  v BLOB NOT NULL
);
", "
CREATE TABLE routers (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL UNIQUE COLLATE NOCASE,
  host TEXT NOT NULL,
  port INTEGER NOT NULL,
  tls_pin TEXT,
  username TEXT NOT NULL,
  can_write INTEGER NOT NULL DEFAULT 0,
  model TEXT NOT NULL DEFAULT '',
  version TEXT NOT NULL DEFAULT '',
  status TEXT NOT NULL DEFAULT 'offline',
  error TEXT,
  last_poll_at INTEGER,
  last_import_at INTEGER,
  created_at INTEGER NOT NULL
);
"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PurgeCounts {
    pub events: u64,
    pub users: u64,
    pub profiles: u64,
    pub audit: u64,
}

/// Nilai `None` = tanpa batas waktu (selamanya / tidak pernah).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetentionPolicy {
    pub event_days: Option<u32>,
    pub max_events: u32,
    pub deleted_days: Option<u32>,
    /// Riwayat perubahan disimpan N hari.
    pub audit_days: Option<u32>,
}

#[derive(Debug, Clone, Default)]
pub struct EventQuery {
    pub router_id: Option<String>,
    /// Kosong = semua jenis.
    pub kinds: Vec<EventKind>,
    /// Pencarian bagian username (tanpa membedakan huruf besar kecil).
    pub username: Option<String>,
    pub since_ms: Option<i64>,
    pub until_ms: Option<i64>,
    /// Kursor untuk scroll tanpa henti: hanya id yang lebih kecil.
    pub before_id: Option<i64>,
    /// Dibatasi 1..=500; 0 dianggap 50.
    pub limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredEvent {
    pub id: i64,
    pub router_id: String,
    pub event: Event,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserRow {
    pub id: String,
    pub router_id: String,
    pub username: String,
    pub profile: String,
    pub status: UserStatus,
    pub address: Option<String>,
    pub caller_id: Option<String>,
    pub connected_at: Option<i64>,
    pub last_seen_at: Option<i64>,
    pub last_event: Option<EventKind>,
    pub last_event_at: Option<i64>,
    pub comment: Option<String>,
    pub static_address: Option<String>,
    pub deleted_at: Option<i64>,
    pub deleted_reason: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SyncReport {
    pub added: u32,
    pub updated: u32,
    /// Hilang di router, ditandai soft delete.
    pub soft_deleted: u32,
    /// Muncul lagi di router setelah ditandai hilang.
    pub restored: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UserMeta {
    pub customer_name: Option<String>,
    pub phone: Option<String>,
    pub note: Option<String>,
    pub tags: Vec<String>,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuditOutcome {
    Ok,
    /// Gagal (atau ditolak) setelah `steps_done` langkah selesai di router.
    Failed { steps_done: usize, error: String },
}

/// Satu baris riwayat perubahan ke router. Tidak pernah memuat password.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditEntry {
    pub at_ms: i64,
    /// Akun yang menjalankan (`desktop` untuk aplikasi desktop tanpa login).
    pub actor: String,
    pub router_id: String,
    pub action: String,
    pub target: String,
    pub title: String,
    pub changes: Vec<Change>,
    pub outcome: AuditOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredAudit {
    pub id: i64,
    pub entry: AuditEntry,
}

#[derive(Debug, Clone, Default)]
pub struct AuditQuery {
    pub router_id: Option<String>,
    pub actor: Option<String>,
    /// Pencarian bagian nama target (username/profile).
    pub target: Option<String>,
    pub only_failed: bool,
    pub since_ms: Option<i64>,
    pub until_ms: Option<i64>,
    pub before_id: Option<i64>,
    /// Dibatasi 1..=500; 0 dianggap 50.
    pub limit: usize,
}

const FS: char = '\u{1f}';
const RS: char = '\u{1e}';

fn clean(s: &str) -> String {
    s.replace([FS, RS], " ")
}

/// Perubahan disimpan sebagai teks: baris dipisah RS, kolom FS; `-` = kosong, `+teks` = berisi.
fn encode_changes(cs: &[Change]) -> String {
    let opt = |v: &Option<String>| v.as_deref().map_or("-".to_string(), |t| format!("+{}", clean(t)));
    cs.iter().map(|c| format!("{}{FS}{}{FS}{}", clean(&c.field), opt(&c.from), opt(&c.to))).collect::<Vec<_>>().join(&RS.to_string())
}

fn decode_changes(s: &str) -> Vec<Change> {
    let opt = |v: &str| v.strip_prefix('+').map(String::from);
    s.split(RS)
        .filter(|l| !l.is_empty())
        .filter_map(|l| {
            let mut p = l.split(FS);
            Some(Change { field: p.next()?.to_string(), from: opt(p.next()?), to: opt(p.next()?) })
        })
        .collect()
}

// ---------------- router ----------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouterStatus {
    Online,
    Offline,
    Error,
}

impl RouterStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            RouterStatus::Online => "online",
            RouterStatus::Offline => "offline",
            RouterStatus::Error => "error",
        }
    }
    fn parse(s: &str) -> Self {
        match s {
            "online" => RouterStatus::Online,
            "error" => RouterStatus::Error,
            _ => RouterStatus::Offline,
        }
    }
}

/// Konfigurasi router. Kata sandi API **tidak** ada di sini: disimpan di keyring OS oleh aplikasi
/// (kunci `router:<id>`). `tls_pin` ada = api-ssl dengan sertifikat di-pin; kosong = API biasa.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouterRecord {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    /// Sidik jari SHA-256 `AA:BB:...` (huruf besar).
    pub tls_pin: Option<String>,
    pub username: String,
    /// Akun API punya hak write dan pemilik mengizinkan edit dari Telemon.
    pub can_write: bool,
    pub model: String,
    pub version: String,
    pub status: RouterStatus,
    pub error: Option<String>,
    pub last_poll_at: Option<i64>,
    pub last_import_at: Option<i64>,
    pub created_at: i64,
}

impl RouterRecord {
    pub fn addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }

    /// Mode sambungan untuk `MikrotikClient::connect_with`.
    #[cfg(feature = "tls")]
    pub fn tls_mode(&self) -> crate::router::TlsMode {
        match self.tls_pin.as_deref().and_then(crate::tls::Fingerprint::parse) {
            Some(fp) => crate::router::TlsMode::Pinned(fp),
            None => crate::router::TlsMode::Plain,
        }
    }

    #[cfg(not(feature = "tls"))]
    pub fn tls_mode(&self) -> crate::router::TlsMode {
        crate::router::TlsMode::Plain
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewRouter {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub tls_pin: Option<String>,
    pub username: String,
    pub can_write: bool,
}

/// `Some(None)` pada `tls_pin` = kembali ke API biasa; `None` = tidak diubah.
#[derive(Debug, Clone, Default)]
pub struct RouterPatch {
    pub name: Option<String>,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub tls_pin: Option<Option<String>>,
    pub username: Option<String>,
    pub can_write: Option<bool>,
}

#[derive(Debug)]
pub enum RouterError {
    /// Validasi gagal; pesan siap tampil.
    Invalid(String),
    NotFound,
    Db(rusqlite::Error),
}

impl std::fmt::Display for RouterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RouterError::Invalid(m) => f.write_str(m),
            RouterError::NotFound => f.write_str("router tidak ditemukan"),
            RouterError::Db(e) => write!(f, "basis data: {e}"),
        }
    }
}

impl std::error::Error for RouterError {}

impl From<rusqlite::Error> for RouterError {
    fn from(e: rusqlite::Error) -> Self {
        RouterError::Db(e)
    }
}

/// Data yang ikut terhapus bersama router. Riwayat perubahan sengaja dipertahankan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RouterRemoved {
    pub events: u64,
    pub users: u64,
    pub profiles: u64,
}

/// `AA:BB:..` huruf besar dari 64 digit heksa (dengan atau tanpa pemisah); None bila tidak valid.
pub fn normalize_fingerprint(s: &str) -> Option<String> {
    let hex: String = s.chars().filter(|c| !matches!(c, ':' | ' ' | '-')).collect();
    if hex.len() != 64 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let up = hex.to_ascii_uppercase();
    Some(up.as_bytes().chunks(2).map(|c| std::str::from_utf8(c).unwrap()).collect::<Vec<_>>().join(":"))
}

fn check_router_fields(name: &str, host: &str, port: u16, username: &str) -> std::result::Result<(), RouterError> {
    let bad = |m: &str| Err(RouterError::Invalid(m.into()));
    if name.trim().is_empty() || name.trim().chars().count() > 40 {
        return bad("nama router 1-40 karakter");
    }
    if host.trim().is_empty() || host.trim().chars().any(char::is_whitespace) || host.contains('/') {
        return bad("host tidak valid");
    }
    if port == 0 {
        return bad("port harus 1-65535");
    }
    if username.trim().is_empty() {
        return bad("username API wajib diisi");
    }
    Ok(())
}

fn router_from_row(r: &rusqlite::Row<'_>) -> Result<RouterRecord> {
    Ok(RouterRecord {
        id: r.get(0)?,
        name: r.get(1)?,
        host: r.get(2)?,
        port: r.get::<_, i64>(3)?.clamp(1, 65535) as u16,
        tls_pin: r.get(4)?,
        username: r.get(5)?,
        can_write: r.get::<_, i64>(6)? != 0,
        model: r.get(7)?,
        version: r.get(8)?,
        status: RouterStatus::parse(&r.get::<_, String>(9)?),
        error: r.get(10)?,
        last_poll_at: r.get(11)?,
        last_import_at: r.get(12)?,
        created_at: r.get(13)?,
    })
}

const ROUTER_COLS: &str = "id, name, host, port, tls_pin, username, can_write, model, version, status, error, last_poll_at, last_import_at, created_at";

fn status_str(s: UserStatus) -> &'static str {
    match s {
        UserStatus::Active => "active",
        UserStatus::Inactive => "inactive",
        UserStatus::Disabled => "disabled",
    }
}

fn status_parse(s: &str) -> UserStatus {
    match s {
        "active" => UserStatus::Active,
        "disabled" => UserStatus::Disabled,
        _ => UserStatus::Inactive,
    }
}

pub struct Store {
    conn: Connection,
    /// Ada setelah [`Store::unlock`]; tanpa itu password tidak disimpan maupun dibaca.
    vault: Option<Vault>,
}

#[derive(Debug)]
pub enum PasswordError {
    /// Belum `unlock`.
    Locked,
    /// Kunci tidak cocok dengan basis data ini.
    WrongKey,
    Vault(VaultError),
    Db(rusqlite::Error),
}

impl std::fmt::Display for PasswordError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PasswordError::Locked => f.write_str("penyimpanan password terkunci"),
            PasswordError::WrongKey => f.write_str("kunci enkripsi tidak cocok dengan basis data"),
            PasswordError::Vault(e) => write!(f, "{e}"),
            PasswordError::Db(e) => write!(f, "basis data: {e}"),
        }
    }
}

impl std::error::Error for PasswordError {}

impl From<rusqlite::Error> for PasswordError {
    fn from(e: rusqlite::Error) -> Self {
        PasswordError::Db(e)
    }
}

impl From<VaultError> for PasswordError {
    fn from(e: VaultError) -> Self {
        PasswordError::Vault(e)
    }
}

const CANARY_PLAIN: &[u8] = b"telemon-vault-v1";
const CANARY_AAD: &[u8] = b"canary";

impl Store {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let conn = Connection::open(path)?;
        // WAL: pembacaan UI tidak menunggu penulisan poller.
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        Self::init(conn)
    }

    pub fn open_in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self> {
        conn.pragma_update(None, "foreign_keys", "ON")?;
        let done = conn.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))? as usize;
        for (i, sql) in MIGRATIONS.iter().enumerate().skip(done) {
            let tx = conn.unchecked_transaction()?;
            tx.execute_batch(sql)?;
            tx.pragma_update(None, "user_version", (i + 1) as i64)?;
            tx.commit()?;
        }
        Ok(Self { conn, vault: None })
    }

    // ---------------- event ----------------

    /// Simpan event satu router dalam satu transaksi dan perbarui "event terakhir" user.
    pub fn insert_events(&mut self, router_id: &str, events: &[Event]) -> Result<usize> {
        let tx = self.conn.transaction()?;
        for e in events {
            tx.execute(
                "INSERT INTO events (at_ms, router_id, kind, username, address, caller_id, message) VALUES (?1,?2,?3,?4,?5,?6,?7)",
                params![e.at_ms, router_id, e.kind.as_str(), e.username, e.address, e.caller_id, e.message],
            )?;
            if let Some(u) = &e.username {
                let id = user_id(router_id, u);
                // Hanya event yang tidak lebih lama dari event terakhir user yang mengubah keadaannya.
                let fresh = "(last_event_at IS NULL OR last_event_at <= ?2)";
                tx.execute(&format!("UPDATE users SET last_event = ?1, last_event_at = ?2 WHERE id = ?3 AND {fresh}"), params![e.kind.as_str(), e.at_ms, id])?;
                match e.kind {
                    EventKind::Connected => {
                        tx.execute(
                            &format!("UPDATE users SET status = CASE WHEN status = 'disabled' THEN status ELSE 'active' END, connected_at = ?2, last_seen_at = ?2,
                                address = COALESCE(?4, address), caller_id = COALESCE(?5, caller_id) WHERE id = ?3 AND {fresh} AND status != 'active'"),
                            params![e.kind.as_str(), e.at_ms, id, e.address, e.caller_id],
                        )?;
                    }
                    EventKind::Disconnected => {
                        tx.execute(
                            &format!("UPDATE users SET status = CASE WHEN status = 'disabled' THEN status ELSE 'inactive' END, connected_at = NULL, last_seen_at = ?2 WHERE id = ?3 AND {fresh}"),
                            params![e.kind.as_str(), e.at_ms, id],
                        )?;
                    }
                    _ => {}
                }
            }
        }
        tx.commit()?;
        Ok(events.len())
    }

    /// Terbaru dulu. Pakai `before_id` dari baris terakhir untuk halaman berikutnya.
    pub fn list_events(&self, q: &EventQuery) -> Result<Vec<StoredEvent>> {
        let mut sql = String::from("SELECT id, at_ms, router_id, kind, username, address, caller_id, message FROM events WHERE 1=1");
        let mut args: Vec<Value> = Vec::new();
        if let Some(r) = &q.router_id {
            sql.push_str(" AND router_id = ?");
            args.push(r.clone().into());
        }
        if !q.kinds.is_empty() {
            sql.push_str(&format!(" AND kind IN ({})", vec!["?"; q.kinds.len()].join(",")));
            args.extend(q.kinds.iter().map(|k| Value::from(k.as_str().to_string())));
        }
        if let Some(u) = q.username.as_deref().filter(|u| !u.is_empty()) {
            sql.push_str(" AND username LIKE ? ESCAPE '\\'");
            let esc = u.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_");
            args.push(format!("%{esc}%").into());
        }
        if let Some(t) = q.since_ms {
            sql.push_str(" AND at_ms >= ?");
            args.push(t.into());
        }
        if let Some(t) = q.until_ms {
            sql.push_str(" AND at_ms <= ?");
            args.push(t.into());
        }
        if let Some(id) = q.before_id {
            sql.push_str(" AND id < ?");
            args.push(id.into());
        }
        sql.push_str(" ORDER BY id DESC LIMIT ?");
        let limit = if q.limit == 0 { 50 } else { q.limit.min(500) };
        args.push((limit as i64).into());

        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params_from_iter(args), |r| {
            let kind: String = r.get(3)?;
            // Jenis tak dikenal (dari versi yang lebih baru) dilewati, bukan dipalsukan.
            let Some(kind) = EventKind::parse(&kind) else { return Ok(None) };
            Ok(Some(StoredEvent {
                id: r.get(0)?,
                router_id: r.get(2)?,
                event: Event { at_ms: r.get(1)?, kind, username: r.get(4)?, address: r.get(5)?, caller_id: r.get(6)?, message: r.get(7)? },
            }))
        })?;
        rows.filter_map(|r| r.transpose()).collect()
    }

    pub fn count_events(&self) -> Result<u64> {
        self.conn.query_row("SELECT COUNT(*) FROM events", [], |r| r.get::<_, i64>(0)).map(|n| n as u64)
    }

    // ---------------- sinkronisasi dari router ----------------

    /// Import: salinan Telemon ditimpa data router. Yang hilang di router ditandai soft delete
    /// (alasan "router"); yang muncul lagi dipulihkan. User yang dihapus manual di Telemon
    /// (alasan "manual") tidak dipulihkan walau secretnya masih ada.
    pub fn sync_users(&mut self, router_id: &str, now_ms: i64, secrets: &[PppSecret], active: &[PppActive]) -> Result<SyncReport> {
        let tx = self.conn.transaction()?;
        let mut rep = SyncReport::default();
        let mut seen = Vec::with_capacity(secrets.len());
        for s in secrets {
            let id = user_id(router_id, &s.name);
            seen.push(id.clone());
            if let (Some(pw), Some(v)) = (&s.password, &self.vault) {
                // Router adalah sumber kebenaran: password yang dikirim router menimpa salinan lokal.
                // Bila router tidak mengirim password (hak akun API kurang), salinan lama dibiarkan.
                if let Ok(enc) = v.seal(id.as_bytes(), pw.as_bytes()) {
                    tx.execute(
                        "INSERT INTO user_secrets (user_id, enc, updated_at) VALUES (?1,?2,?3) ON CONFLICT(user_id) DO UPDATE SET enc=?2, updated_at=?3",
                        params![id, enc, now_ms],
                    )?;
                }
            }
            let status = status_str(user_status(s, active));
            let act = active.iter().find(|a| a.name == s.name);
            let connected_at = act.and_then(|a| a.uptime_secs).map(|u| now_ms - (u as i64) * 1000);
            let existing: Option<(Option<i64>, Option<String>)> = tx
                .query_row("SELECT deleted_at, deleted_reason FROM users WHERE id = ?1", [&id], |r| Ok((r.get(0)?, r.get(1)?)))
                .optional()?;
            match existing {
                None => {
                    tx.execute(
                        "INSERT INTO users (id, router_id, username, secret_id, profile, status, address, caller_id, connected_at, last_seen_at, comment, static_address)
                         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
                        params![id, router_id, s.name, s.id, s.profile, status, act.and_then(|a| a.address.clone()), act.and_then(|a| a.caller_id.clone()), connected_at, act.map(|_| now_ms), s.comment, s.remote_address],
                    )?;
                    rep.added += 1;
                }
                Some((deleted_at, reason)) => {
                    let restore = deleted_at.is_some() && reason.as_deref() == Some("router");
                    tx.execute(
                        "UPDATE users SET secret_id=?2, profile=?3, status=?4, address=?5, caller_id=?6, connected_at=?7,
                           last_seen_at = COALESCE(?8, last_seen_at), comment=?9, static_address=?10,
                           deleted_at = CASE WHEN ?11 THEN NULL ELSE deleted_at END,
                           deleted_reason = CASE WHEN ?11 THEN NULL ELSE deleted_reason END
                         WHERE id=?1",
                        params![id, s.id, s.profile, status, act.and_then(|a| a.address.clone()), act.and_then(|a| a.caller_id.clone()), connected_at, act.map(|_| now_ms), s.comment, s.remote_address, restore],
                    )?;
                    if restore { rep.restored += 1 } else { rep.updated += 1 }
                }
            }
        }
        rep.soft_deleted += soft_delete_missing(&tx, "users", "id", router_id, &seen, now_ms)?;
        tx.commit()?;
        Ok(rep)
    }

    pub fn sync_profiles(&mut self, router_id: &str, now_ms: i64, profiles: &[PppProfile]) -> Result<SyncReport> {
        let tx = self.conn.transaction()?;
        let mut rep = SyncReport::default();
        let mut seen = Vec::with_capacity(profiles.len());
        for p in profiles {
            let id = format!("{router_id}:{}", p.name);
            seen.push(id.clone());
            let existing: Option<(Option<i64>, Option<String>)> = tx
                .query_row("SELECT deleted_at, deleted_reason FROM profiles WHERE id = ?1", [&id], |r| Ok((r.get(0)?, r.get(1)?)))
                .optional()?;
            match existing {
                None => {
                    tx.execute(
                        "INSERT INTO profiles (id, router_id, name, rate_limit, local_address, remote_address, only_one, comment) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
                        params![id, router_id, p.name, p.rate_limit, p.local_address, p.remote_address, p.only_one, p.comment],
                    )?;
                    rep.added += 1;
                }
                Some((deleted_at, reason)) => {
                    let restore = deleted_at.is_some() && reason.as_deref() == Some("router");
                    tx.execute(
                        "UPDATE profiles SET rate_limit=?2, local_address=?3, remote_address=?4, only_one=?5, comment=?6,
                           deleted_at = CASE WHEN ?7 THEN NULL ELSE deleted_at END,
                           deleted_reason = CASE WHEN ?7 THEN NULL ELSE deleted_reason END
                         WHERE id=?1",
                        params![id, p.rate_limit, p.local_address, p.remote_address, p.only_one, p.comment, restore],
                    )?;
                    if restore { rep.restored += 1 } else { rep.updated += 1 }
                }
            }
        }
        rep.soft_deleted += soft_delete_missing(&tx, "profiles", "id", router_id, &seen, now_ms)?;
        tx.commit()?;
        Ok(rep)
    }

    /// Soft delete manual dari Telemon (secret sudah dinonaktifkan di router oleh pemanggil).
    pub fn soft_delete_user(&mut self, id: &str, now_ms: i64) -> Result<bool> {
        let n = self.conn.execute(
            "UPDATE users SET deleted_at = ?2, deleted_reason = 'manual' WHERE id = ?1 AND deleted_at IS NULL",
            params![id, now_ms],
        )?;
        Ok(n > 0)
    }

    pub fn restore_user(&mut self, id: &str) -> Result<bool> {
        let n = self.conn.execute("UPDATE users SET deleted_at = NULL, deleted_reason = NULL WHERE id = ?1 AND deleted_at IS NOT NULL", [id])?;
        Ok(n > 0)
    }

    pub fn list_users(&self, router_id: Option<&str>, include_deleted: bool) -> Result<Vec<UserRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, router_id, username, profile, status, address, caller_id, connected_at, last_seen_at, last_event, last_event_at,
                    comment, static_address, deleted_at, deleted_reason
             FROM users WHERE (?1 IS NULL OR router_id = ?1) AND (?2 OR deleted_at IS NULL) ORDER BY username COLLATE NOCASE",
        )?;
        let rows = stmt.query_map(params![router_id, include_deleted], |r| {
            let st: String = r.get(4)?;
            let le: Option<String> = r.get(9)?;
            Ok(UserRow {
                id: r.get(0)?,
                router_id: r.get(1)?,
                username: r.get(2)?,
                profile: r.get(3)?,
                status: status_parse(&st),
                address: r.get(5)?,
                caller_id: r.get(6)?,
                connected_at: r.get(7)?,
                last_seen_at: r.get(8)?,
                last_event: le.as_deref().and_then(EventKind::parse),
                last_event_at: r.get(10)?,
                comment: r.get(11)?,
                static_address: r.get(12)?,
                deleted_at: r.get(13)?,
                deleted_reason: r.get(14)?,
            })
        })?;
        rows.collect()
    }

    // ---------------- router ----------------

    pub fn add_router(&mut self, n: &NewRouter, now_ms: i64) -> std::result::Result<RouterRecord, RouterError> {
        check_router_fields(&n.name, &n.host, n.port, &n.username)?;
        let pin = match &n.tls_pin {
            Some(p) => Some(normalize_fingerprint(p).ok_or_else(|| RouterError::Invalid("sidik jari sertifikat tidak valid".into()))?),
            None => None,
        };
        let id = format!("r-{}", crate::vault::generate_key().map_err(|_| RouterError::Invalid("gagal membuat id".into()))?.iter().take(6).map(|b| format!("{b:02x}")).collect::<String>());
        match self.conn.execute(
            "INSERT INTO routers (id, name, host, port, tls_pin, username, can_write, created_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
            params![id, n.name.trim(), n.host.trim(), i64::from(n.port), pin, n.username.trim(), n.can_write, now_ms],
        ) {
            Ok(_) => {}
            Err(rusqlite::Error::SqliteFailure(e, _)) if e.code == rusqlite::ErrorCode::ConstraintViolation => {
                return Err(RouterError::Invalid(format!("nama router {} sudah dipakai", n.name.trim())));
            }
            Err(e) => return Err(e.into()),
        }
        self.get_router(&id)?.ok_or(RouterError::NotFound)
    }

    pub fn get_router(&self, id: &str) -> Result<Option<RouterRecord>> {
        self.conn.query_row(&format!("SELECT {ROUTER_COLS} FROM routers WHERE id = ?1"), [id], router_from_row).optional()
    }

    pub fn list_routers(&self) -> Result<Vec<RouterRecord>> {
        let mut stmt = self.conn.prepare(&format!("SELECT {ROUTER_COLS} FROM routers ORDER BY name COLLATE NOCASE"))?;
        stmt.query_map([], router_from_row)?.collect()
    }

    pub fn update_router(&mut self, id: &str, p: &RouterPatch) -> std::result::Result<RouterRecord, RouterError> {
        let cur = self.get_router(id)?.ok_or(RouterError::NotFound)?;
        let name = p.name.clone().unwrap_or(cur.name.clone());
        let host = p.host.clone().unwrap_or(cur.host.clone());
        let port = p.port.unwrap_or(cur.port);
        let username = p.username.clone().unwrap_or(cur.username.clone());
        check_router_fields(&name, &host, port, &username)?;
        let pin = match &p.tls_pin {
            None => cur.tls_pin.clone(),
            Some(None) => None,
            Some(Some(x)) => Some(normalize_fingerprint(x).ok_or_else(|| RouterError::Invalid("sidik jari sertifikat tidak valid".into()))?),
        };
        match self.conn.execute(
            "UPDATE routers SET name=?2, host=?3, port=?4, tls_pin=?5, username=?6, can_write=?7 WHERE id=?1",
            params![id, name.trim(), host.trim(), i64::from(port), pin, username.trim(), p.can_write.unwrap_or(cur.can_write)],
        ) {
            Ok(_) => {}
            Err(rusqlite::Error::SqliteFailure(e, _)) if e.code == rusqlite::ErrorCode::ConstraintViolation => {
                return Err(RouterError::Invalid(format!("nama router {} sudah dipakai", name.trim())));
            }
            Err(e) => return Err(e.into()),
        }
        self.get_router(id)?.ok_or(RouterError::NotFound)
    }

    /// Hapus router beserta event, user, profile, catatan lokal, dan password tersalinnya.
    /// Riwayat perubahan dipertahankan (jejak apa yang pernah dilakukan ke router itu).
    pub fn remove_router(&mut self, id: &str) -> std::result::Result<RouterRemoved, RouterError> {
        let tx = self.conn.transaction()?;
        if tx.execute("DELETE FROM routers WHERE id = ?1", [id])? == 0 {
            return Err(RouterError::NotFound);
        }
        let prefix = format!("{id}:");
        let like = "user_id LIKE ?1 || '%' ESCAPE '\\'";
        let esc = prefix.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_");
        for table in ["user_meta", "user_secrets"] {
            tx.execute(&format!("DELETE FROM {table} WHERE {like}"), [&esc])?;
        }
        let out = RouterRemoved {
            events: tx.execute("DELETE FROM events WHERE router_id = ?1", [id])? as u64,
            users: tx.execute("DELETE FROM users WHERE router_id = ?1", [id])? as u64,
            profiles: tx.execute("DELETE FROM profiles WHERE router_id = ?1", [id])? as u64,
        };
        tx.commit()?;
        Ok(out)
    }

    /// Hasil polling terakhir: online (dengan waktu) atau offline/error (dengan pesan).
    pub fn set_router_state(&mut self, id: &str, status: RouterStatus, error: Option<&str>, now_ms: i64) -> Result<()> {
        if status == RouterStatus::Online {
            self.conn.execute("UPDATE routers SET status = 'online', error = NULL, last_poll_at = ?2 WHERE id = ?1", params![id, now_ms])?;
        } else {
            self.conn.execute("UPDATE routers SET status = ?2, error = ?3 WHERE id = ?1", params![id, status.as_str(), error])?;
        }
        Ok(())
    }

    /// Model dan versi dari `/system/resource`, dan waktu import terakhir.
    pub fn set_router_info(&mut self, id: &str, model: &str, version: &str, imported_at: i64) -> Result<()> {
        self.conn.execute("UPDATE routers SET model = ?2, version = ?3, last_import_at = ?4 WHERE id = ?1", params![id, model, version, imported_at])?;
        Ok(())
    }

    // ---------------- password terenkripsi (ADR 0009) ----------------

    /// Buka penyimpanan password dengan kunci dari keyring. Basis data baru mencatat "kenari"
    /// terenkripsi; basis data yang sudah punya kenari menolak kunci yang salah (`WrongKey`).
    pub fn unlock(&mut self, key: &[u8; 32]) -> std::result::Result<(), PasswordError> {
        let vault = Vault::new(key);
        let canary: Option<Vec<u8>> = self.conn.query_row("SELECT v FROM vault_meta WHERE k = 'canary'", [], |r| r.get(0)).optional()?;
        match canary {
            Some(blob) => match vault.open(CANARY_AAD, &blob) {
                Ok(p) if p == CANARY_PLAIN => {}
                Ok(_) | Err(VaultError::Decrypt) => return Err(PasswordError::WrongKey),
                Err(e) => return Err(e.into()),
            },
            None => {
                self.conn.execute("INSERT INTO vault_meta (k, v) VALUES ('canary', ?1)", [vault.seal(CANARY_AAD, CANARY_PLAIN)?])?;
            }
        }
        self.vault = Some(vault);
        Ok(())
    }

    pub fn is_unlocked(&self) -> bool {
        self.vault.is_some()
    }

    /// Simpan (atau ganti) password sebuah user. Boleh sebelum barisnya ada (mis. setelah menambah
    /// user ke router, sebelum sinkronisasi berikutnya).
    pub fn set_user_password(&mut self, user_id: &str, password: &str, now_ms: i64) -> std::result::Result<(), PasswordError> {
        let v = self.vault.as_ref().ok_or(PasswordError::Locked)?;
        let enc = v.seal(user_id.as_bytes(), password.as_bytes())?;
        self.conn.execute(
            "INSERT INTO user_secrets (user_id, enc, updated_at) VALUES (?1,?2,?3) ON CONFLICT(user_id) DO UPDATE SET enc=?2, updated_at=?3",
            params![user_id, enc, now_ms],
        )?;
        Ok(())
    }

    /// Password user, atau `None` bila belum tersimpan. Hanya dipanggil untuk yang berizin Ubah;
    /// sengaja terpisah dari `list_users` agar password tidak ikut terbaca di daftar.
    pub fn user_password(&self, user_id: &str) -> std::result::Result<Option<String>, PasswordError> {
        let v = self.vault.as_ref().ok_or(PasswordError::Locked)?;
        let blob: Option<Vec<u8>> = self.conn.query_row("SELECT enc FROM user_secrets WHERE user_id = ?1", [user_id], |r| r.get(0)).optional()?;
        let Some(blob) = blob else { return Ok(None) };
        let plain = v.open(user_id.as_bytes(), &blob)?;
        String::from_utf8(plain).map(Some).map_err(|_| PasswordError::Vault(VaultError::Corrupt))
    }

    pub fn has_user_password(&self, user_id: &str) -> Result<bool> {
        self.conn.query_row("SELECT EXISTS(SELECT 1 FROM user_secrets WHERE user_id = ?1)", [user_id], |r| r.get(0))
    }

    /// Ganti kunci: semua password dibuka dengan kunci sekarang lalu dienkripsi ulang dengan kunci
    /// baru dalam satu transaksi. Gagal di tengah = tidak ada yang berubah.
    pub fn rekey(&mut self, new_key: &[u8; 32]) -> std::result::Result<(), PasswordError> {
        let old = self.vault.as_ref().ok_or(PasswordError::Locked)?;
        let new = Vault::new(new_key);
        let tx = self.conn.transaction()?;
        let rows: Vec<(String, Vec<u8>)> = {
            let mut stmt = tx.prepare("SELECT user_id, enc FROM user_secrets")?;
            stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<Result<_>>()?
        };
        for (id, blob) in rows {
            let plain = old.open(id.as_bytes(), &blob)?;
            tx.execute("UPDATE user_secrets SET enc = ?2 WHERE user_id = ?1", params![id, new.seal(id.as_bytes(), &plain)?])?;
        }
        tx.execute("UPDATE vault_meta SET v = ?1 WHERE k = 'canary'", [new.seal(CANARY_AAD, CANARY_PLAIN)?])?;
        tx.commit()?;
        self.vault = Some(new);
        Ok(())
    }

    // ---------------- riwayat perubahan ----------------

    pub fn record_audit(&mut self, e: &AuditEntry) -> Result<i64> {
        let (ok, done, err) = match &e.outcome {
            AuditOutcome::Ok => (1, 0, None),
            AuditOutcome::Failed { steps_done, error } => (0, *steps_done as i64, Some(error.as_str())),
        };
        self.conn.execute(
            "INSERT INTO audit (at_ms, actor, router_id, action, target, title, changes, ok, steps_done, error) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
            params![e.at_ms, e.actor, e.router_id, e.action, e.target, e.title, encode_changes(&e.changes), ok, done, err],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Terbaru dulu; `before_id` untuk halaman berikutnya.
    pub fn list_audit(&self, q: &AuditQuery) -> Result<Vec<StoredAudit>> {
        let mut sql = String::from("SELECT id, at_ms, actor, router_id, action, target, title, changes, ok, steps_done, error FROM audit WHERE 1=1");
        let mut args: Vec<Value> = Vec::new();
        if let Some(r) = &q.router_id {
            sql.push_str(" AND router_id = ?");
            args.push(r.clone().into());
        }
        if let Some(a) = &q.actor {
            sql.push_str(" AND actor = ?");
            args.push(a.clone().into());
        }
        if let Some(t) = q.target.as_deref().filter(|t| !t.is_empty()) {
            sql.push_str(" AND target LIKE ? ESCAPE '\\'");
            let esc = t.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_");
            args.push(format!("%{esc}%").into());
        }
        if q.only_failed {
            sql.push_str(" AND ok = 0");
        }
        if let Some(t) = q.since_ms {
            sql.push_str(" AND at_ms >= ?");
            args.push(t.into());
        }
        if let Some(t) = q.until_ms {
            sql.push_str(" AND at_ms <= ?");
            args.push(t.into());
        }
        if let Some(id) = q.before_id {
            sql.push_str(" AND id < ?");
            args.push(id.into());
        }
        sql.push_str(" ORDER BY id DESC LIMIT ?");
        args.push((if q.limit == 0 { 50 } else { q.limit.min(500) } as i64).into());
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params_from_iter(args), |r| {
            let ok: i64 = r.get(8)?;
            let outcome = if ok == 1 {
                AuditOutcome::Ok
            } else {
                AuditOutcome::Failed { steps_done: r.get::<_, i64>(9)?.max(0) as usize, error: r.get::<_, Option<String>>(10)?.unwrap_or_default() }
            };
            let changes: String = r.get(7)?;
            Ok(StoredAudit {
                id: r.get(0)?,
                entry: AuditEntry { at_ms: r.get(1)?, actor: r.get(2)?, router_id: r.get(3)?, action: r.get(4)?, target: r.get(5)?, title: r.get(6)?, changes: decode_changes(&changes), outcome },
            })
        })?;
        rows.collect()
    }

    // ---------------- catatan lokal ----------------

    pub fn set_user_meta(&mut self, user_id: &str, m: &UserMeta) -> Result<()> {
        self.conn.execute(
            "INSERT INTO user_meta (user_id, customer_name, phone, note, tags, updated_at) VALUES (?1,?2,?3,?4,?5,?6)
             ON CONFLICT(user_id) DO UPDATE SET customer_name=?2, phone=?3, note=?4, tags=?5, updated_at=?6",
            params![user_id, m.customer_name, m.phone, m.note, m.tags.join("\u{1f}"), m.updated_at],
        )?;
        Ok(())
    }

    pub fn get_user_meta(&self, user_id: &str) -> Result<Option<UserMeta>> {
        self.conn
            .query_row("SELECT customer_name, phone, note, tags, updated_at FROM user_meta WHERE user_id = ?1", [user_id], |r| {
                let tags: String = r.get(3)?;
                Ok(UserMeta {
                    customer_name: r.get(0)?,
                    phone: r.get(1)?,
                    note: r.get(2)?,
                    tags: tags.split('\u{1f}').filter(|t| !t.is_empty()).map(String::from).collect(),
                    updated_at: r.get(4)?,
                })
            })
            .optional()
    }

    // ---------------- retensi ----------------

    /// Yang akan dihapus oleh [`Store::purge`] dengan kebijakan ini (untuk pratinjau di UI).
    pub fn purge_preview(&self, now_ms: i64, p: &RetentionPolicy) -> Result<PurgeCounts> {
        let by_age = match p.event_days {
            Some(d) => self.scalar("SELECT COUNT(*) FROM events WHERE at_ms < ?1", params![now_ms - i64::from(d) * DAY_MS])?,
            None => 0,
        };
        // Sisa setelah dipangkas umur, lalu batas jumlah: hitung kelebihannya.
        let remaining = match p.event_days {
            Some(d) => self.scalar("SELECT COUNT(*) FROM events WHERE at_ms >= ?1", params![now_ms - i64::from(d) * DAY_MS])?,
            None => self.scalar("SELECT COUNT(*) FROM events", [])?,
        };
        let over = remaining.saturating_sub(u64::from(p.max_events));
        let (users, profiles) = match p.deleted_days {
            Some(d) => {
                let cutoff = now_ms - i64::from(d) * DAY_MS;
                (
                    self.scalar("SELECT COUNT(*) FROM users WHERE deleted_at IS NOT NULL AND deleted_at < ?1", [cutoff])?,
                    self.scalar("SELECT COUNT(*) FROM profiles WHERE deleted_at IS NOT NULL AND deleted_at < ?1", [cutoff])?,
                )
            }
            None => (0, 0),
        };
        let audit = match p.audit_days {
            Some(d) => self.scalar("SELECT COUNT(*) FROM audit WHERE at_ms < ?1", [now_ms - i64::from(d) * DAY_MS])?,
            None => 0,
        };
        Ok(PurgeCounts { events: by_age + over, users, profiles, audit })
    }

    /// Hapus event lewat umur, pangkas ke `max_events` terbaru, dan hapus permanen item soft delete
    /// yang terlalu lama (beserta catatan lokalnya).
    pub fn purge(&mut self, now_ms: i64, p: &RetentionPolicy) -> Result<PurgeCounts> {
        let tx = self.conn.transaction()?;
        let mut out = PurgeCounts::default();
        if let Some(d) = p.event_days {
            out.events += tx.execute("DELETE FROM events WHERE at_ms < ?1", [now_ms - i64::from(d) * DAY_MS])? as u64;
        }
        out.events += tx.execute(
            "DELETE FROM events WHERE id <= COALESCE((SELECT id FROM events ORDER BY id DESC LIMIT 1 OFFSET ?1), 0)",
            [i64::from(p.max_events)],
        )? as u64;
        if let Some(d) = p.deleted_days {
            let cutoff = now_ms - i64::from(d) * DAY_MS;
            for table in ["user_meta", "user_secrets"] {
                tx.execute(
                    &format!("DELETE FROM {table} WHERE user_id IN (SELECT id FROM users WHERE deleted_at IS NOT NULL AND deleted_at < ?1)"),
                    [cutoff],
                )?;
            }
            out.users = tx.execute("DELETE FROM users WHERE deleted_at IS NOT NULL AND deleted_at < ?1", [cutoff])? as u64;
            out.profiles = tx.execute("DELETE FROM profiles WHERE deleted_at IS NOT NULL AND deleted_at < ?1", [cutoff])? as u64;
        }
        if let Some(d) = p.audit_days {
            out.audit = tx.execute("DELETE FROM audit WHERE at_ms < ?1", [now_ms - i64::from(d) * DAY_MS])? as u64;
        }
        tx.commit()?;
        Ok(out)
    }

    /// Ukuran berkas basis data (halaman x ukuran halaman).
    pub fn size_bytes(&self) -> Result<u64> {
        let pages = self.scalar("PRAGMA page_count", [])?;
        let size = self.scalar("PRAGMA page_size", [])?;
        Ok(pages * size)
    }

    fn scalar(&self, sql: &str, p: impl rusqlite::Params) -> Result<u64> {
        self.conn.query_row(sql, p, |r| r.get::<_, i64>(0)).map(|n| n.max(0) as u64)
    }
}

fn user_id(router_id: &str, username: &str) -> String {
    format!("{router_id}:{username}")
}

/// Tandai soft delete semua baris router ini yang tidak ada di `seen` dan belum terhapus.
fn soft_delete_missing(tx: &rusqlite::Transaction<'_>, table: &str, id_col: &str, router_id: &str, seen: &[String], now_ms: i64) -> Result<u32> {
    let mut stmt = tx.prepare(&format!("SELECT {id_col} FROM {table} WHERE router_id = ?1 AND deleted_at IS NULL"))?;
    let live: Vec<String> = stmt.query_map([router_id], |r| r.get(0))?.collect::<Result<_>>()?;
    drop(stmt);
    let set: std::collections::HashSet<&str> = seen.iter().map(String::as_str).collect();
    let mut n = 0;
    for id in live.iter().filter(|id| !set.contains(id.as_str())) {
        tx.execute(&format!("UPDATE {table} SET deleted_at = ?2, deleted_reason = 'router' WHERE {id_col} = ?1"), params![id, now_ms])?;
        n += 1;
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Row;

    fn secret(name: &str, profile: &str, disabled: bool) -> PppSecret {
        let mut r = Row::new();
        r.insert(".id".into(), format!("*{name}"));
        r.insert("name".into(), name.into());
        r.insert("profile".into(), profile.into());
        r.insert("disabled".into(), if disabled { "true" } else { "false" }.into());
        PppSecret::from_row(&r)
    }
    fn act(name: &str, uptime: u64) -> PppActive {
        PppActive { id: format!("*a{name}"), name: name.into(), service: "pppoe".into(), address: Some("100.64.0.9".into()), caller_id: Some("AA".into()), uptime_secs: Some(uptime) }
    }
    fn ev(at: i64, kind: EventKind, user: Option<&str>) -> Event {
        Event { at_ms: at, kind, username: user.map(String::from), address: None, caller_id: None, message: format!("{} {kind:?}", user.unwrap_or("-")) }
    }
    fn policy(days: Option<u32>, max: u32, del: Option<u32>) -> RetentionPolicy {
        RetentionPolicy { event_days: days, max_events: max, deleted_days: del, audit_days: None }
    }

    #[test]
    fn migrasi_idempoten_dan_berkas() {
        let dir = std::env::temp_dir().join(format!("telemon-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("t.db");
        {
            let mut s = Store::open(&path).unwrap();
            s.insert_events("r1", &[ev(1, EventKind::Connected, Some("a"))]).unwrap();
        }
        let s = Store::open(&path).unwrap(); // buka ulang: tidak migrasi ulang, data tetap
        assert_eq!(s.count_events().unwrap(), 1);
        assert!(s.size_bytes().unwrap() > 0);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn event_filter_kursor_dan_pencarian() {
        let mut s = Store::open_in_memory().unwrap();
        let evs: Vec<Event> = (0..10).map(|i| ev(i * 10, if i % 2 == 0 { EventKind::Connected } else { EventKind::AuthFailed }, Some(if i < 5 { "budi_1" } else { "sari%x" }))).collect();
        s.insert_events("r1", &evs).unwrap();
        s.insert_events("r2", &[ev(5, EventKind::RouterDown, None)]).unwrap();

        let all = s.list_events(&EventQuery { limit: 100, ..Default::default() }).unwrap();
        assert_eq!(all.len(), 11);
        assert!(all[0].id > all[1].id);

        let q = EventQuery { router_id: Some("r1".into()), kinds: vec![EventKind::AuthFailed], limit: 100, ..Default::default() };
        assert_eq!(s.list_events(&q).unwrap().len(), 5);

        // Halaman dengan kursor tidak tumpang tindih.
        let p1 = s.list_events(&EventQuery { router_id: Some("r1".into()), limit: 4, ..Default::default() }).unwrap();
        let p2 = s.list_events(&EventQuery { router_id: Some("r1".into()), limit: 4, before_id: Some(p1.last().unwrap().id), ..Default::default() }).unwrap();
        assert_eq!((p1.len(), p2.len()), (4, 4));
        assert!(p1.last().unwrap().id > p2[0].id);

        // Pencarian: % dan _ diperlakukan huruf biasa.
        let f = |u: &str| s.list_events(&EventQuery { username: Some(u.into()), limit: 100, ..Default::default() }).unwrap().len();
        assert_eq!(f("BUDI"), 5);
        assert_eq!(f("%"), 5);
        assert_eq!(f("_1"), 5);
        assert_eq!(f("zzz"), 0);

        let range = s.list_events(&EventQuery { since_ms: Some(20), until_ms: Some(40), router_id: Some("r1".into()), limit: 100, ..Default::default() }).unwrap();
        assert_eq!(range.len(), 3);
        assert_eq!(s.list_events(&EventQuery { limit: 100_000, ..Default::default() }).unwrap().len(), 11);
    }

    #[test]
    fn sync_menimpa_dan_soft_delete_dari_router() {
        let mut s = Store::open_in_memory().unwrap();
        let r = s.sync_users("r1", 1_000_000, &[secret("budi", "10M", false), secret("sari", "20M", false)], &[act("budi", 60)]).unwrap();
        assert_eq!(r, SyncReport { added: 2, ..Default::default() });
        let users = s.list_users(Some("r1"), false).unwrap();
        assert_eq!(users.len(), 2);
        let budi = users.iter().find(|u| u.username == "budi").unwrap();
        assert_eq!(budi.status, UserStatus::Active);
        assert_eq!(budi.connected_at, Some(1_000_000 - 60_000));
        assert_eq!(users.iter().find(|u| u.username == "sari").unwrap().status, UserStatus::Inactive);

        // Router berubah: profile budi diganti, sari hilang, tika baru.
        let r = s.sync_users("r1", 2_000_000, &[secret("budi", "50M", true), secret("tika", "10M", false)], &[]).unwrap();
        assert_eq!(r, SyncReport { added: 1, updated: 1, soft_deleted: 1, restored: 0 });
        let all = s.list_users(Some("r1"), true).unwrap();
        let budi = all.iter().find(|u| u.username == "budi").unwrap();
        assert_eq!((budi.profile.as_str(), budi.status), ("50M", UserStatus::Disabled));
        let sari = all.iter().find(|u| u.username == "sari").unwrap();
        assert_eq!((sari.deleted_at, sari.deleted_reason.as_deref()), (Some(2_000_000), Some("router")));
        assert_eq!(s.list_users(Some("r1"), false).unwrap().len(), 2);

        // Muncul lagi: dipulihkan.
        let r = s.sync_users("r1", 3_000_000, &[secret("budi", "50M", true), secret("tika", "10M", false), secret("sari", "20M", false)], &[]).unwrap();
        assert_eq!((r.restored, r.soft_deleted), (1, 0));
        assert_eq!(s.list_users(Some("r1"), false).unwrap().len(), 3);

        // Router lain tidak tersentuh.
        s.sync_users("r2", 3_000_000, &[secret("x", "p", false)], &[]).unwrap();
        s.sync_users("r1", 4_000_000, &[], &[]).unwrap();
        assert_eq!(s.list_users(Some("r2"), false).unwrap().len(), 1);
        assert_eq!(s.list_users(Some("r1"), false).unwrap().len(), 0);
    }

    #[test]
    fn hapus_manual_tidak_dipulihkan_sync() {
        let mut s = Store::open_in_memory().unwrap();
        s.sync_users("r1", 1, &[secret("budi", "10M", false)], &[]).unwrap();
        assert!(s.soft_delete_user("r1:budi", 5).unwrap());
        assert!(!s.soft_delete_user("r1:budi", 6).unwrap());
        let r = s.sync_users("r1", 10, &[secret("budi", "10M", true)], &[]).unwrap();
        assert_eq!((r.restored, r.updated), (0, 1));
        let u = &s.list_users(None, true).unwrap()[0];
        assert_eq!((u.deleted_at, u.deleted_reason.as_deref()), (Some(5), Some("manual")));
        assert!(s.restore_user("r1:budi").unwrap());
        assert_eq!(s.list_users(None, false).unwrap().len(), 1);
    }

    #[test]
    fn event_terakhir_user_diperbarui() {
        let mut s = Store::open_in_memory().unwrap();
        s.sync_users("r1", 1, &[secret("budi", "10M", false)], &[]).unwrap();
        s.insert_events("r1", &[ev(100, EventKind::Connected, Some("budi")), ev(200, EventKind::AuthFailed, Some("budi")), ev(150, EventKind::Disconnected, Some("budi"))]).unwrap();
        let u = &s.list_users(None, false).unwrap()[0];
        assert_eq!((u.last_event, u.last_event_at), (Some(EventKind::AuthFailed), Some(200)));
    }

    #[test]
    fn profile_sync() {
        let mut s = Store::open_in_memory().unwrap();
        let p = |n: &str, rl: &str| {
            let mut r = Row::new();
            r.insert(".id".into(), "*1".into());
            r.insert("name".into(), n.into());
            r.insert("rate-limit".into(), rl.into());
            PppProfile::from_row(&r)
        };
        assert_eq!(s.sync_profiles("r1", 1, &[p("10M", "10M/10M"), p("20M", "20M/20M")]).unwrap().added, 2);
        let r = s.sync_profiles("r1", 2, &[p("10M", "12M/12M")]).unwrap();
        assert_eq!((r.updated, r.soft_deleted), (1, 1));
    }

    #[test]
    fn event_mengubah_status_dengan_aturan() {
        let mut s = Store::open_in_memory().unwrap();
        s.sync_users("r1", 1, &[secret("budi", "10M", false), secret("mati", "10M", true)], &[]).unwrap();
        let st = |s: &Store, n: &str| s.list_users(None, false).unwrap().into_iter().find(|u| u.username == n).unwrap();
        let mut c = ev(100, EventKind::Connected, Some("budi"));
        c.address = Some("100.64.0.7".into());
        s.insert_events("r1", &[c]).unwrap();
        let b = st(&s, "budi");
        assert_eq!((b.status, b.connected_at, b.address.as_deref()), (UserStatus::Active, Some(100), Some("100.64.0.7")));
        // Event lebih lama yang datang belakangan tidak membalikkan keadaan.
        s.insert_events("r1", &[ev(50, EventKind::Disconnected, Some("budi"))]).unwrap();
        assert_eq!(st(&s, "budi").status, UserStatus::Active);
        s.insert_events("r1", &[ev(200, EventKind::Disconnected, Some("budi"))]).unwrap();
        let b = st(&s, "budi");
        assert_eq!((b.status, b.connected_at, b.last_seen_at), (UserStatus::Inactive, None, Some(200)));
        // Secret dinonaktifkan tetap Disabled walau ada event sesi.
        s.insert_events("r1", &[ev(300, EventKind::Connected, Some("mati"))]).unwrap();
        assert_eq!(st(&s, "mati").status, UserStatus::Disabled);
        // User yang belum dikenal tidak membuat baris baru.
        s.insert_events("r1", &[ev(400, EventKind::Connected, Some("asing"))]).unwrap();
        assert_eq!(s.list_users(None, true).unwrap().len(), 2);
    }

    fn audit(at: i64, actor: &str, target: &str, ok: bool) -> AuditEntry {
        AuditEntry {
            at_ms: at,
            actor: actor.into(),
            router_id: "r1".into(),
            action: "user.edit".into(),
            target: target.into(),
            title: format!("Ubah user {target}"),
            changes: vec![
                Change { field: "profile".into(), from: Some("10M".into()), to: Some("20M".into()) },
                Change { field: "comment".into(), from: None, to: Some("a\u{1f}b\u{1e}c".into()) },
                Change { field: "remote-address".into(), from: Some("1.2.3.4".into()), to: None },
            ],
            outcome: if ok { AuditOutcome::Ok } else { AuditOutcome::Failed { steps_done: 1, error: "router menolak".into() } },
        }
    }

    #[test]
    fn riwayat_perubahan_simpan_dan_cari() {
        let mut s = Store::open_in_memory().unwrap();
        let first = audit(10, "admin", "budi_1", true);
        s.record_audit(&first).unwrap();
        s.record_audit(&audit(20, "operator", "sari", false)).unwrap();
        s.record_audit(&audit(30, "admin", "sari", true)).unwrap();

        let all = s.list_audit(&AuditQuery { limit: 100, ..Default::default() }).unwrap();
        assert_eq!(all.iter().map(|a| a.entry.at_ms).collect::<Vec<_>>(), vec![30, 20, 10]);
        // Perubahan kembali utuh: kosong vs terisi dibedakan; pemisah dalam teks diganti spasi.
        let got = &all[2].entry;
        assert_eq!(got.changes[0], first.changes[0]);
        assert_eq!(got.changes[1].to.as_deref(), Some("a b c"));
        assert_eq!((got.changes[1].from.clone(), got.changes[2].to.clone()), (None, None));
        assert_eq!(all[1].entry.outcome, AuditOutcome::Failed { steps_done: 1, error: "router menolak".into() });

        let q = |f: &dyn Fn(&mut AuditQuery)| {
            let mut q = AuditQuery { limit: 100, ..Default::default() };
            f(&mut q);
            s.list_audit(&q).unwrap().len()
        };
        assert_eq!(q(&|q| q.actor = Some("admin".into())), 2);
        assert_eq!(q(&|q| q.only_failed = true), 1);
        assert_eq!(q(&|q| q.target = Some("SARI".into())), 2);
        assert_eq!(q(&|q| q.target = Some("_".into())), 1);
        assert_eq!(q(&|q| q.router_id = Some("r2".into())), 0);
        assert_eq!(q(&|q| { q.since_ms = Some(15); q.until_ms = Some(25) }), 1);
        assert_eq!(q(&|q| { q.limit = 2; q.before_id = Some(3) }), 2);
    }

    #[test]
    fn retensi_riwayat_perubahan() {
        let mut s = Store::open_in_memory().unwrap();
        let now = 100 * DAY_MS;
        s.record_audit(&audit(now - 400 * DAY_MS, "a", "x", true)).unwrap();
        s.record_audit(&audit(now - DAY_MS, "a", "y", true)).unwrap();
        let pol = RetentionPolicy { event_days: None, max_events: 1000, deleted_days: None, audit_days: Some(365) };
        assert_eq!(s.purge_preview(now, &pol).unwrap().audit, 1);
        assert_eq!(s.purge(now, &pol).unwrap().audit, 1);
        assert_eq!(s.list_audit(&AuditQuery { limit: 10, ..Default::default() }).unwrap().len(), 1);
        let forever = RetentionPolicy { audit_days: None, ..pol };
        assert_eq!(s.purge(now, &forever).unwrap().audit, 0);
    }

    #[test]
    fn migrasi_dari_versi_lama() {
        let dir = std::env::temp_dir().join(format!("telemon-mig-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("old.db");
        {
            let c = Connection::open(&path).unwrap();
            c.execute_batch(MIGRATIONS[0]).unwrap();
            c.pragma_update(None, "user_version", 1i64).unwrap();
            c.execute("INSERT INTO events (at_ms, router_id, kind, message) VALUES (1,'r1','connected','lama')", []).unwrap();
        }
        let mut s = Store::open(&path).unwrap();
        assert_eq!(s.count_events().unwrap(), 1, "data lama tetap");
        s.record_audit(&audit(1, "a", "x", true)).unwrap(); // tabel baru ada
        std::fs::remove_dir_all(dir).unwrap();
    }

    fn secret_pw(name: &str, pw: Option<&str>) -> PppSecret {
        let mut sec = secret(name, "10M", false);
        sec.password = pw.map(String::from);
        sec
    }

    fn key() -> [u8; 32] {
        crate::vault::generate_key().unwrap()
    }

    #[test]
    fn password_terkunci_tanpa_kunci() {
        let mut s = Store::open_in_memory().unwrap();
        assert!(!s.is_unlocked());
        assert!(matches!(s.set_user_password("r1:budi", "x", 1), Err(PasswordError::Locked)));
        assert!(matches!(s.user_password("r1:budi"), Err(PasswordError::Locked)));
        // Sinkronisasi tanpa kunci tidak menyimpan password apa pun.
        s.sync_users("r1", 1, &[secret_pw("budi", Some("rahasia123"))], &[]).unwrap();
        assert!(!s.has_user_password("r1:budi").unwrap());
        assert!(matches!(s.rekey(&key()), Err(PasswordError::Locked)));
    }

    #[test]
    fn password_tersimpan_terenkripsi_dan_mengikuti_router() {
        let mut s = Store::open_in_memory().unwrap();
        s.unlock(&key()).unwrap();
        s.sync_users("r1", 1, &[secret_pw("budi", Some("rahasia123")), secret_pw("sari", None)], &[]).unwrap();
        assert_eq!(s.user_password("r1:budi").unwrap().as_deref(), Some("rahasia123"));
        assert_eq!(s.user_password("r1:sari").unwrap(), None);
        // Yang tersimpan di berkas adalah blob terenkripsi, bukan teks.
        let raw: Vec<u8> = s.conn.query_row("SELECT enc FROM user_secrets WHERE user_id='r1:budi'", [], |r| r.get(0)).unwrap();
        assert!(!raw.windows(10).any(|w| w == b"rahasia123"));
        // Router tidak mengirim password lagi: salinan lama dibiarkan. Router mengirim yang baru: ditimpa.
        s.sync_users("r1", 2, &[secret_pw("budi", None)], &[]).unwrap();
        assert_eq!(s.user_password("r1:budi").unwrap().as_deref(), Some("rahasia123"));
        s.sync_users("r1", 3, &[secret_pw("budi", Some("baru-baru"))], &[]).unwrap();
        assert_eq!(s.user_password("r1:budi").unwrap().as_deref(), Some("baru-baru"));
        // Password tidak ikut daftar user.
        assert!(!format!("{:?}", s.list_users(None, true).unwrap()).contains("baru-baru"));
    }

    #[test]
    fn password_boleh_disimpan_sebelum_user_ada() {
        let mut s = Store::open_in_memory().unwrap();
        s.unlock(&key()).unwrap();
        s.set_user_password("r1:tika", "tika-pass", 5).unwrap();
        assert_eq!(s.user_password("r1:tika").unwrap().as_deref(), Some("tika-pass"));
        s.set_user_password("r1:tika", "ganti-lagi", 6).unwrap();
        assert_eq!(s.user_password("r1:tika").unwrap().as_deref(), Some("ganti-lagi"));
    }

    #[test]
    fn kunci_salah_ditolak_dan_blob_terpindah_gagal() {
        let dir = std::env::temp_dir().join(format!("telemon-vault-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("v.db");
        let k = key();
        {
            let mut s = Store::open(&path).unwrap();
            s.unlock(&k).unwrap();
            s.set_user_password("r1:budi", "rahasia123", 1).unwrap();
            s.set_user_password("r1:sari", "lain-lain", 1).unwrap();
        }
        let mut s = Store::open(&path).unwrap();
        assert!(matches!(s.unlock(&key()), Err(PasswordError::WrongKey)));
        assert!(!s.is_unlocked(), "kunci salah tidak boleh membuka");
        s.unlock(&k).unwrap();
        assert_eq!(s.user_password("r1:budi").unwrap().as_deref(), Some("rahasia123"));
        // Menukar blob antar baris: terdeteksi karena terikat ke identitas baris.
        s.conn.execute("UPDATE user_secrets SET enc = (SELECT enc FROM user_secrets WHERE user_id='r1:sari') WHERE user_id='r1:budi'", []).unwrap();
        assert!(matches!(s.user_password("r1:budi"), Err(PasswordError::Vault(VaultError::Decrypt))));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn ganti_kunci_menjaga_password() {
        let mut s = Store::open_in_memory().unwrap();
        let (k1, k2) = (key(), key());
        s.unlock(&k1).unwrap();
        s.set_user_password("r1:a", "satu-satu", 1).unwrap();
        s.set_user_password("r1:b", "dua-dua-dua", 1).unwrap();
        s.rekey(&k2).unwrap();
        assert_eq!(s.user_password("r1:b").unwrap().as_deref(), Some("dua-dua-dua"));
        // Kunci lama tidak lagi cocok; kunci baru cocok.
        let canary: Vec<u8> = s.conn.query_row("SELECT v FROM vault_meta WHERE k='canary'", [], |r| r.get(0)).unwrap();
        assert!(Vault::new(&k1).open(CANARY_AAD, &canary).is_err());
        assert!(Vault::new(&k2).open(CANARY_AAD, &canary).is_ok());
    }

    #[test]
    fn purge_menghapus_password_user_terhapus_permanen() {
        let mut s = Store::open_in_memory().unwrap();
        s.unlock(&key()).unwrap();
        let now = 100 * DAY_MS;
        s.sync_users("r1", now - 60 * DAY_MS, &[secret_pw("lama", Some("lama-lama")), secret_pw("hidup", Some("hidup-hidup"))], &[]).unwrap();
        s.sync_users("r1", now - 59 * DAY_MS, &[secret_pw("hidup", None)], &[]).unwrap(); // lama hilang di router
        s.purge(now, &policy(None, 1000, Some(30))).unwrap();
        assert!(!s.has_user_password("r1:lama").unwrap());
        assert!(s.has_user_password("r1:hidup").unwrap());
    }

    fn new_router(name: &str) -> NewRouter {
        NewRouter { name: name.into(), host: "10.0.0.1".into(), port: 8728, tls_pin: None, username: "api".into(), can_write: false }
    }

    #[test]
    fn router_tambah_validasi_dan_unik() {
        let mut s = Store::open_in_memory().unwrap();
        let bad = |s: &mut Store, f: &dyn Fn(&mut NewRouter)| {
            let mut n = new_router("x");
            f(&mut n);
            match s.add_router(&n, 1) {
                Err(RouterError::Invalid(m)) => m,
                other => panic!("harusnya Invalid, dapat {other:?}"),
            }
        };
        assert!(bad(&mut s, &|n| n.name = "  ".into()).contains("nama"));
        assert!(bad(&mut s, &|n| n.host = "a b".into()).contains("host"));
        assert!(bad(&mut s, &|n| n.host = "http://x/".into()).contains("host"));
        assert!(bad(&mut s, &|n| n.port = 0).contains("port"));
        assert!(bad(&mut s, &|n| n.username = "".into()).contains("username"));
        assert!(bad(&mut s, &|n| n.tls_pin = Some("AA:BB".into())).contains("sidik jari"));

        let pin_lower = "ab".repeat(32);
        let r = s.add_router(&NewRouter { tls_pin: Some(pin_lower), can_write: true, ..new_router("Pusat") }, 100).unwrap();
        assert!(r.id.starts_with("r-") && r.id.len() == 14);
        assert_eq!(r.tls_pin.as_deref(), Some(&*["AB"; 32].join(":")));
        assert_eq!((r.addr().as_str(), r.can_write, r.status, r.created_at), ("10.0.0.1:8728", true, RouterStatus::Offline, 100));
        // Nama unik tanpa membedakan huruf besar kecil.
        match s.add_router(&new_router("pusat"), 1) {
            Err(RouterError::Invalid(m)) => assert!(m.contains("sudah dipakai")),
            other => panic!("{other:?}"),
        }
        let b = s.add_router(&new_router("Cabang"), 2).unwrap();
        assert_ne!(r.id, b.id);
        assert_eq!(s.list_routers().unwrap().iter().map(|x| x.name.as_str()).collect::<Vec<_>>(), vec!["Cabang", "Pusat"]);
        assert_eq!(s.get_router("tidak-ada").unwrap(), None);
    }

    #[cfg(feature = "tls")]
    #[test]
    fn router_tls_mode_dari_pin() {
        use crate::router::TlsMode;
        let mut s = Store::open_in_memory().unwrap();
        let plain = s.add_router(&new_router("a"), 1).unwrap();
        assert_eq!(plain.tls_mode(), TlsMode::Plain);
        let pinned = s.add_router(&NewRouter { tls_pin: Some("cd".repeat(32)), port: 8729, ..new_router("b") }, 1).unwrap();
        match pinned.tls_mode() {
            TlsMode::Pinned(fp) => assert_eq!(fp.to_hex(), pinned.tls_pin.clone().unwrap()),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn router_ubah() {
        let mut s = Store::open_in_memory().unwrap();
        let a = s.add_router(&NewRouter { tls_pin: Some("11".repeat(32)), ..new_router("a") }, 1).unwrap();
        let b = s.add_router(&new_router("b"), 1).unwrap();
        // Tanpa perubahan kolom lain: pin tetap.
        let u = s.update_router(&a.id, &RouterPatch { name: Some("a2".into()), port: Some(8729), can_write: Some(true), ..Default::default() }).unwrap();
        assert_eq!((u.name.as_str(), u.port, u.can_write, u.tls_pin.clone()), ("a2", 8729, true, a.tls_pin.clone()));
        // Kembali ke API biasa, lalu pin baru.
        assert_eq!(s.update_router(&a.id, &RouterPatch { tls_pin: Some(None), ..Default::default() }).unwrap().tls_pin, None);
        let p = s.update_router(&a.id, &RouterPatch { tls_pin: Some(Some("22".repeat(32))), ..Default::default() }).unwrap();
        assert_eq!(p.tls_pin.as_deref(), Some(&*["22"; 32].join(":")));
        // Validasi dan nama ganda.
        assert!(matches!(s.update_router(&a.id, &RouterPatch { port: Some(0), ..Default::default() }), Err(RouterError::Invalid(_))));
        assert!(matches!(s.update_router(&a.id, &RouterPatch { tls_pin: Some(Some("zz".into())), ..Default::default() }), Err(RouterError::Invalid(_))));
        assert!(matches!(s.update_router(&a.id, &RouterPatch { name: Some("B".into()), ..Default::default() }), Err(RouterError::Invalid(_))));
        assert!(matches!(s.update_router("nope", &RouterPatch::default()), Err(RouterError::NotFound)));
        assert_eq!(s.get_router(&b.id).unwrap().unwrap().name, "b");
    }

    #[test]
    fn router_hapus_menghapus_datanya_tapi_riwayat_tetap() {
        let mut s = Store::open_in_memory().unwrap();
        s.unlock(&key()).unwrap();
        let r1 = s.add_router(&new_router("satu"), 1).unwrap();
        let r2 = s.add_router(&new_router("dua"), 1).unwrap();
        for r in [&r1.id, &r2.id] {
            s.sync_users(r, 1, &[secret_pw("budi", Some("rahasia123"))], &[]).unwrap();
            s.insert_events(r, &[ev(5, EventKind::Connected, Some("budi"))]).unwrap();
            s.set_user_meta(&format!("{r}:budi"), &UserMeta { note: Some("n".into()), updated_at: 1, ..Default::default() }).unwrap();
            let mut a = audit(9, "admin", "budi", true);
            a.router_id = r.clone();
            s.record_audit(&a).unwrap();
        }
        let out = s.remove_router(&r1.id).unwrap();
        assert_eq!(out, RouterRemoved { events: 1, users: 1, profiles: 0 });
        assert_eq!(s.get_router(&r1.id).unwrap(), None);
        assert!(!s.has_user_password(&format!("{}:budi", r1.id)).unwrap());
        assert_eq!(s.get_user_meta(&format!("{}:budi", r1.id)).unwrap(), None);
        // Router lain utuh, dan riwayat perubahan router yang dihapus masih ada.
        assert!(s.has_user_password(&format!("{}:budi", r2.id)).unwrap());
        assert_eq!(s.list_users(Some(&r2.id), false).unwrap().len(), 1);
        assert_eq!(s.list_audit(&AuditQuery { limit: 10, ..Default::default() }).unwrap().len(), 2);
        assert!(matches!(s.remove_router(&r1.id), Err(RouterError::NotFound)));
    }

    #[test]
    fn router_status_dan_info() {
        let mut s = Store::open_in_memory().unwrap();
        let r = s.add_router(&new_router("a"), 1).unwrap();
        s.set_router_state(&r.id, RouterStatus::Online, None, 500).unwrap();
        s.set_router_info(&r.id, "RB4011", "7.15 (stable)", 600).unwrap();
        let g = s.get_router(&r.id).unwrap().unwrap();
        assert_eq!((g.status, g.last_poll_at, g.last_import_at, g.model.as_str(), g.version.as_str()), (RouterStatus::Online, Some(500), Some(600), "RB4011", "7.15 (stable)"));
        s.set_router_state(&r.id, RouterStatus::Offline, Some("timeout"), 700).unwrap();
        let g = s.get_router(&r.id).unwrap().unwrap();
        // Waktu polling terakhir yang BERHASIL tidak ikut berubah saat gagal.
        assert_eq!((g.status, g.error.as_deref(), g.last_poll_at), (RouterStatus::Offline, Some("timeout"), Some(500)));
        s.set_router_state(&r.id, RouterStatus::Online, None, 800).unwrap();
        assert_eq!(s.get_router(&r.id).unwrap().unwrap().error, None);
    }

    #[test]
    fn normalisasi_sidik_jari() {
        assert_eq!(normalize_fingerprint(&"ab".repeat(32)).unwrap().len(), 95);
        assert_eq!(normalize_fingerprint(&format!(" {} ", ["aB"; 32].join("-"))), Some(["AB"; 32].join(":")));
        assert_eq!(normalize_fingerprint("AA:BB"), None);
        assert_eq!(normalize_fingerprint(&"gg".repeat(32)), None);
    }

    #[test]
    fn jenis_event_tak_dikenal_dilewati() {
        let mut s = Store::open_in_memory().unwrap();
        s.insert_events("r1", &[ev(1, EventKind::Connected, Some("a"))]).unwrap();
        s.conn.execute("INSERT INTO events (at_ms, router_id, kind, message) VALUES (2, 'r1', 'jenis_baru', 'x')", []).unwrap();
        let l = s.list_events(&EventQuery { limit: 10, ..Default::default() }).unwrap();
        assert_eq!(l.len(), 1);
        assert_eq!(s.count_events().unwrap(), 2);
    }

    #[test]
    fn catatan_lokal() {
        let mut s = Store::open_in_memory().unwrap();
        assert_eq!(s.get_user_meta("r1:budi").unwrap(), None);
        let m = UserMeta { customer_name: Some("Budi S".into()), phone: None, note: Some("VIP".into()), tags: vec!["vip".into(), "rt-3".into()], updated_at: 9 };
        s.set_user_meta("r1:budi", &m).unwrap();
        assert_eq!(s.get_user_meta("r1:budi").unwrap(), Some(m));
        let m2 = UserMeta { tags: vec![], updated_at: 10, ..Default::default() };
        s.set_user_meta("r1:budi", &m2).unwrap();
        assert_eq!(s.get_user_meta("r1:budi").unwrap(), Some(m2));
    }

    #[test]
    fn retensi_umur_jumlah_dan_terhapus() {
        let mut s = Store::open_in_memory().unwrap();
        let now = 100 * DAY_MS;
        // 3 event lama (40 hari), 5 baru.
        let mut evs: Vec<Event> = (0..3).map(|i| ev(now - 40 * DAY_MS + i, EventKind::Connected, Some("a"))).collect();
        evs.extend((0..5).map(|i| ev(now - DAY_MS + i, EventKind::Connected, Some("a"))));
        s.insert_events("r1", &evs).unwrap();
        // Dua user terhapus: satu lama (60 hari), satu baru (1 hari), plus catatan untuk yang lama.
        s.sync_users("r1", now - 60 * DAY_MS, &[secret("lama", "p", false), secret("baru", "p", false), secret("hidup", "p", false)], &[]).unwrap();
        s.sync_users("r1", now - 60 * DAY_MS + 1, &[secret("hidup", "p", false), secret("baru", "p", false)], &[]).unwrap();
        s.sync_users("r1", now - DAY_MS, &[secret("hidup", "p", false)], &[]).unwrap();
        s.set_user_meta("r1:lama", &UserMeta { note: Some("x".into()), updated_at: 1, ..Default::default() }).unwrap();

        let pol = policy(Some(30), 4, Some(30));
        let pv = s.purge_preview(now, &pol).unwrap();
        assert_eq!(pv, PurgeCounts { events: 3 + 1, users: 1, ..Default::default() });

        // Pratinjau tidak mengubah apa pun.
        assert_eq!(s.count_events().unwrap(), 8);
        let done = s.purge(now, &pol).unwrap();
        assert_eq!(done, pv);
        assert_eq!(s.count_events().unwrap(), 4);
        // Yang tersisa adalah yang terbaru.
        let left = s.list_events(&EventQuery { limit: 10, ..Default::default() }).unwrap();
        assert!(left.iter().all(|e| e.event.at_ms >= now - DAY_MS));
        let users = s.list_users(None, true).unwrap();
        assert_eq!(users.len(), 2);
        assert!(users.iter().any(|u| u.username == "baru" && u.deleted_at.is_some()));
        assert_eq!(s.get_user_meta("r1:lama").unwrap(), None);

        // Kebijakan "selamanya": tidak menghapus apa pun selain kelebihan jumlah.
        let none = s.purge(now, &policy(None, 1000, None)).unwrap();
        assert_eq!(none, PurgeCounts::default());
        // Batas jumlah saja.
        let cut = s.purge(now, &policy(None, 2, None)).unwrap();
        assert_eq!(cut.events, 2);
        assert_eq!(s.count_events().unwrap(), 2);
    }
}
