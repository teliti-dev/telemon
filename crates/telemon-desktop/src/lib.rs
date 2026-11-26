//! Peluncur aplikasi desktop (ADR 0021). Membuka penyimpanan SQLite, kunci enkripsi dan kata sandi router
//! dari penyimpanan rahasia (keyring OS di aplikasi), menyalakan pemantau semua router, dan menjalankan
//! **server loopback** yang dilayani ke webview Tauri. Webview memuat UI dari server itu, sehingga UI memakai
//! adapter HTTP yang sama dengan akses browser, tetapi sebagai pemilik (tanpa login) lewat token per peluncuran.
//! Crate ini tidak bergantung pada Tauri sehingga seluruh alurnya teruji di mesin tanpa WebKitGTK.

pub mod cloudflared;
pub mod extstore;
pub mod secrets;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use telemon_core::auth::{Auth, HashParams};
use telemon_core::store::Store;
use telemon_core::{NetConnector, SecretStore, Telemon};
use telemon_http::{AppState, HttpConfig, HttpLlm};

/// Port loopback pilihan: tetap, supaya asal (origin) webview stabil dan pilihan tema/bahasa di localStorage
/// bertahan antar peluncuran. Bila terpakai, port acak.
pub const PREFERRED_PORT: u16 = 47831;

const KEY_NAME: &str = "database-key";

pub struct DesktopConfig {
    /// Berkas basis data (folder induknya dibuat bila belum ada).
    pub db_path: PathBuf,
    /// Folder hasil build UI; kosong = tidak dilayani (mis. dev, UI dari Vite).
    pub static_dir: Option<PathBuf>,
    pub secrets: Arc<dyn SecretStore>,
    /// Nol = pilih port acak.
    pub port: u16,
    pub tunnel: TunnelChoice,
}

/// Penyedia akses Publik.
#[derive(Clone, Default)]
pub enum TunnelChoice {
    /// `cloudflared` resmi, diunduh otomatis ke `<folder data>/bin` saat pertama dibutuhkan (ADR 0022).
    #[default]
    Official,
    /// Tanpa akses Publik.
    Off,
    /// Penyedia lain (uji).
    Custom(Arc<dyn telemon_http::Tunnel>),
}

pub struct Desktop {
    pub state: Arc<AppState<NetConnector>>,
    pub telemon: Arc<Telemon<NetConnector>>,
    /// Token per peluncuran; hanya diketahui webview.
    pub token: String,
    pub addr: SocketAddr,
    stop: tokio::sync::oneshot::Sender<()>,
    server: tokio::task::JoinHandle<()>,
}

#[derive(Debug)]
pub enum StartError {
    Storage(String),
    Secret(String),
    Server(String),
}

impl std::fmt::Display for StartError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StartError::Storage(m) => write!(f, "penyimpanan: {m}"),
            StartError::Secret(m) => write!(f, "penyimpanan rahasia: {m}"),
            StartError::Server(m) => write!(f, "server: {m}"),
        }
    }
}

impl std::error::Error for StartError {}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn parse_key(s: &str) -> Option<[u8; 32]> {
    if s.len() != 64 || !s.is_ascii() {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, b) in out.iter_mut().enumerate() {
        *b = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(out)
}

/// Buka penyimpanan dengan kunci enkripsinya. Kunci baru **hanya disimpan setelah terbukti cocok** dengan
/// basis data: bila basis data sudah ada tetapi kuncinya tidak terbaca (keyring terkunci sesaat, berkas hilang),
/// kunci baru ditolak `unlock` dan kunci lama tidak tertimpa, sehingga bisa dipulihkan.
fn unlock_store(store: &mut Store, secrets: &dyn SecretStore) -> Result<(), StartError> {
    let mismatch = |e: &dyn std::fmt::Display| StartError::Secret(format!("kunci tidak cocok dengan basis data ({e}); pulihkan kunci dari keyring atau cadangan"));
    if let Some(k) = secrets.get(KEY_NAME).as_deref().and_then(parse_key) {
        return store.unlock(&k).map_err(|e| mismatch(&e));
    }
    let key = telemon_core::generate_key().map_err(|e| StartError::Secret(e.to_string()))?;
    store.unlock(&key).map_err(|e| mismatch(&e))?;
    secrets.set(KEY_NAME, &hex(&key)).map_err(StartError::Secret)
}

impl Desktop {
    pub async fn start(cfg: DesktopConfig) -> Result<Self, StartError> {
        if let Some(dir) = cfg.db_path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| StartError::Storage(format!("membuat {}: {e}", dir.display())))?;
        }
        let mut store = Store::open(&cfg.db_path).map_err(|e| StartError::Storage(e.to_string()))?;
        unlock_store(&mut store, cfg.secrets.as_ref())?;
        let settings = store.get_settings().map_err(|e| StartError::Storage(e.to_string()))?;

