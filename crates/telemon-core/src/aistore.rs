//! Penyimpanan fitur AI chat (ADR 0007, 0024): provider, catatan pemakaian, dan riwayat percakapan.
//! Hanya SQL: JSON pesan disimpan sebagai teks buram (diurai di `telemon-ai`), dan kunci API tidak pernah
//! masuk basis data (hanya petunjuk `key_hint`; kuncinya di `SecretStore`).

use rusqlite::{OptionalExtension, Row, params};

use crate::store::Store;

type Result<T> = rusqlite::Result<T>;

#[derive(Debug, Clone, PartialEq)]
pub struct ProviderRow {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub model: String,
    pub base_url: Option<String>,
    pub key_hint: Option<String>,
    /// `ok` | `error` | `unconfigured`.
    pub status: String,
    pub error: Option<String>,
    pub latency_ms: Option<i64>,
    pub checked_at: Option<i64>,
    pub is_default: bool,
    /// Harga USD per 1 juta token; kosong = biaya tidak dihitung.
    pub price_in: Option<f64>,
    pub price_out: Option<f64>,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UsageRow {
    pub at: i64,
    pub provider_id: String,
    pub model: String,
    /// Id akun penanya; `desktop` untuk aplikasi desktop.
    pub account: String,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cost_usd: f64,
    pub ok: bool,
    pub latency_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatMeta {
    pub id: String,
    pub title: String,
    pub pinned: bool,
    pub created_at: i64,
    pub updated_at: i64,
    /// Jumlah dan pratinjau dihitung `telemon-ai` dari JSON; di sini hanya ukuran teksnya.
    pub messages_len: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatRow {
    pub id: String,
    pub account: String,
    pub title: String,
    pub pinned: bool,
    pub created_at: i64,
    pub updated_at: i64,
    /// Larik JSON pesan (bentuknya urusan `telemon-ai`).
    pub messages: String,
}

fn provider(r: &Row<'_>) -> Result<ProviderRow> {
    Ok(ProviderRow {
        id: r.get(0)?,
        name: r.get(1)?,
        kind: r.get(2)?,
        model: r.get(3)?,
        base_url: r.get(4)?,
        key_hint: r.get(5)?,
        status: r.get(6)?,
        error: r.get(7)?,
        latency_ms: r.get(8)?,
        checked_at: r.get(9)?,
        is_default: r.get::<_, i64>(10)? != 0,
        price_in: r.get(11)?,
        price_out: r.get(12)?,
        created_at: r.get(13)?,
    })
}

const PROVIDER_COLS: &str = "id, name, kind, model, base_url, key_hint, status, error, latency_ms, checked_at, is_default, price_in, price_out, created_at";

impl Store {
    pub fn list_ai_providers(&self) -> Result<Vec<ProviderRow>> {
        let mut st = self.conn.prepare(&format!("SELECT {PROVIDER_COLS} FROM ai_providers ORDER BY is_default DESC, created_at, id"))?;
        st.query_map([], provider)?.collect()
    }

    pub fn get_ai_provider(&self, id: &str) -> Result<Option<ProviderRow>> {
        self.conn.query_row(&format!("SELECT {PROVIDER_COLS} FROM ai_providers WHERE id = ?1"), [id], provider).optional()
    }

    pub fn default_ai_provider(&self) -> Result<Option<ProviderRow>> {
        self.conn
            .query_row(&format!("SELECT {PROVIDER_COLS} FROM ai_providers ORDER BY is_default DESC, created_at LIMIT 1"), [], provider)
            .optional()
    }

    /// Simpan (tambah atau ganti). Provider pertama otomatis menjadi bawaan.
    pub fn save_ai_provider(&mut self, p: &ProviderRow) -> Result<()> {
        let tx = self.conn.transaction()?;
        let has_default: i64 = tx.query_row("SELECT COUNT(*) FROM ai_providers WHERE is_default = 1 AND id <> ?1", [&p.id], |r| r.get(0))?;
        let is_default = p.is_default || has_default == 0;
        if is_default {
            tx.execute("UPDATE ai_providers SET is_default = 0 WHERE id <> ?1", [&p.id])?;
        }
        tx.execute(
            "INSERT INTO ai_providers (id, name, kind, model, base_url, key_hint, status, error, latency_ms, checked_at, is_default, price_in, price_out, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
             ON CONFLICT(id) DO UPDATE SET name = ?2, kind = ?3, model = ?4, base_url = ?5, key_hint = ?6, status = ?7, error = ?8,
               latency_ms = ?9, checked_at = ?10, is_default = ?11, price_in = ?12, price_out = ?13",
            params![p.id, p.name, p.kind, p.model, p.base_url, p.key_hint, p.status, p.error, p.latency_ms, p.checked_at, is_default as i64, p.price_in, p.price_out, p.created_at],
        )?;
        tx.commit()
    }

    pub fn set_default_ai_provider(&mut self, id: &str) -> Result<bool> {
        let tx = self.conn.transaction()?;
        let n = tx.execute("UPDATE ai_providers SET is_default = 1 WHERE id = ?1", [id])?;
        if n == 0 {
            return Ok(false);
        }
        tx.execute("UPDATE ai_providers SET is_default = 0 WHERE id <> ?1", [id])?;
        tx.commit()?;
        Ok(true)
    }

    pub fn set_ai_provider_status(&mut self, id: &str, status: &str, error: Option<&str>, latency_ms: Option<i64>, checked_at: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE ai_providers SET status = ?2, error = ?3, latency_ms = ?4, checked_at = ?5 WHERE id = ?1",
            params![id, status, error, latency_ms, checked_at],
        )?;
        Ok(())
    }

    /// Hapus provider; bila ia bawaan, provider tertua yang tersisa menjadi bawaan.
    pub fn delete_ai_provider(&mut self, id: &str) -> Result<bool> {
        let tx = self.conn.transaction()?;
        let n = tx.execute("DELETE FROM ai_providers WHERE id = ?1", [id])?;
        if n == 0 {
            return Ok(false);
        }
        let any_default: i64 = tx.query_row("SELECT COUNT(*) FROM ai_providers WHERE is_default = 1", [], |r| r.get(0))?;
        if any_default == 0 {
            tx.execute("UPDATE ai_providers SET is_default = 1 WHERE id = (SELECT id FROM ai_providers ORDER BY created_at, id LIMIT 1)", [])?;
        }
        tx.commit()?;
        Ok(true)
    }

    pub fn add_ai_usage(&mut self, u: &UsageRow) -> Result<()> {
        self.conn.execute(
            "INSERT INTO ai_usage (at, provider_id, model, account, input_tokens, output_tokens, cost_usd, ok, latency_ms) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![u.at, u.provider_id, u.model, u.account, u.input_tokens, u.output_tokens, u.cost_usd, u.ok as i64, u.latency_ms],
        )?;
        Ok(())
    }

    pub fn ai_usage_since(&self, since_ms: i64) -> Result<Vec<UsageRow>> {
        let mut st = self.conn.prepare(
            "SELECT at, provider_id, model, account, input_tokens, output_tokens, cost_usd, ok, latency_ms FROM ai_usage WHERE at >= ?1 ORDER BY at",
        )?;
        st.query_map([since_ms], |r| {
            Ok(UsageRow {
                at: r.get(0)?,
                provider_id: r.get(1)?,
                model: r.get(2)?,
                account: r.get(3)?,
                input_tokens: r.get(4)?,
                output_tokens: r.get(5)?,
                cost_usd: r.get(6)?,
                ok: r.get::<_, i64>(7)? != 0,
                latency_ms: r.get(8)?,
            })
        })?
        .collect()
    }

    /// Daftar percakapan milik satu akun, yang disematkan lebih dulu lalu yang terbaru.
    pub fn list_ai_chats(&self, account: &str) -> Result<Vec<(ChatMeta, String)>> {
        let mut st = self.conn.prepare(
            "SELECT id, title, pinned, created_at, updated_at, messages FROM ai_chats WHERE account = ?1 ORDER BY pinned DESC, updated_at DESC",
        )?;
        st.query_map([account], |r| {
            let messages: String = r.get(5)?;
            Ok((
                ChatMeta { id: r.get(0)?, title: r.get(1)?, pinned: r.get::<_, i64>(2)? != 0, created_at: r.get(3)?, updated_at: r.get(4)?, messages_len: messages.len() },
                messages,
            ))
        })?
        .collect()
    }

    pub fn get_ai_chat(&self, account: &str, id: &str) -> Result<Option<ChatRow>> {
        self.conn
            .query_row("SELECT id, account, title, pinned, created_at, updated_at, messages FROM ai_chats WHERE id = ?1 AND account = ?2", params![id, account], |r| {
                Ok(ChatRow { id: r.get(0)?, account: r.get(1)?, title: r.get(2)?, pinned: r.get::<_, i64>(3)? != 0, created_at: r.get(4)?, updated_at: r.get(5)?, messages: r.get(6)? })
            })
            .optional()
    }

    /// Simpan percakapan. Id yang sudah dimiliki akun lain ditolak (false) agar tidak menimpa riwayat orang lain.
    pub fn save_ai_chat(&mut self, c: &ChatRow) -> Result<bool> {
        let owner: Option<String> = self.conn.query_row("SELECT account FROM ai_chats WHERE id = ?1", [&c.id], |r| r.get(0)).optional()?;
        if owner.as_deref().is_some_and(|o| o != c.account) {
            return Ok(false);
        }
        self.conn.execute(
            "INSERT INTO ai_chats (id, account, title, pinned, created_at, updated_at, messages) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(id) DO UPDATE SET title = ?3, pinned = ?4, updated_at = ?6, messages = ?7",
            params![c.id, c.account, c.title, c.pinned as i64, c.created_at, c.updated_at, c.messages],
        )?;
        Ok(true)
    }

    pub fn delete_ai_chat(&mut self, account: &str, id: &str) -> Result<bool> {
        Ok(self.conn.execute("DELETE FROM ai_chats WHERE id = ?1 AND account = ?2", params![id, account])? > 0)
    }

    /// Hapus percakapan yang terakhir diubah sebelum `before_ms` (retensi), kecuali yang disematkan.
    pub fn purge_ai_chats(&mut self, before_ms: i64) -> Result<u64> {
        Ok(self.conn.execute("DELETE FROM ai_chats WHERE updated_at < ?1 AND pinned = 0", [before_ms])? as u64)
    }

    /// Hapus seluruh riwayat satu akun (mis. akun dihapus).
    pub fn delete_ai_chats_of(&mut self, account: &str) -> Result<u64> {
        Ok(self.conn.execute("DELETE FROM ai_chats WHERE account = ?1", [account])? as u64)
    }

    pub fn purge_ai_usage(&mut self, before_ms: i64) -> Result<u64> {
        Ok(self.conn.execute("DELETE FROM ai_usage WHERE at < ?1", [before_ms])? as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prov(id: &str, default: bool, at: i64) -> ProviderRow {
        ProviderRow { id: id.into(), name: id.into(), kind: "openai".into(), model: "m".into(), base_url: None, key_hint: Some("sk-…abcd".into()), status: "unconfigured".into(), error: None, latency_ms: None, checked_at: None, is_default: default, price_in: None, price_out: None, created_at: at }
    }

    #[test]
    fn provider_pertama_jadi_bawaan_dan_hapus_memindahkannya() {
        let mut s = Store::open_in_memory().unwrap();
        s.save_ai_provider(&prov("a", false, 1)).unwrap();
        s.save_ai_provider(&prov("b", false, 2)).unwrap();
        assert!(s.get_ai_provider("a").unwrap().unwrap().is_default);
        assert!(!s.get_ai_provider("b").unwrap().unwrap().is_default);
        assert!(s.set_default_ai_provider("b").unwrap());
        assert_eq!(s.default_ai_provider().unwrap().unwrap().id, "b");
        assert!(!s.set_default_ai_provider("zzz").unwrap());
        assert!(s.delete_ai_provider("b").unwrap());
        assert_eq!(s.default_ai_provider().unwrap().unwrap().id, "a");
        assert!(s.get_ai_provider("a").unwrap().unwrap().is_default);
        assert!(!s.delete_ai_provider("b").unwrap());
    }

    #[test]
    fn simpan_provider_mempertahankan_waktu_dibuat_dan_status_diperbarui() {
        let mut s = Store::open_in_memory().unwrap();
        s.save_ai_provider(&prov("a", true, 5)).unwrap();
        let mut p = prov("a", true, 999);
        p.name = "baru".into();
        s.save_ai_provider(&p).unwrap();
        let got = s.get_ai_provider("a").unwrap().unwrap();
        assert_eq!((got.name.as_str(), got.created_at), ("baru", 5));
        s.set_ai_provider_status("a", "ok", None, Some(120), 77).unwrap();
        let got = s.get_ai_provider("a").unwrap().unwrap();
        assert_eq!((got.status.as_str(), got.latency_ms, got.checked_at), ("ok", Some(120), Some(77)));
    }

    #[test]
    fn pemakaian_dicatat_dan_dibersihkan_menurut_waktu() {
        let mut s = Store::open_in_memory().unwrap();
        for at in [100, 200, 300] {
            s.add_ai_usage(&UsageRow { at, provider_id: "a".into(), model: "m".into(), account: "x".into(), input_tokens: 10, output_tokens: 5, cost_usd: 0.01, ok: at != 200, latency_ms: 50 }).unwrap();
        }
        assert_eq!(s.ai_usage_since(150).unwrap().len(), 2);
        assert_eq!(s.purge_ai_usage(250).unwrap(), 2);
        assert_eq!(s.ai_usage_since(0).unwrap().len(), 1);
    }

    #[test]
    fn riwayat_chat_terpisah_per_akun_dan_tidak_bisa_ditimpa_akun_lain() {
        let mut s = Store::open_in_memory().unwrap();
        let c = |acc: &str, id: &str, pinned: bool, upd: i64| ChatRow { id: id.into(), account: acc.into(), title: id.into(), pinned, created_at: 1, updated_at: upd, messages: "[]".into() };
        assert!(s.save_ai_chat(&c("u1", "c1", false, 10)).unwrap());
        assert!(s.save_ai_chat(&c("u1", "c2", true, 5)).unwrap());
        assert!(!s.save_ai_chat(&c("u2", "c1", false, 99)).unwrap(), "id milik u1 tidak boleh ditimpa u2");
        assert_eq!(s.get_ai_chat("u1", "c1").unwrap().unwrap().updated_at, 10);
        assert!(s.get_ai_chat("u2", "c1").unwrap().is_none());
        let ids: Vec<String> = s.list_ai_chats("u1").unwrap().into_iter().map(|(m, _)| m.id).collect();
        assert_eq!(ids, ["c2", "c1"], "yang disematkan di atas");
        assert!(s.list_ai_chats("u2").unwrap().is_empty());
        assert!(!s.delete_ai_chat("u2", "c1").unwrap());
        assert_eq!(s.purge_ai_chats(100).unwrap(), 1, "yang disematkan tidak ikut dibersihkan");
        assert_eq!(s.delete_ai_chats_of("u1").unwrap(), 1);
    }
}
