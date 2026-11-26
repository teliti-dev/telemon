//! Keadaan bersama server HTTP: layanan inti, akun, batas laju, dan rencana tulis yang menunggu penerapan.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use telemon_core::auth::{Auth, Resource};
use telemon_core::{Connector, Plan, Telemon};

#[derive(Clone)]
pub struct HttpConfig {
    /// Folder hasil build UI (`dist/`); dilayani untuk semua jalur selain `/api`.
    pub static_dir: Option<PathBuf>,
    /// Percobaan login (semua langkah) per IP per menit.
    pub login_per_minute: u32,
    /// Selalu beri `Secure` pada cookie (bawaan: hanya untuk akses publik/HTTPS).
    pub force_secure_cookie: bool,
    /// Header Content-Security-Policy untuk halaman UI (bukan JSON).
    pub csp: Option<String>,
    /// Masa berlaku rencana tulis sebelum harus dibuat ulang.
    pub plan_ttl: Duration,
    /// Batas ukuran badan permintaan.
    pub max_body: usize,
    /// Jalur berkas basis data untuk kartu Penyimpanan (hanya ditampilkan).
    pub db_path: String,
    /// **Hanya server demo:** peran berizin Ubah boleh masuk tanpa 2FA. Saklar Wajib 2FA dan akses Publik tetap memaksa.
    pub relax_update_2fa: bool,
    /// Extension (ADR 0028). `None` = mati: tidak ada yang dimuat dan halaman Extension hanya memberi petunjuk.
    pub extensions: Option<ExtConfig>,
    /// Versi Telemon untuk memeriksa kompatibilitas extension (rentang `telemon` di manifest).
    pub host_version: String,
}

/// Pengaturan extension: folder paket dan kunci tepercaya. Diisi aplikasi desktop (aktif bawaan) atau
/// server dari `TELEMON_EXTENSIONS=1`.
#[derive(Clone)]
pub struct ExtConfig {
    pub dir: PathBuf,
    pub keys: telemon_ext::TrustedKeys,
}

/// Kunci penanda tangan official (heksa Ed25519). **Kosong sampai kunci Teliti dibuat** (disimpan
/// seperti kunci updater, ADR 0023); sebelum itu extension official hanya bisa dipakai lewat
/// `TELEMON_EXTENSIONS_OFFICIAL_KEYS` oleh pemilik server.
pub const OFFICIAL_KEYS: &[&str] = &[];

impl ExtConfig {
    /// Baca saklar dari lingkungan. `TELEMON_EXTENSIONS` = `1`/`true` menyalakan, `0`/`false` mematikan;
    /// tanpa nilai, `default_on` (desktop: ya; server dan image: tidak). `TELEMON_EXTENSIONS_DIR`
    /// mengganti folder paket. `TELEMON_EXTENSIONS_OFFICIAL_KEYS` (heksa, pisah koma) menambah kunci
    /// official: hanya pemilik server yang mengatur lingkungan, jadi ini tidak membuka jalan bagi admin web.
    /// `TELEMON_EXTENSIONS_ALLOW_UNSIGNED=1` hanya untuk pengembangan.
    pub fn from_env(default_dir: PathBuf, default_on: bool) -> Option<Self> {
        Self::from_vars(default_dir, default_on, |k| std::env::var(k).ok())
    }

    pub fn from_vars(default_dir: PathBuf, default_on: bool, var: impl Fn(&str) -> Option<String>) -> Option<Self> {
        let on = match var("TELEMON_EXTENSIONS").as_deref().map(str::trim) {
            Some("1") | Some("true") => true,
            Some("0") | Some("false") => false,
            _ => default_on,
        };
        if !on {
            return None;
        }
        let mut official: Vec<String> = OFFICIAL_KEYS.iter().map(|k| k.to_string()).collect();
        if let Some(extra) = var("TELEMON_EXTENSIONS_OFFICIAL_KEYS") {
            official.extend(extra.split(',').map(|k| k.trim().to_ascii_lowercase()).filter(|k| k.len() == 64));
        }
        let allow_unsigned = matches!(var("TELEMON_EXTENSIONS_ALLOW_UNSIGNED").as_deref(), Some("1") | Some("true"));
        let dir = var("TELEMON_EXTENSIONS_DIR").filter(|d| !d.trim().is_empty()).map_or(default_dir, PathBuf::from);
        Some(Self { dir, keys: telemon_ext::TrustedKeys { official, publishers: Vec::new(), allow_unsigned } })
    }
}

