//! Lapisan service: satu pintu untuk aplikasi (Tauri/HTTP). Menggabungkan penyimpanan, kredensial
//! (keyring lewat [`SecretStore`]), sambungan router yang menyambung ulang sendiri, pemantauan,
//! penulisan dengan tinjauan + riwayat, dan retensi. Semua yang bisa diuji tanpa router nyata ada di sini.

use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokio::sync::{broadcast, mpsc, watch};
use tokio::task::JoinHandle;

use crate::events::Event;
use crate::monitor::{MonitorConfig, SharedStore, run_purge, run_router, run_traffic, sync_router};
use crate::traffic::TrafficConfig;
use crate::router::{Error, MikrotikClient, Row, RouterApi, TlsMode};
use crate::store::{NewRouter, PasswordError, PurgeCounts, RetentionPolicy, RouterError, RouterPatch, RouterRecord, Store};
use crate::write::{self, NewProfile, NewUser, Plan, ProfilePatch, UserPatch};

fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

// ---------------- kredensial ----------------

/// Penyimpanan kata sandi API router. Di aplikasi = keyring OS; di uji = memori.
pub trait SecretStore: Send + Sync {
    fn get(&self, key: &str) -> Option<String>;
    fn set(&self, key: &str, value: &str) -> Result<(), String>;
    fn delete(&self, key: &str);
}

#[derive(Default)]
pub struct MemorySecrets(Mutex<HashMap<String, String>>);

impl SecretStore for MemorySecrets {
    fn get(&self, key: &str) -> Option<String> {
        self.0.lock().expect("lock").get(key).cloned()
    }
    fn set(&self, key: &str, value: &str) -> Result<(), String> {
        self.0.lock().expect("lock").insert(key.into(), value.into());
        Ok(())
    }
    fn delete(&self, key: &str) {
        self.0.lock().expect("lock").remove(key);
    }
}

fn secret_key(router_id: &str) -> String {
    format!("router:{router_id}")
}

// ---------------- sambungan ----------------

/// Pembuat sambungan ke router. Nyata: [`NetConnector`]; uji: tiruan.
pub trait Connector: Send + Sync + 'static {
    type Api: RouterApi + Clone + Send + 'static;
    fn connect(&self, addr: &str, username: &str, password: &str, mode: TlsMode) -> impl Future<Output = Result<Self::Api, Error>> + Send;
}

pub struct NetConnector;

impl Connector for NetConnector {
    type Api = MikrotikClient;
    async fn connect(&self, addr: &str, username: &str, password: &str, mode: TlsMode) -> Result<MikrotikClient, Error> {
        MikrotikClient::connect_with(addr, username, password, mode).await
    }
}

/// Sambungan yang dibuat saat dibutuhkan dan dibuang bila putus, sehingga polling berikutnya
/// otomatis menyambung ulang (mikrotik-rs tidak menyambung ulang sendiri).
pub struct Resilient<C: Connector> {
    conn: Arc<C>,
    addr: String,
    username: String,
    password: String,
    mode: TlsMode,
    cur: tokio::sync::Mutex<Option<C::Api>>,
}

impl<C: Connector> Resilient<C> {
    async fn api(&self) -> Result<C::Api, Error> {
        let mut g = self.cur.lock().await;
        if let Some(a) = &*g {
            return Ok(a.clone());
        }
        let a = self.conn.connect(&self.addr, &self.username, &self.password, self.mode).await?;
        *g = Some(a.clone());
        Ok(a)
    }

    async fn after<T>(&self, r: Result<T, Error>) -> Result<T, Error> {
        if matches!(r, Err(Error::Fatal(_) | Error::Connect(_))) {
            *self.cur.lock().await = None;
        }
        r
    }
}

impl<C: Connector> RouterApi for Resilient<C> {
    async fn print(&self, path: &str) -> Result<Vec<Row>, Error> {
        let a = self.api().await?;
        self.after(a.print(path).await).await
    }
    async fn find(&self, path: &str, key: &str, value: &str) -> Result<Vec<Row>, Error> {
        let a = self.api().await?;
        self.after(a.find(path, key, value).await).await
    }
    async fn run(&self, cmd: &str, attrs: &[(String, String)]) -> Result<Vec<Row>, Error> {
        let a = self.api().await?;
        self.after(a.run(cmd, attrs).await).await
    }
}

// ---------------- galat dan tipe ----------------

/// Kredensial SSH satu router (lihat `Telemon::ssh_credentials`).
pub struct SshCredentials {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    /// Sidik jari kunci host yang disematkan; `None` = belum disematkan.
    pub host_key: Option<String>,
}

impl std::fmt::Debug for SshCredentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SshCredentials").field("host", &self.host).field("port", &self.port).field("username", &self.username).field("password", &"***").field("host_key", &self.host_key).finish()
    }
}

