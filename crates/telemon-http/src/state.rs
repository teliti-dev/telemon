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
}

impl<C: Connector> AppState<C> {
    pub fn new(telemon: Arc<Telemon<C>>, auth: Arc<Auth>, cfg: HttpConfig) -> Arc<Self> {
        Self::with_clock(telemon, auth, cfg, Arc::new(system_now))
    }

    pub fn with_clock(telemon: Arc<Telemon<C>>, auth: Arc<Auth>, cfg: HttpConfig, clock: Arc<dyn Fn() -> i64 + Send + Sync>) -> Arc<Self> {
        let limiter = RateLimiter::new(cfg.login_per_minute, 60_000);
        let st = Arc::new(Self { telemon, auth, cfg, limiter, plans: PlanCache::default(), clock, share: Mutex::new((false, false)) });
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

    pub(crate) fn apply_policy(&self, s: &telemon_core::Settings) {
        let (require_2fa, public_on) = *self.share.lock().expect("lock");
        self.auth.set_policy(s.auth_policy(telemon_core::auth::AuthPolicy { require_2fa, public_on, ..Default::default() }));
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