impl Default for HttpConfig {
    fn default() -> Self {
        Self {
            static_dir: None,
            login_per_minute: 20,
            force_secure_cookie: false,
            csp: Some("default-src 'self'; img-src 'self' data:; style-src 'self' 'unsafe-inline'; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'self'".into()),
            plan_ttl: Duration::from_secs(600),
            max_body: 64 * 1024,
            db_path: String::new(),
            relax_update_2fa: false,
            extensions: None,
            host_version: env!("CARGO_PKG_VERSION").into(),
        }
    }
}

pub struct AppState<C: Connector> {
    pub telemon: Arc<Telemon<C>>,
    pub auth: Arc<Auth>,
    pub cfg: HttpConfig,
    pub limiter: RateLimiter,
    pub plans: PlanCache,
    pub(crate) clock: Arc<dyn Fn() -> i64 + Send + Sync>,
    /// (wajib 2FA untuk semua, akses publik aktif): berasal dari halaman Berbagi di aplikasi desktop.
    share: Mutex<(bool, bool)>,
    pub(crate) lan: tokio::sync::Mutex<crate::share::Lan>,
    pub(crate) publik: tokio::sync::Mutex<crate::share::Publik>,
    pub(crate) tunnel: Mutex<Option<Arc<dyn crate::share::Tunnel>>>,
    pub(crate) update: crate::update::UpdateState,
    /// Layanan AI (dipasang aplikasi desktop atau server demo); tanpa itu rute `/api/ai` menjawab 501.
    pub(crate) ai: crate::ai::AiSlot,
    /// Batas pertanyaan AI per akun per menit.
    pub(crate) ai_rate: RateLimiter,
    /// Naik tiap kali server LAN dimatikan/diganti: koneksi WebSocket dari server itu ikut ditutup.
    pub(crate) lan_epoch: tokio::sync::watch::Sender<u64>,
    /// Paket extension yang sudah diverifikasi (id → versi dan isi).
    pub(crate) ext_cache: Mutex<HashMap<String, (String, Arc<telemon_ext::Package>)>>,
}

impl<C: Connector> AppState<C> {
    pub fn new(telemon: Arc<Telemon<C>>, auth: Arc<Auth>, cfg: HttpConfig) -> Arc<Self> {
        Self::with_clock(telemon, auth, cfg, Arc::new(system_now))
    }

    pub fn with_clock(telemon: Arc<Telemon<C>>, auth: Arc<Auth>, cfg: HttpConfig, clock: Arc<dyn Fn() -> i64 + Send + Sync>) -> Arc<Self> {
        let limiter = RateLimiter::new(cfg.login_per_minute, 60_000);
        let st = Arc::new(Self { telemon, auth, cfg, limiter, plans: PlanCache::default(), clock, share: Mutex::new((false, false)), lan: tokio::sync::Mutex::new(crate::share::Lan::default()), publik: tokio::sync::Mutex::new(crate::share::Publik::default()), tunnel: Mutex::new(None), update: crate::update::UpdateState::default(), ai: crate::ai::AiSlot::default(), ai_rate: RateLimiter::new(crate::ai::MAX_QUESTIONS_PER_MINUTE, 60_000), lan_epoch: tokio::sync::watch::channel(0).0, ext_cache: Mutex::new(HashMap::new()) });
        if let Ok(Some(v)) = st.telemon.with_store(|s| s.kv_get("share.require_2fa")) {
            st.share.lock().expect("lock").0 = v == "true";
        }
        // Pengaturan tersimpan langsung berlaku (sesi, penguncian, izin tulis publik).
        let _ = st.reload_settings();
        st
    }

    /// Baca pengaturan tersimpan dan terapkan ke kebijakan akun dan pemantau.
    pub fn reload_settings(&self) -> Result<telemon_core::Settings, String> {
        let s = self.telemon.with_store(|st| st.get_settings()).map_err(|e| e.to_string())?;
        self.apply_policy(&s);
        Ok(s)
    }

    pub(crate) fn share_flags(&self) -> (bool, bool) {
        *self.share.lock().expect("lock")
    }

    pub(crate) fn apply_policy(&self, s: &telemon_core::Settings) {
        let (require_2fa, public_on) = *self.share.lock().expect("lock");
        self.auth.set_policy(s.auth_policy(telemon_core::auth::AuthPolicy { require_2fa, public_on, update_roles_need_2fa: !self.cfg.relax_update_2fa, ..Default::default() }));
    }

    /// Status Berbagi dari aplikasi desktop: saklar "Wajib 2FA" dan akses Publik aktif.
    pub fn set_share(&self, require_2fa: bool, public_on: bool) {
        *self.share.lock().expect("lock") = (require_2fa, public_on);
        if let Ok(s) = self.telemon.with_store(|st| st.get_settings()) {
            self.apply_policy(&s);
        }
    }

    pub fn now(&self) -> i64 {
        (self.clock)()
    }
}

pub fn system_now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