#[derive(Debug)]
pub enum ServiceError {
    Router(Error),
    Config(RouterError),
    Password(PasswordError),
    Db(rusqlite::Error),
    /// Kredensial router tidak ada di penyimpanan rahasia atau gagal disimpan.
    Secret(String),
    NotFound,
    /// Ditolak sebelum menyentuh router (mis. router tidak diizinkan ditulis).
    Rejected(String),
}

impl std::fmt::Display for ServiceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ServiceError::Router(e) => write!(f, "{e}"),
            ServiceError::Config(e) => write!(f, "{e}"),
            ServiceError::Password(e) => write!(f, "{e}"),
            ServiceError::Db(e) => write!(f, "basis data: {e}"),
            ServiceError::Secret(m) => write!(f, "kredensial: {m}"),
            ServiceError::NotFound => f.write_str("router tidak ditemukan"),
            ServiceError::Rejected(m) => write!(f, "ditolak: {m}"),
        }
    }
}

impl std::error::Error for ServiceError {}

macro_rules! from_err {
    ($t:ty, $v:ident) => {
        impl From<$t> for ServiceError {
            fn from(e: $t) -> Self {
                ServiceError::$v(e)
            }
        }
    };
}
from_err!(Error, Router);
from_err!(RouterError, Config);
from_err!(PasswordError, Password);
from_err!(rusqlite::Error, Db);

type Res<T> = Result<T, ServiceError>;

/// Hasil uji sambungan: identitas router dan hak akun API (untuk peringatan di UI).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Probe {
    pub model: String,
    pub version: String,
    /// Akun punya hak `write`: boleh mengaktifkan "izinkan edit".
    pub can_write: bool,
    /// Akun punya hak `sensitive`/`full`: password secret ikut terbaca.
    pub can_read_passwords: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct RuntimeConfig {
    pub poll_every: Duration,
    pub sync_every: Option<Duration>,
    /// `None` = pengumpulan bandwidth mati.
    pub traffic: Option<TrafficConfig>,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self { poll_every: Duration::from_secs(30), sync_every: Some(Duration::from_secs(3600)), traffic: None }
    }
}

/// Event dari satu router, untuk disalurkan ke UI.
#[derive(Debug, Clone)]
pub struct RouterEvent {
    /// Id baris event di basis data (sama dengan id di `list_events`).
    pub id: i64,
    pub router_id: String,
    pub event: Event,
}

struct Monitor {
    stop: watch::Sender<bool>,
    tasks: Vec<JoinHandle<()>>,
}

pub struct Telemon<C: Connector> {
    store: SharedStore,
    secrets: Arc<dyn SecretStore>,
    conn: Arc<C>,
    clients: Mutex<HashMap<String, Arc<Resilient<C>>>>,
    monitors: Mutex<HashMap<String, Monitor>>,
    purge: Mutex<Option<Monitor>>,
    events: broadcast::Sender<RouterEvent>,
    cfg: Mutex<RuntimeConfig>,
}

fn policy_check(rows: &[Row], key: &str) -> bool {
    rows.iter().any(|r| r.get("policy").is_some_and(|p| p.split(',').any(|x| x.trim() == key)))
}

impl<C: Connector> Telemon<C> {
    /// `store` harus sudah `unlock` bila password pelanggan ingin disimpan.
    pub fn new(store: Store, secrets: Arc<dyn SecretStore>, conn: C, cfg: RuntimeConfig) -> Self {
        Self {
            store: Arc::new(Mutex::new(store)),
            secrets,
            conn: Arc::new(conn),
            clients: Mutex::new(HashMap::new()),
            monitors: Mutex::new(HashMap::new()),
            purge: Mutex::new(None),
            events: broadcast::channel(256).0,
            cfg: Mutex::new(cfg),
        }
    }

    /// Penyimpanan rahasia (kunci API provider AI memakainya; kuncinya tidak pernah masuk basis data).
    pub fn secrets(&self) -> &Arc<dyn SecretStore> {
        &self.secrets
    }

    pub fn store(&self) -> SharedStore {
        self.store.clone()
    }

    /// Jalankan fungsi pada Store (untuk pembacaan: daftar user, event, riwayat, dst).
    pub fn with_store<R>(&self, f: impl FnOnce(&mut Store) -> R) -> R {
        f(&mut self.store.lock().expect("store lock"))
    }

    pub fn subscribe(&self) -> broadcast::Receiver<RouterEvent> {
        self.events.subscribe()
    }

    pub fn set_config(&self, cfg: RuntimeConfig) {
        *self.cfg.lock().expect("lock") = cfg;
    }

    // ---------------- router ----------------

    /// Tambah router; kata sandi API masuk ke penyimpanan rahasia, bukan SQLite.
    pub fn add_router(&self, n: &NewRouter, password: &str) -> Res<RouterRecord> {
        if password.is_empty() {
            return Err(RouterError::Invalid("kata sandi API wajib diisi".into()).into());
        }
        let rec = self.with_store(|s| s.add_router(n, now_ms()))?;
        if let Err(m) = self.secrets.set(&secret_key(&rec.id), password) {
            let _ = self.with_store(|s| s.remove_router(&rec.id));
            return Err(ServiceError::Secret(m));
        }
        Ok(rec)
    }