        let telemon = Arc::new(Telemon::new(store, cfg.secrets.clone(), NetConnector, settings.runtime_config()));
        let auth = Arc::new(Auth::new(telemon.store(), HashParams::default()).map_err(|e| StartError::Storage(e.to_string()))?);
        let ext_dir = cfg.db_path.parent().map_or_else(|| PathBuf::from("extensions"), |d| d.join("extensions"));
        // Extension (ADR 0028): aktif bawaan di desktop; `TELEMON_EXTENSIONS=0` mematikan.
        let http_cfg = HttpConfig { static_dir: cfg.static_dir, db_path: cfg.db_path.display().to_string(), extensions: telemon_http::ExtConfig::from_env(ext_dir, true), ..Default::default() };
        let state = AppState::new(telemon.clone(), auth, http_cfg);
        // AI chat: bawaan mati sampai pemilik menambah provider di Pengaturan (ADR 0007, 0024).
        state.enable_ai(Arc::new(HttpLlm::default()));
        // Toko extension official (ADR 0028): pengunduh HTTPS; aktif hanya bila extension menyala.
        state.set_ext_fetcher(Arc::new(extstore::HttpsFetcher::default()));
        // Worker terjadwal extension (ADR 0028): penjadwal latar; tanpa extension aktif tidak melakukan apa-apa.
        if state.cfg.extensions.is_some() {
            telemon_http::spawn_scheduler(state.clone());
        }

        match &cfg.tunnel {
            TunnelChoice::Official => {
                let bin = cfg.db_path.parent().map_or_else(|| PathBuf::from("bin"), |d| d.join("bin"));
                if let Some(cf) = cloudflared::Cloudflared::official(bin) {
                    state.set_tunnel(Arc::new(cf));
                }
            }
            TunnelChoice::Custom(t) => state.set_tunnel(t.clone()),
            TunnelChoice::Off => {}
        }
        // Pemantau semua router tersimpan dan purge berkala dari pengaturan.
        telemon.start_all().map_err(|e| StartError::Storage(e.to_string()))?;
        telemon.start_purge_from_settings(Duration::from_secs(3600));

        let token = hex(&telemon_core::generate_key().map_err(|e| StartError::Secret(e.to_string()))?);
        let listener = match tokio::net::TcpListener::bind(("127.0.0.1", cfg.port)).await {
            Ok(l) => l,
            Err(_) if cfg.port != 0 => tokio::net::TcpListener::bind(("127.0.0.1", 0)).await.map_err(|e| StartError::Server(e.to_string()))?,
            Err(e) => return Err(StartError::Server(e.to_string())),
        };
        let addr = listener.local_addr().map_err(|e| StartError::Server(e.to_string()))?;
        let (stop, rx) = tokio::sync::oneshot::channel::<()>();
        let app = telemon_http::app_desktop(state.clone(), &token);
        let server = tokio::spawn(async move {
            let _ = axum_serve(listener, app, async {
                let _ = rx.await;
            })
            .await;
        });
        Ok(Self { state, telemon, token, addr, stop, server })
    }

    /// Alamat yang dibuka webview: menukar token dengan cookie lalu memuat UI.
    pub fn entry_url(&self) -> String {
        format!("http://127.0.0.1:{}/desktop-entry?t={}", self.addr.port(), self.token)
    }

    /// Hentikan berbagi, pemantau, dan server. Dipanggil saat aplikasi ditutup.
    pub async fn shutdown(self) {
        self.state.stop_update_checks();
        self.state.stop_public().await;
        self.state.shutdown_lan().await;
        self.telemon.shutdown().await;
        let _ = self.stop.send(());
        let _ = tokio::time::timeout(Duration::from_secs(3), self.server).await;
    }
}

async fn axum_serve(listener: tokio::net::TcpListener, app: telemon_http::Router, shutdown: impl std::future::Future<Output = ()> + Send + 'static) -> std::io::Result<()> {
    telemon_http::serve_router(listener, app, shutdown).await
}
