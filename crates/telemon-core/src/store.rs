//! Penyimpanan SQLite: event, salinan user/profile dari router (Mikrotik = sumber kebenaran,
//! ADR 0004), catatan lokal, dan retensi (ADR 0006). Password belum disimpan di sini; itu
//! butuh enkripsi dengan kunci keyring (ADR 0009) dan dikerjakan terpisah.

use std::path::Path;

use rusqlite::types::Value;
use rusqlite::{Connection, OptionalExtension, params, params_from_iter};

use crate::events::{Event, EventKind};
use crate::model::{PppActive, PppProfile, PppSecret};
use crate::poller::{UserStatus, user_status};

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
"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PurgeCounts {
    pub events: u64,
    pub users: u64,
    pub profiles: u64,
}

/// Nilai `None` = tanpa batas waktu (selamanya / tidak pernah).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetentionPolicy {
    pub event_days: Option<u32>,
    pub max_events: u32,
    pub deleted_days: Option<u32>,
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
}

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
        Ok(Self { conn })
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
                tx.execute(
                    "UPDATE users SET last_event = ?1, last_event_at = ?2 WHERE id = ?3 AND (last_event_at IS NULL OR last_event_at <= ?2)",
                    params![e.kind.as_str(), e.at_ms, user_id(router_id, u)],
                )?;
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
        Ok(PurgeCounts { events: by_age + over, users, profiles })
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
            tx.execute(
                "DELETE FROM user_meta WHERE user_id IN (SELECT id FROM users WHERE deleted_at IS NOT NULL AND deleted_at < ?1)",
                [cutoff],
            )?;
            out.users = tx.execute("DELETE FROM users WHERE deleted_at IS NOT NULL AND deleted_at < ?1", [cutoff])? as u64;
            out.profiles = tx.execute("DELETE FROM profiles WHERE deleted_at IS NOT NULL AND deleted_at < ?1", [cutoff])? as u64;
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
        RetentionPolicy { event_days: days, max_events: max, deleted_days: del }
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
        assert_eq!(pv, PurgeCounts { events: 3 + 1, users: 1, profiles: 0 });

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