    /// Ubah router. Bila koneksi berubah (alamat, port, pin, username, kata sandi) sambungan lama
    /// dibuang dan pemantau yang berjalan dimulai ulang.
    pub fn update_router(&self, id: &str, patch: &RouterPatch, new_password: Option<&str>) -> Res<RouterRecord> {
        let before = self.with_store(|s| s.get_router(id))?.ok_or(ServiceError::NotFound)?;
        let rec = self.with_store(|s| s.update_router(id, patch))?;
        if let Some(pw) = new_password.filter(|p| !p.is_empty()) {
            self.secrets.set(&secret_key(id), pw).map_err(ServiceError::Secret)?;
        }
        let changed = rec.host != before.host || rec.port != before.port || rec.tls_pin != before.tls_pin || rec.username != before.username || new_password.is_some_and(|p| !p.is_empty());
        if changed {
            self.clients.lock().expect("lock").remove(id);
            if self.monitors.lock().expect("lock").contains_key(id) {
                self.stop_monitor(id);
                self.start_monitor(id)?;
            }
        }
        Ok(rec)
    }

    pub fn remove_router(&self, id: &str) -> Res<crate::store::RouterRemoved> {
        self.stop_monitor(id);
        self.clients.lock().expect("lock").remove(id);
        let out = self.with_store(|s| s.remove_router(id))?;
        self.secrets.delete(&secret_key(id));
        Ok(out)
    }

    /// Uji sambungan sebelum disimpan: terhubung, baca identitas, dan periksa hak akun API.
    pub async fn probe(&self, addr: &str, username: &str, password: &str, mode: TlsMode) -> Res<Probe> {
        let api = self.conn.connect(addr, username, password, mode).await?;
        let mut p = Probe::default();
        if let Some(r) = api.print("/system/resource").await?.first() {
            p.model = r.get("board-name").cloned().unwrap_or_default();
            p.version = r.get("version").cloned().unwrap_or_default();
        }
        // Hak diturunkan dari grup akun; bila tidak terbaca (mis. akun tanpa hak) dianggap tidak ada.
        let group = api.find("/user", "name", username).await.ok().and_then(|u| u.into_iter().next()).and_then(|u| u.get("group").cloned());
        if let Some(g) = group {
            let rows = api.find("/user/group", "name", &g).await.unwrap_or_default();
            p.can_write = policy_check(&rows, "write") || policy_check(&rows, "full");
            p.can_read_passwords = policy_check(&rows, "sensitive") || policy_check(&rows, "full");
        }
        Ok(p)
    }

    /// Sidik jari sertifikat api-ssl untuk dikonfirmasi pengguna (percaya-saat-pertama).
    #[cfg(feature = "tls")]
    pub async fn fetch_fingerprint(&self, addr: &str) -> Res<crate::tls::Fingerprint> {
        Ok(crate::tls::fetch_fingerprint(addr, Duration::from_secs(10)).await?)
    }

    /// Alamat, akun, dan kata sandi router untuk SSH (akun yang sama dengan API; ADR 0029). Hanya untuk router yang boleh
    /// ditulis (`can_write`): SSH selalu memberi hak penuh akun itu. Kata sandi tidak boleh masuk log.
    pub fn ssh_credentials(&self, router_id: &str) -> Res<SshCredentials> {
        let rec = self.with_store(|s| s.get_router(router_id))?.ok_or(ServiceError::NotFound)?;
        if !rec.can_write {
            return Err(ServiceError::Rejected("Router ini tidak diizinkan ditulis dari Telemon, jadi terminal SSH dimatikan".into()));
        }
        let (port, pin) = self.with_store(|s| s.router_ssh_get(router_id))?;
        let password = self.secrets.get(&secret_key(router_id)).ok_or_else(|| ServiceError::Secret("kata sandi router tidak ditemukan".into()))?;
        Ok(SshCredentials { host: rec.host, port, username: rec.username, password, host_key: pin })
    }

    /// Sambungan router yang dipakai bersama pemantau dan penulisan.
    pub fn client(&self, router_id: &str) -> Res<Arc<Resilient<C>>> {
        if let Some(c) = self.clients.lock().expect("lock").get(router_id) {
            return Ok(c.clone());
        }
        let rec = self.with_store(|s| s.get_router(router_id))?.ok_or(ServiceError::NotFound)?;
        let password = self.secrets.get(&secret_key(router_id)).ok_or_else(|| ServiceError::Secret("kata sandi router tidak ditemukan".into()))?;
        let mode = rec.tls_mode();
        let c = Arc::new(Resilient { conn: self.conn.clone(), addr: rec.addr(), username: rec.username.clone(), password, mode, cur: tokio::sync::Mutex::new(None) });
        self.clients.lock().expect("lock").insert(router_id.to_string(), c.clone());
        Ok(c)
    }