/// Jendela tetap per kunci (IP). Cukup untuk meredam tebakan; penguncian akun ada di `Auth`.
pub struct RateLimiter {
    limit: u32,
    window_ms: i64,
    hits: Mutex<HashMap<String, (i64, u32)>>,
}

impl RateLimiter {
    pub fn new(limit: u32, window_ms: i64) -> Self {
        Self { limit, window_ms, hits: Mutex::new(HashMap::new()) }
    }

    /// `Err(detik)` = harus menunggu sekian detik.
    pub fn check(&self, key: &str, now: i64) -> Result<(), u32> {
        let mut h = self.hits.lock().expect("lock");
        if h.len() > 10_000 {
            h.retain(|_, (start, _)| now - *start < self.window_ms);
        }
        let e = h.entry(key.to_string()).or_insert((now, 0));
        if now - e.0 >= self.window_ms {
            *e = (now, 0);
        }
        if e.1 >= self.limit {
            return Err(((e.0 + self.window_ms - now) as f64 / 1000.0).ceil().max(1.0) as u32);
        }
        e.1 += 1;
        Ok(())
    }
}

pub const MAX_PLANS_PER_ACCOUNT: usize = 50;

#[derive(Debug, PartialEq, Eq)]
pub enum PlanError {
    Full,
    Random,
}

pub struct PendingPlan {
    pub plan: Plan,
    pub router_id: String,
    pub account_id: String,
    pub resource: Resource,
    pub expires: i64,
}

/// Rencana tulis disimpan di server: klien hanya memegang id-nya, sehingga langkah yang dikirim ke
/// router tidak bisa diubah klien di antara tinjauan dan penerapan.
#[derive(Default)]
pub struct PlanCache {
    map: Mutex<HashMap<String, PendingPlan>>,
}

impl PlanCache {
    pub fn put(&self, p: PendingPlan, now: i64) -> Result<String, PlanError> {
        let id = telemon_core::generate_key().map_err(|_| PlanError::Random)?.iter().take(16).map(|b| format!("{b:02x}")).collect::<String>();
        let mut m = self.map.lock().expect("lock");
        m.retain(|_, v| v.expires > now);
        // Batas per akun: satu akun tidak boleh menimbun memori dengan rencana yang tak pernah diterapkan.
        if m.values().filter(|v| v.account_id == p.account_id).count() >= MAX_PLANS_PER_ACCOUNT {
            return Err(PlanError::Full);
        }
        m.insert(id.clone(), p);
        Ok(id)
    }

    /// Ambil dan hapus (sekali pakai). Milik akun lain atau kedaluwarsa = `None`.
    pub fn take(&self, id: &str, account_id: &str, now: i64) -> Option<PendingPlan> {
        let mut m = self.map.lock().expect("lock");
        match m.get(id) {
            Some(p) if p.account_id == account_id && p.expires > now => m.remove(id),
            _ => None,
        }
    }
}

#[cfg(test)]
mod ext_env_tests {
    use super::*;

    fn cfg(default_on: bool, vars: &[(&str, &str)]) -> Option<ExtConfig> {
        let v: HashMap<String, String> = vars.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        ExtConfig::from_vars("/data/ext".into(), default_on, |k| v.get(k).cloned())
    }

    #[test]
    fn switch_follows_env_then_default() {
        assert!(cfg(true, &[]).is_some(), "desktop: aktif bawaan");
        assert!(cfg(false, &[]).is_none(), "server dan image: mati bawaan");
        assert!(cfg(false, &[("TELEMON_EXTENSIONS", "1")]).is_some());
        assert!(cfg(true, &[("TELEMON_EXTENSIONS", "0")]).is_none());
        assert!(cfg(false, &[("TELEMON_EXTENSIONS", "ya")]).is_none(), "nilai aneh = bawaan");
    }

    #[test]
    fn keys_dir_and_unsigned_come_from_env() {
        let k = "ab".repeat(32);
        let c = cfg(false, &[("TELEMON_EXTENSIONS", "1"), ("TELEMON_EXTENSIONS_OFFICIAL_KEYS", &format!("{k}, pendek ,{}", "CD".repeat(32))), ("TELEMON_EXTENSIONS_DIR", "/v/ext")]).unwrap();
        assert_eq!(c.dir, PathBuf::from("/v/ext"));
        assert_eq!(c.keys.official, vec![k, "cd".repeat(32)], "kunci rusak dibuang, huruf besar dinormalkan");
        assert!(!c.keys.allow_unsigned);
        assert!(cfg(true, &[("TELEMON_EXTENSIONS_ALLOW_UNSIGNED", "1")]).unwrap().keys.allow_unsigned);
        assert_eq!(cfg(true, &[]).unwrap().dir, PathBuf::from("/data/ext"));
    }
}
