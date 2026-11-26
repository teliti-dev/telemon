//! Pengaturan aplikasi (retensi, polling, akses jarak jauh, AI). Nilai bawaan dan rentang sama
//! dengan UI (`src/lib/defaults.ts`, ADR 0006). Disimpan sebagai pasangan kunci-nilai.

use std::time::Duration;

use crate::auth::AuthPolicy;
use crate::service::RuntimeConfig;
use crate::store::RetentionPolicy;
use crate::traffic::TrafficConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Retention {
    /// Hari; `None` = selamanya.
    pub event_days: Option<u32>,
    pub ai_chat_days: Option<u32>,
    pub max_events: u32,
    pub deleted_days: Option<u32>,
    pub audit_days: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Polling {
    pub session_seconds: u32,
    pub log_seconds: u32,
    /// Jam; `None` = hanya manual.
    pub auto_import_hours: Option<u32>,
    pub timeout_seconds: u32,
    pub retries: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Remote {
    pub session_hours: u32,
    pub max_failures: u32,
    pub lock_minutes: u32,
    /// Peran berizin Ubah boleh menulis lewat akses Publik. Hanya bisa diubah dari aplikasi desktop.
    pub allow_public_write: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ai {
    pub enabled: bool,
    pub mask_sensitive: bool,
}

/// Pengumpulan bandwidth per antarmuka (ADR 0026).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bandwidth {
    pub enabled: bool,
    pub interval_seconds: u32,
    pub retention_days: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Settings {
    pub retention: Retention,
    pub polling: Polling,
    pub remote: Remote,
    pub ai: Ai,
    pub bandwidth: Bandwidth,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            retention: Retention { event_days: Some(30), ai_chat_days: Some(90), max_events: 200_000, deleted_days: Some(90), audit_days: Some(365) },
            polling: Polling { session_seconds: 30, log_seconds: 30, auto_import_hours: Some(24), timeout_seconds: 10, retries: 2 },
            remote: Remote { session_hours: 12, max_failures: 5, lock_minutes: 5, allow_public_write: false },
            ai: Ai { enabled: true, mask_sensitive: true },
            bandwidth: Bandwidth { enabled: true, interval_seconds: 60, retention_days: 7 },
        }
    }
}

fn range(label: &str, v: u32, min: u32, max: u32) -> Result<(), String> {
    if (min..=max).contains(&v) { Ok(()) } else { Err(format!("{label} harus {min}-{max}")) }
}

fn range_opt(label: &str, v: Option<u32>, min: u32, max: u32) -> Result<(), String> {
    v.map_or(Ok(()), |v| range(label, v, min, max))
}

impl Settings {
    /// Rentang sama dengan validasi UI; pesan siap tampil.
    pub fn validate(&self) -> Result<(), String> {
        let (r, p, m) = (&self.retention, &self.polling, &self.remote);
        range_opt("Simpan event (hari)", r.event_days, 1, 3650)?;
        range_opt("Simpan riwayat chat AI (hari)", r.ai_chat_days, 1, 3650)?;
        range("Maksimum event", r.max_events, 1000, 5_000_000)?;
        range_opt("Hapus permanen item terhapus (hari)", r.deleted_days, 1, 3650)?;
        range_opt("Simpan riwayat perubahan (hari)", r.audit_days, 30, 3650)?;
        range("Interval sesi aktif (detik)", p.session_seconds, 10, 600)?;
        range("Interval log router (detik)", p.log_seconds, 10, 600)?;
        range_opt("Import otomatis (jam)", p.auto_import_hours, 1, 168)?;
        range("Timeout koneksi (detik)", p.timeout_seconds, 3, 60)?;
        range("Percobaan ulang", p.retries, 0, 5)?;
        range("Masa sesi login (jam)", m.session_hours, 1, 720)?;
        range("Batas gagal login", m.max_failures, 3, 20)?;
        range("Lama penguncian (menit)", m.lock_minutes, 1, 1440)?;
        range("Interval bandwidth (detik)", self.bandwidth.interval_seconds, 10, 3600)?;
        range("Simpan data bandwidth (hari)", self.bandwidth.retention_days, 1, 365)?;
        Ok(())
    }

    pub fn retention_policy(&self) -> RetentionPolicy {
        RetentionPolicy { event_days: self.retention.event_days, max_events: self.retention.max_events, deleted_days: self.retention.deleted_days, audit_days: self.retention.audit_days }
    }

    /// Kebijakan akun dari pengaturan; `require_2fa` dan `public_on` berasal dari status Berbagi (`base`).
    pub fn auth_policy(&self, base: AuthPolicy) -> AuthPolicy {
        AuthPolicy {
            session_hours: self.remote.session_hours,
            max_failures: self.remote.max_failures,
            lock_minutes: self.remote.lock_minutes,
            allow_public_write: self.remote.allow_public_write,
            ..base
        }
    }

    /// Polling membaca sesi aktif dan log dalam satu putaran, jadi memakai interval yang lebih pendek.
    pub fn runtime_config(&self) -> RuntimeConfig {
        RuntimeConfig {
            poll_every: Duration::from_secs(u64::from(self.polling.session_seconds.min(self.polling.log_seconds))),
            sync_every: self.polling.auto_import_hours.map(|h| Duration::from_secs(u64::from(h) * 3600)),
            traffic: self.bandwidth.enabled.then(|| TrafficConfig {
                every: Duration::from_secs(u64::from(self.bandwidth.interval_seconds)),
                retention: Duration::from_secs(u64::from(self.bandwidth.retention_days) * 86_400),
            }),
        }
    }

    /// Daftar (kunci, nilai) untuk penyimpanan; `null` = tanpa batas.
    pub fn to_pairs(&self) -> Vec<(&'static str, String)> {
        let o = |v: Option<u32>| v.map_or("null".to_string(), |v| v.to_string());
        let (r, p, m, a, b) = (&self.retention, &self.polling, &self.remote, &self.ai, &self.bandwidth);
        vec![
            ("retention.event_days", o(r.event_days)),
            ("retention.ai_chat_days", o(r.ai_chat_days)),
            ("retention.max_events", r.max_events.to_string()),
            ("retention.deleted_days", o(r.deleted_days)),
            ("retention.audit_days", o(r.audit_days)),
            ("polling.session_seconds", p.session_seconds.to_string()),
            ("polling.log_seconds", p.log_seconds.to_string()),
            ("polling.auto_import_hours", o(p.auto_import_hours)),
            ("polling.timeout_seconds", p.timeout_seconds.to_string()),
            ("polling.retries", p.retries.to_string()),
            ("remote.session_hours", m.session_hours.to_string()),
            ("remote.max_failures", m.max_failures.to_string()),
            ("remote.lock_minutes", m.lock_minutes.to_string()),
            ("remote.allow_public_write", m.allow_public_write.to_string()),
            ("ai.enabled", a.enabled.to_string()),
            ("ai.mask_sensitive", a.mask_sensitive.to_string()),
            ("bandwidth.enabled", b.enabled.to_string()),
            ("bandwidth.interval_seconds", b.interval_seconds.to_string()),
            ("bandwidth.retention_days", b.retention_days.to_string()),
        ]
    }

    /// Kolom yang tidak ada atau rusak memakai nilai bawaan.
    pub fn from_pairs<'a>(pairs: impl IntoIterator<Item = (&'a str, &'a str)>) -> Self {
        let mut s = Settings::default();
        let num = |v: &str| v.parse::<u32>().ok();
        let opt = |v: &str| if v == "null" { Some(None) } else { v.parse::<u32>().ok().map(Some) };
        for (k, v) in pairs {
            match k {
                "retention.event_days" => opt(v).into_iter().for_each(|x| s.retention.event_days = x),
                "retention.ai_chat_days" => opt(v).into_iter().for_each(|x| s.retention.ai_chat_days = x),
                "retention.max_events" => num(v).into_iter().for_each(|x| s.retention.max_events = x),
                "retention.deleted_days" => opt(v).into_iter().for_each(|x| s.retention.deleted_days = x),
                "retention.audit_days" => opt(v).into_iter().for_each(|x| s.retention.audit_days = x),
                "polling.session_seconds" => num(v).into_iter().for_each(|x| s.polling.session_seconds = x),
                "polling.log_seconds" => num(v).into_iter().for_each(|x| s.polling.log_seconds = x),
                "polling.auto_import_hours" => opt(v).into_iter().for_each(|x| s.polling.auto_import_hours = x),
                "polling.timeout_seconds" => num(v).into_iter().for_each(|x| s.polling.timeout_seconds = x),
                "polling.retries" => num(v).into_iter().for_each(|x| s.polling.retries = x),
                "remote.session_hours" => num(v).into_iter().for_each(|x| s.remote.session_hours = x),
                "remote.max_failures" => num(v).into_iter().for_each(|x| s.remote.max_failures = x),
                "remote.lock_minutes" => num(v).into_iter().for_each(|x| s.remote.lock_minutes = x),
                "remote.allow_public_write" => s.remote.allow_public_write = v == "true",
                "ai.enabled" => s.ai.enabled = v == "true",
                "ai.mask_sensitive" => s.ai.mask_sensitive = v == "true",
                "bandwidth.enabled" => s.bandwidth.enabled = v == "true",
                "bandwidth.interval_seconds" => num(v).into_iter().for_each(|x| s.bandwidth.interval_seconds = x),
                "bandwidth.retention_days" => num(v).into_iter().for_each(|x| s.bandwidth.retention_days = x),
                _ => {}
            }
        }
        s
    }

    /// Perbedaan terhadap `other` sebagai (kunci, dari, ke), untuk riwayat perubahan.
    pub fn diff(&self, other: &Settings) -> Vec<(&'static str, String, String)> {
        self.to_pairs().into_iter().zip(other.to_pairs()).filter(|((_, a), (_, b))| a != b).map(|((k, a), (_, b))| (k, a, b)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Store;

    #[test]
    fn bawaan_sama_dengan_ui_dan_valid() {
        let s = Settings::default();
        assert!(s.validate().is_ok());
        assert_eq!((s.retention.event_days, s.retention.max_events, s.polling.session_seconds, s.remote.max_failures), (Some(30), 200_000, 30, 5));
        assert_eq!(Settings::from_pairs(s.to_pairs().iter().map(|(k, v)| (*k, v.as_str()))), s);
    }

    #[test]
    fn rentang_validasi() {
        let mut s = Settings::default();
        let bad = |f: &dyn Fn(&mut Settings)| {
            let mut x = Settings::default();
            f(&mut x);
            x.validate().expect_err("harusnya ditolak")
        };
        assert!(bad(&|x| x.retention.event_days = Some(0)).contains("Simpan event"));
        assert!(bad(&|x| x.retention.max_events = 10).contains("Maksimum event"));
        assert!(bad(&|x| x.retention.audit_days = Some(7)).contains("30-3650"));
        assert!(bad(&|x| x.polling.session_seconds = 5).contains("10-600"));
        assert!(bad(&|x| x.polling.retries = 6).contains("0-5"));
        assert!(bad(&|x| x.remote.max_failures = 2).contains("3-20"));
        assert!(bad(&|x| x.remote.lock_minutes = 0).contains("1-1440"));
        assert!(bad(&|x| x.bandwidth.interval_seconds = 5).contains("10-3600"));
        assert!(bad(&|x| x.bandwidth.retention_days = 0).contains("1-365"));
        // Selamanya (None) valid untuk kolom yang membolehkannya.
        s.retention.event_days = None;
        s.polling.auto_import_hours = None;
        assert!(s.validate().is_ok());
    }

    #[test]
    fn simpan_baca_dan_kolom_rusak_kembali_ke_bawaan() {
        let mut st = Store::open_in_memory().unwrap();
        assert_eq!(st.get_settings().unwrap(), Settings::default());
        let mut s = Settings::default();
        s.retention.event_days = None;
        s.polling.session_seconds = 45;
        s.remote.allow_public_write = true;
        s.ai.enabled = false;
        st.save_settings(&s).unwrap();
        assert_eq!(st.get_settings().unwrap(), s);
        st.conn.execute("UPDATE settings SET v = 'abc' WHERE k = 'polling.session_seconds'", []).unwrap();
        assert_eq!(st.get_settings().unwrap().polling.session_seconds, 30);
        assert_eq!(Settings::from_pairs([("tidak.dikenal", "1")]), Settings::default());
    }

    #[test]
    fn turunan_kebijakan_dan_diff() {
        let mut s = Settings::default();
        s.remote.max_failures = 7;
        s.polling.session_seconds = 60;
        s.polling.log_seconds = 20;
        s.polling.auto_import_hours = None;
        let base = AuthPolicy { require_2fa: true, public_on: true, ..Default::default() };
        let p = s.auth_policy(base);
        assert_eq!((p.max_failures, p.require_2fa, p.public_on, p.session_hours), (7, true, true, 12));
        let rc = s.runtime_config();
        assert_eq!((rc.poll_every, rc.sync_every), (Duration::from_secs(20), None));
        // Bandwidth: bawaan aktif tiap 60 detik, simpan 7 hari; dimatikan = tanpa pengumpul.
        assert_eq!(rc.traffic, Some(TrafficConfig { every: Duration::from_secs(60), retention: Duration::from_secs(7 * 86_400) }));
        let mut off = s;
        off.bandwidth.enabled = false;
        assert_eq!(off.runtime_config().traffic, None);
        assert_eq!(s.retention_policy().max_events, 200_000);
        let d = Settings::default().diff(&s);
        let keys: Vec<&str> = d.iter().map(|(k, ..)| *k).collect();
        assert_eq!(keys, vec!["polling.session_seconds", "polling.log_seconds", "polling.auto_import_hours", "remote.max_failures"]);
        assert_eq!(d[2], ("polling.auto_import_hours", "24".into(), "null".into()));
    }
}