    // ---------------- pemantauan ----------------

    pub fn start_monitor(&self, router_id: &str) -> Res<()> {
        if self.monitors.lock().expect("lock").contains_key(router_id) {
            return Ok(());
        }
        let api = self.client(router_id)?;
        let cfg = *self.cfg.lock().expect("lock");
        let traffic_api = api.clone();
        let (stop_tx, stop_rx) = watch::channel(false);
        let (tx, mut rx) = mpsc::channel::<(i64, Event)>(64);
        let events = self.events.clone();
        let id = router_id.to_string();
        let forward = tokio::spawn({
            let id = id.clone();
            async move {
                while let Some((eid, event)) = rx.recv().await {
                    let _ = events.send(RouterEvent { id: eid, router_id: id.clone(), event });
                }
            }
        });
        let traffic_task = cfg.traffic.map(|t| tokio::spawn(run_traffic(ArcApi(traffic_api), self.store.clone(), id.clone(), t, stop_rx.clone())));
        let run = tokio::spawn(run_router_arc(api, self.store.clone(), id, MonitorConfig { poll_every: cfg.poll_every, sync_every: cfg.sync_every }, tx, stop_rx));
        let mut tasks = vec![run, forward];
        tasks.extend(traffic_task);
        self.monitors.lock().expect("lock").insert(router_id.to_string(), Monitor { stop: stop_tx, tasks });
        Ok(())
    }

    pub fn stop_monitor(&self, router_id: &str) {
        if let Some(m) = self.monitors.lock().expect("lock").remove(router_id) {
            let _ = m.stop.send(true);
            // Tugas berhenti sendiri setelah sinyal; tidak ditunggu agar pemanggil sinkron tidak menggantung.
            drop(m.tasks);
        }
    }

    pub fn start_all(&self) -> Res<usize> {
        let ids: Vec<String> = self.with_store(|s| s.list_routers())?.into_iter().map(|r| r.id).collect();
        let mut n = 0;
        for id in ids {
            if self.start_monitor(&id).is_ok() {
                n += 1;
            }
        }
        Ok(n)
    }

    /// Hentikan semua pemantau dan tugas purge, lalu tunggu selesai.
    pub async fn shutdown(&self) {
        let monitors: Vec<Monitor> = self.monitors.lock().expect("lock").drain().map(|(_, m)| m).collect();
        let purge = self.purge.lock().expect("lock").take();
        for m in monitors.into_iter().chain(purge) {
            let _ = m.stop.send(true);
            for t in m.tasks {
                let _ = t.await;
            }
        }
    }

    /// Jalankan purge retensi berkala; kebijakan dibaca ulang tiap putaran lewat `policy`.
    pub fn start_purge(&self, every: Duration, policy: impl Fn() -> RetentionPolicy + Send + 'static) {
        let mut slot = self.purge.lock().expect("lock");
        if slot.is_some() {
            return;
        }
        let (stop_tx, stop_rx) = watch::channel(false);
        let task = tokio::spawn(run_purge(self.store.clone(), policy, every, stop_rx));
        *slot = Some(Monitor { stop: stop_tx, tasks: vec![task] });
    }

    /// Terapkan pengaturan: interval polling untuk pemantau berikutnya dan kebijakan retensi.
    /// Pemantau yang sedang berjalan dimulai ulang agar memakai interval baru.
    pub fn apply_settings(&self, s: &crate::settings::Settings) -> Res<()> {
        self.set_config(s.runtime_config());
        let running: Vec<String> = self.monitors.lock().expect("lock").keys().cloned().collect();
        for id in running {
            self.stop_monitor(&id);
            let _ = self.start_monitor(&id);
        }
        Ok(())
    }

    /// Purge berkala yang membaca kebijakan retensi dari pengaturan tersimpan tiap putaran.
    pub fn start_purge_from_settings(&self, every: Duration) {
        let store = self.store.clone();
        self.start_purge(every, move || store.lock().expect("store lock").get_settings().map(|s| s.retention_policy()).unwrap_or_else(|_| crate::settings::Settings::default().retention_policy()));
    }

    pub fn purge_now(&self, policy: &RetentionPolicy) -> Res<PurgeCounts> {
        Ok(self.with_store(|s| s.purge(now_ms(), policy))?)
    }

    pub fn purge_preview(&self, policy: &RetentionPolicy) -> Res<PurgeCounts> {
        Ok(self.with_store(|s| s.purge_preview(now_ms(), policy))?)
    }

    /// Impor manual: timpa salinan lokal dengan data router sekarang.
    pub async fn import_now(&self, router_id: &str) -> Res<(crate::store::SyncReport, crate::store::SyncReport)> {
        let api = self.client(router_id)?;
        Ok(sync_router(&*api, &self.store, router_id).await?)
    }

    // ---------------- penulisan (rencana -> tinjau -> terapkan) ----------------

    pub async fn plan_add_user(&self, router_id: &str, u: &NewUser) -> Res<Plan> {
        Ok(write::plan_add_user(&*self.client(router_id)?, u).await?)
    }
    pub async fn plan_edit_user(&self, router_id: &str, name: &str, p: &UserPatch) -> Res<Plan> {
        Ok(write::plan_edit_user(&*self.client(router_id)?, name, p).await?)
    }
    pub async fn plan_soft_delete_user(&self, router_id: &str, name: &str) -> Res<Plan> {
        Ok(write::plan_soft_delete_user(&*self.client(router_id)?, name).await?)
    }
    pub async fn plan_restore_user(&self, router_id: &str, name: &str) -> Res<Plan> {
        Ok(write::plan_restore_user(&*self.client(router_id)?, name).await?)
    }
    pub async fn plan_add_profile(&self, router_id: &str, p: &NewProfile) -> Res<Plan> {
        Ok(write::plan_add_profile(&*self.client(router_id)?, p).await?)
    }
    pub async fn plan_edit_profile(&self, router_id: &str, name: &str, p: &ProfilePatch) -> Res<Plan> {
        Ok(write::plan_edit_profile(&*self.client(router_id)?, name, p).await?)
    }
    pub async fn check_profile_deletable(&self, router_id: &str, name: &str) -> Res<()> {
        Ok(write::check_profile_deletable(&*self.client(router_id)?, name).await?)
    }

    /// Terapkan rencana yang sudah ditinjau. Hanya untuk router `can_write`. Setelah berhasil:
    /// tanda hapus/pulihkan lokal mengikuti, lalu salinan lokal disinkronkan ulang dengan router.
    /// Wewenang akun/peran ditegakkan pemanggil sebelum memanggil ini.
    pub async fn apply(&self, router_id: &str, actor: &str, plan: &Plan) -> Res<usize> {
        let rec = self.with_store(|s| s.get_router(router_id))?.ok_or(ServiceError::NotFound)?;
        if !rec.can_write {
            return Err(ServiceError::Rejected(format!("router {} tidak diizinkan ditulis dari Telemon", rec.name)));
        }
        let api = self.client(router_id)?;
        let now = now_ms();
        let done = write::apply_audited(&*api, &self.store, router_id, actor, now, plan).await.map_err(|(_, e)| ServiceError::Router(e))?;
        let uid = format!("{router_id}:{}", plan.target);
        match plan.action.as_str() {
            "user.soft_delete" => {
                let _ = self.with_store(|s| s.soft_delete_user(&uid, now));
            }
            "user.restore" => {
                let _ = self.with_store(|s| s.restore_user(&uid));
            }
            _ => {}
        }
        // Salinan lokal mengikuti router; kegagalan sinkron tidak membatalkan penulisan yang sudah berhasil.
        let _ = sync_router(&*api, &self.store, router_id).await;
        Ok(done)
    }
}

/// `run_router` menerima `impl RouterApi` bernilai; bungkus `Arc` agar dapat dipakai bersama.
struct ArcApi<A>(Arc<A>);

impl<A: RouterApi + Send + Sync> RouterApi for ArcApi<A> {
    fn print(&self, path: &str) -> impl Future<Output = Result<Vec<Row>, Error>> + Send {
        self.0.print(path)
    }
    fn find(&self, path: &str, key: &str, value: &str) -> impl Future<Output = Result<Vec<Row>, Error>> + Send {
        self.0.find(path, key, value)
    }
    fn run(&self, cmd: &str, attrs: &[(String, String)]) -> impl Future<Output = Result<Vec<Row>, Error>> + Send {
        self.0.run(cmd, attrs)
    }
}

async fn run_router_arc<A: RouterApi + Send + Sync + 'static>(api: Arc<A>, store: SharedStore, id: String, cfg: MonitorConfig, tx: mpsc::Sender<(i64, Event)>, stop: watch::Receiver<bool>) {
    run_router(ArcApi(api), store, id, cfg, Some(tx), stop).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::EventKind;
    use crate::poller::UserStatus;
    use crate::store::RouterStatus;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    fn row(kv: &[(&str, &str)]) -> Row {
        kv.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    /// Keadaan router tiruan yang dipakai bersama semua sambungan.
    struct World {
        secrets: Mutex<Vec<Row>>,
        profiles: Vec<Row>,
        active: Mutex<Vec<Row>>,
        group_policy: Mutex<String>,
    }

    #[derive(Clone)]
    struct FakeApi {
        world: Arc<World>,
        alive: Arc<AtomicBool>,
    }

    impl FakeApi {
        fn check(&self) -> Result<(), Error> {
            if self.alive.load(Ordering::SeqCst) { Ok(()) } else { Err(Error::Fatal("koneksi putus".into())) }
        }
    }

    impl RouterApi for FakeApi {
        async fn print(&self, path: &str) -> Result<Vec<Row>, Error> {
            self.check()?;
            let w = &self.world;
            Ok(match path {
                "/ppp/secret" => w.secrets.lock().unwrap().clone(),
                "/ppp/profile" => w.profiles.clone(),
                "/ppp/active" => w.active.lock().unwrap().clone(),
                "/system/resource" => vec![row(&[("board-name", "RB4011"), ("version", "7.15 (stable)")])],
                "/user" => vec![row(&[("name", "api"), ("group", "g1")])],
                "/user/group" => vec![row(&[("name", "g1"), ("policy", &w.group_policy.lock().unwrap())])],
                _ => vec![],
            })
        }

        async fn run(&self, cmd: &str, attrs: &[(String, String)]) -> Result<Vec<Row>, Error> {
            self.check()?;
            let get = |k: &str| attrs.iter().find(|(a, _)| a == k).map(|(_, v)| v.clone());
            let w = &self.world;
            match cmd {
                "/ppp/secret/add" => {
                    let mut r: Row = attrs.iter().cloned().collect();
                    r.insert(".id".into(), format!("*n{}", w.secrets.lock().unwrap().len() + 10));
                    r.entry("disabled".into()).or_insert_with(|| "false".into());
                    w.secrets.lock().unwrap().push(r);
                }
                "/ppp/secret/set" => {
                    let id = get(".id").unwrap();
                    let mut s = w.secrets.lock().unwrap();
                    let r = s.iter_mut().find(|r| r[".id"] == id).ok_or_else(|| Error::Trap("no such item".into()))?;
                    for (k, v) in attrs.iter().filter(|(k, _)| k != ".id") {
                        r.insert(k.clone(), if k == "disabled" { (v == "yes").to_string() } else { v.clone() });
                    }
                }
                "/ppp/active/remove" => w.active.lock().unwrap().retain(|r| r[".id"] != get(".id").unwrap()),
                _ => return Err(Error::Trap("unknown command".into())),
            }
            Ok(vec![])
        }
    }

    #[derive(Default)]
    struct FakeConnector {
        world: Option<Arc<World>>,
        connects: AtomicUsize,
        /// Jumlah percobaan sambung pertama yang harus gagal.
        fail_first: AtomicUsize,
        last_alive: Mutex<Option<Arc<AtomicBool>>>,
        last_creds: Mutex<(String, String, String)>,
    }

    impl Connector for FakeConnector {
        type Api = FakeApi;
        async fn connect(&self, addr: &str, user: &str, pass: &str, _mode: TlsMode) -> Result<FakeApi, Error> {
            self.connects.fetch_add(1, Ordering::SeqCst);
            *self.last_creds.lock().unwrap() = (addr.into(), user.into(), pass.into());
            if self.fail_first.load(Ordering::SeqCst) > 0 {
                self.fail_first.fetch_sub(1, Ordering::SeqCst);
                return Err(Error::Connect("tidak terjangkau".into()));
            }
            let alive = Arc::new(AtomicBool::new(true));
            *self.last_alive.lock().unwrap() = Some(alive.clone());
            Ok(FakeApi { world: self.world.clone().unwrap(), alive })
        }
    }

    fn world() -> Arc<World> {
        Arc::new(World {
            secrets: Mutex::new(vec![row(&[(".id", "*1"), ("name", "budi"), ("profile", "10M"), ("disabled", "false"), ("password", "rahasia123")])]),
            profiles: vec![row(&[(".id", "*p1"), ("name", "10M"), ("rate-limit", "10M/10M")]), row(&[(".id", "*p2"), ("name", "20M"), ("rate-limit", "20M/20M")])],
            active: Mutex::new(vec![row(&[(".id", "*a1"), ("name", "budi"), ("service", "pppoe"), ("uptime", "5m")])]),
            group_policy: Mutex::new("read,write,api,sensitive".into()),
        })
    }

    fn app(w: Arc<World>, fail_first: usize) -> (Telemon<FakeConnector>, Arc<dyn SecretStore>) {
        let mut st = Store::open_in_memory().unwrap();
        st.unlock(&crate::vault::generate_key().unwrap()).unwrap();
        let secrets: Arc<dyn SecretStore> = Arc::new(MemorySecrets::default());
        let conn = FakeConnector { world: Some(w), fail_first: AtomicUsize::new(fail_first), ..Default::default() };
        let cfg = RuntimeConfig { poll_every: Duration::from_millis(10), sync_every: None, traffic: None };
        (Telemon::new(st, secrets.clone(), conn, cfg), secrets)
    }

    fn nr(name: &str, can_write: bool) -> NewRouter {
        NewRouter { name: name.into(), host: "10.0.0.1".into(), port: 8728, tls_pin: None, username: "api".into(), can_write }
    }

    async fn wait_until(mut f: impl FnMut() -> bool) {
        for _ in 0..400 {
            if f() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("kondisi tidak tercapai tepat waktu");
    }

    #[tokio::test]
    async fn router_dan_kredensial() {
        let (t, secrets) = app(world(), 0);
        assert!(matches!(t.add_router(&nr("pusat", false), ""), Err(ServiceError::Config(_))));
        let r = t.add_router(&nr("pusat", false), "kata-sandi").unwrap();
        assert_eq!(secrets.get(&format!("router:{}", r.id)).as_deref(), Some("kata-sandi"));
        // Kata sandi tidak ada di basis data.
        assert!(!format!("{:?}", t.with_store(|s| s.list_routers()).unwrap()).contains("kata-sandi"));
        // Nama ganda: tidak meninggalkan kredensial yatim.
        assert!(t.add_router(&nr("PUSAT", false), "x").is_err());
        let rec = t.with_store(|s| s.list_routers()).unwrap();
        assert_eq!(rec.len(), 1);
        // Hapus: baris dan kredensial hilang.
        t.remove_router(&r.id).unwrap();
        assert_eq!(secrets.get(&format!("router:{}", r.id)), None);
        assert!(matches!(t.remove_router(&r.id), Err(ServiceError::Config(RouterError::NotFound))));
    }

    #[tokio::test]
    async fn probe_membaca_identitas_dan_hak() {
        let w = world();
        let (t, _) = app(w.clone(), 0);
        let p = t.probe("h:8728", "api", "x", TlsMode::Plain).await.unwrap();
        assert_eq!((p.model.as_str(), p.version.as_str(), p.can_write, p.can_read_passwords), ("RB4011", "7.15 (stable)", true, true));
        *w.group_policy.lock().unwrap() = "read,api".into();
        let p = t.probe("h:8728", "api", "x", TlsMode::Plain).await.unwrap();
        assert_eq!((p.can_write, p.can_read_passwords), (false, false));
        // Gagal sambung dilaporkan, bukan menggantung.
        let (t2, _) = app(world(), 1);
        assert!(matches!(t2.probe("h", "a", "b", TlsMode::Plain).await, Err(ServiceError::Router(Error::Connect(_)))));
    }

    #[tokio::test]
    async fn pemantau_menyambung_ulang_dan_menyalurkan_event() {
        let w = world();
        // Dua percobaan sambung pertama gagal: router tampak offline lalu online.
        let (t, _) = app(w.clone(), 2);
        let r = t.add_router(&nr("pusat", false), "pw").unwrap();
        let mut rx = t.subscribe();
        t.start_monitor(&r.id).unwrap();
        t.start_monitor(&r.id).unwrap(); // idempoten
        wait_until(|| t.with_store(|s| s.get_router(&r.id)).unwrap().unwrap().status == RouterStatus::Online).await;
        assert!(t.conn.connects.load(Ordering::SeqCst) >= 3);
        // Data awal masuk lewat sinkronisasi pertama.
        wait_until(|| !t.with_store(|s| s.list_users(Some(&r.id), false)).unwrap().is_empty()).await;
        assert_eq!(t.with_store(|s| s.get_router(&r.id)).unwrap().unwrap().model, "RB4011");

        // Sesi baru muncul: event Connected sampai ke pelanggan dan tersimpan.
        w.active.lock().unwrap().push(row(&[(".id", "*a2"), ("name", "sari"), ("service", "pppoe")]));
        let got = tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                let e = rx.recv().await.unwrap();
                if e.event.kind == EventKind::Connected && e.event.username.as_deref() == Some("sari") {
                    return e;
                }
            }
        })
        .await
        .expect("event diterima");
        assert_eq!(got.router_id, r.id);

        // Koneksi putus di tengah jalan: tersambung ulang otomatis.
        let before = t.conn.connects.load(Ordering::SeqCst);
        t.conn.last_alive.lock().unwrap().as_ref().unwrap().store(false, Ordering::SeqCst);
        wait_until(|| t.conn.connects.load(Ordering::SeqCst) > before).await;
        wait_until(|| t.with_store(|s| s.get_router(&r.id)).unwrap().unwrap().status == RouterStatus::Online).await;

        t.shutdown().await;
    }

    #[tokio::test]
    async fn tulis_hanya_bila_diizinkan_dan_tercatat() {
        let w = world();
        let (t, _) = app(w.clone(), 0);
        let ro = t.add_router(&nr("baca", false), "pw").unwrap();
        let rw = t.add_router(&nr("tulis", true), "pw").unwrap();
        t.import_now(&rw.id).await.unwrap();

        let nu = NewUser { name: "tika".into(), password: "tika-rahasia".into(), profile: "20M".into(), ..Default::default() };
        let plan = t.plan_add_user(&ro.id, &nu).await.unwrap();
        assert!(matches!(t.apply(&ro.id, "admin", &plan).await, Err(ServiceError::Rejected(_))));
        assert!(w.secrets.lock().unwrap().iter().all(|r| r["name"] != "tika"), "router baca-saja tidak boleh tertulis");

        // Router yang diizinkan: terapkan, salinan lokal ikut, password terenkripsi, riwayat tercatat.
        let plan = t.plan_add_user(&rw.id, &nu).await.unwrap();
        assert_eq!(t.apply(&rw.id, "admin", &plan).await.unwrap(), 1);
        let uid = format!("{}:tika", rw.id);
        assert!(t.with_store(|s| s.list_users(Some(&rw.id), false)).unwrap().iter().any(|u| u.username == "tika"));
        assert_eq!(t.with_store(|s| s.user_password(&uid)).unwrap().as_deref(), Some("tika-rahasia"));
        let log = t.with_store(|s| s.list_audit(&crate::store::AuditQuery { limit: 10, ..Default::default() })).unwrap();
        assert_eq!(log.len(), 1);
        assert!(!format!("{log:?}").contains("tika-rahasia"));

        // Hapus (soft): secret dinonaktifkan di router, ditandai manual di lokal, dan tidak dipulihkan sinkronisasi.
        let del = t.plan_soft_delete_user(&rw.id, "budi").await.unwrap();
        t.apply(&rw.id, "admin", &del).await.unwrap();
        t.import_now(&rw.id).await.unwrap();
        let users = t.with_store(|s| s.list_users(Some(&rw.id), true)).unwrap();
        let budi = users.iter().find(|u| u.username == "budi").unwrap();
        assert_eq!((budi.status, budi.deleted_reason.as_deref()), (UserStatus::Disabled, Some("manual")));
        assert!(w.active.lock().unwrap().is_empty(), "sesi diputus");
        // Pulihkan.
        let res = t.plan_restore_user(&rw.id, "budi").await.unwrap();
        t.apply(&rw.id, "admin", &res).await.unwrap();
        let budi = t.with_store(|s| s.list_users(Some(&rw.id), false)).unwrap().into_iter().find(|u| u.username == "budi").unwrap();
        assert_eq!(budi.deleted_at, None);
    }

    #[tokio::test]
    async fn ganti_kata_sandi_membuang_sambungan_lama() {
        let (t, _) = app(world(), 0);
        let r = t.add_router(&nr("pusat", false), "lama").unwrap();
        t.import_now(&r.id).await.unwrap();
        assert_eq!(t.conn.last_creds.lock().unwrap().2, "lama");
        t.update_router(&r.id, &RouterPatch { can_write: Some(true), ..Default::default() }, None).unwrap();
        let connects = t.conn.connects.load(Ordering::SeqCst);
        t.import_now(&r.id).await.unwrap();
        assert_eq!(t.conn.connects.load(Ordering::SeqCst), connects, "tanpa perubahan koneksi: sambungan dipakai ulang");
        t.update_router(&r.id, &RouterPatch::default(), Some("baru")).unwrap();
        t.import_now(&r.id).await.unwrap();
        assert_eq!(t.conn.last_creds.lock().unwrap().2, "baru");
        assert!(t.conn.connects.load(Ordering::SeqCst) > connects);
    }

    #[tokio::test]
    async fn purge_berkala_dari_pengaturan_tidak_menggantung() {
        let (t, _) = app(world(), 0);
        let r = t.add_router(&nr("pusat", false), "pw").unwrap();
        let old = Event { at_ms: 1, kind: EventKind::Connected, username: None, address: None, caller_id: None, message: "lama".into() };
        t.with_store(|s| s.insert_events(&r.id, &[old])).unwrap();
        // Kebijakan dibaca dari Store yang sama dengan yang dikunci purge: tidak boleh deadlock.
        t.start_purge_from_settings(Duration::from_millis(10));
        wait_until(|| t.with_store(|s| s.count_events()).unwrap() == 0).await;
        // Penyimpanan tetap bisa dipakai (tidak terkunci selamanya).
        assert!(t.with_store(|s| s.list_routers()).is_ok());
        tokio::time::timeout(Duration::from_secs(3), t.shutdown()).await.expect("shutdown tidak menggantung");
    }

    #[tokio::test]
    async fn purge_dan_pratinjau_lewat_service() {
        let (t, _) = app(world(), 0);
        let r = t.add_router(&nr("pusat", false), "pw").unwrap();
        let old = Event { at_ms: 1, kind: EventKind::Connected, username: None, address: None, caller_id: None, message: "lama".into() };
        t.with_store(|s| s.insert_events(&r.id, &[old])).unwrap();
        let pol = RetentionPolicy { event_days: Some(1), max_events: 1000, deleted_days: None, audit_days: None };
        assert_eq!(t.purge_preview(&pol).unwrap().events, 1);
        assert_eq!(t.purge_now(&pol).unwrap().events, 1);
        assert_eq!(t.with_store(|s| s.count_events()).unwrap(), 0);
        t.start_purge(Duration::from_millis(10), move || pol);
        t.shutdown().await;
    }
}
