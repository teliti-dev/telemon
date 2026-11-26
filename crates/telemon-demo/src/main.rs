//! Server demo: menjalankan `telemon-http` dengan **router tiruan** yang berubah sendiri, supaya UI bisa
//! dicoba di browser dengan data dari server sungguhan. Basis data di memori; hilang saat berhenti.
//! JANGAN dipakai untuk produksi: kata sandi akun demo diketahui publik.
//!
//! Pakai: `telemon-demo [--port 7878] [--static ../../dist] [--bind 127.0.0.1]`

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use telemon_core::auth::{Auth, HashParams, Level, NewAccount, Permissions, Resource, RoleInput};
use telemon_core::store::{NewRouter, Store};
use telemon_core::traffic::{IfaceInfo, Sample, TrafficConfig};
use telemon_core::{Connector, Error, MemorySecrets, RouterApi, Row, RuntimeConfig, Telemon, TlsMode};
use telemon_http::{AppState, HttpConfig};

mod demo_llm;

fn row(kv: &[(&str, &str)]) -> Row {
    kv.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

/// Pembangkit acak sederhana (cukup untuk data demo).
struct Rng(AtomicU64);
impl Rng {
    fn next(&self) -> u64 {
        let mut x = self.0.load(Ordering::Relaxed);
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0.store(x, Ordering::Relaxed);
        x
    }
    fn below(&self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

struct World {
    secrets: Mutex<Vec<Row>>,
    profiles: Mutex<Vec<Row>>,
    active: Mutex<Vec<Row>>,
    log: Mutex<Vec<Row>>,
    seq: AtomicU64,
    /// Counter byte antarmuka tiruan (dinaikkan tiap `/interface` dibaca).
    ifaces: Mutex<IfaceState>,
}

#[derive(Default)]
struct IfaceState {
    last_ms: i64,
    bytes: HashMap<String, (f64, f64)>,
}

fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

/// Beban harian tiruan: sepi dini hari, puncak sekitar pukul 20.30 WIB (0.35 sampai 1.0).
fn load_factor(at_ms: i64) -> f64 {
    let hour = ((at_ms / 60_000) as f64 / 60.0 + 7.0) % 24.0;
    0.35 + 0.65 * (0.5 + 0.5 * (std::f64::consts::TAU * (hour - 20.5) / 24.0).cos())
}

fn plan_mbps(profile: &str) -> f64 {
    profile.trim_end_matches('M').parse::<f64>().unwrap_or(10.0)
}

/// Unduhan pelanggan (bit/detik) pada waktu `at_ms`: sebagian kecil dari paket, berubah tiap 20 detik.
fn user_down_bps(name: &str, mbps: f64, at_ms: i64) -> f64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in name.bytes().chain((at_ms / 20_000).to_le_bytes()) {
        h = (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3);
    }
    let frac = (h >> 11) as f64 / (1u64 << 53) as f64;
    mbps * 1e6 * (0.03 + 0.45 * frac) * load_factor(at_ms)
}

impl World {
    fn new(prefix: &str, users: usize, rng: &Rng) -> Self {
        let profiles = ["5M", "10M", "20M", "50M", "100M"]
            .iter()
            .enumerate()
            .map(|(i, n)| row(&[(".id", &format!("*p{i}")), ("name", n), ("rate-limit", &format!("{n}/{n}")), ("only-one", "yes")]))
            .collect::<Vec<_>>();
        let names = ["budi", "sari", "tika", "andi", "dewi", "rudi", "lina", "agus", "maya", "joko", "putri", "hadi", "wati", "eko", "nina"];
        let mut secrets = Vec::new();
        let mut active = Vec::new();
        for i in 0..users {
            let name = format!("{}{}-{}", names[i % names.len()], i / names.len() + 1, prefix);
            let profile = ["5M", "10M", "20M", "50M", "100M"][rng.below(5)];
            let disabled = i % 11 == 10;
            secrets.push(row(&[
                (".id", &format!("*{i:x}")),
                ("name", &name),
                ("service", "pppoe"),
                ("profile", profile),
                ("disabled", if disabled { "true" } else { "false" }),
                ("password", &format!("pw{:06}", rng.below(1_000_000))),
            ]));
            if !disabled && rng.below(10) < 7 {
                active.push(row(&[
                    (".id", &format!("*a{i:x}")),
                    ("name", &name),
                    ("service", "pppoe"),
                    ("address", &format!("100.64.{}.{}", 1 + i / 250, 2 + i % 250)),
                    ("caller-id", &format!("AA:BB:CC:{:02X}:{:02X}:{:02X}", i / 256, i % 256, rng.below(256))),
                    ("uptime", &format!("{}h{}m", rng.below(72), rng.below(60))),
                ]));
            }
        }
        World { secrets: Mutex::new(secrets), profiles: Mutex::new(profiles), active: Mutex::new(active), log: Mutex::new(Vec::new()), seq: AtomicU64::new(1), ifaces: Mutex::new(IfaceState::default()) }
    }

    /// Pelanggan aktif beserta paketnya (Mbps).
    fn active_plans(&self) -> Vec<(String, f64)> {
        let secrets = self.secrets.lock().unwrap();
        let profile_of = |n: &str| secrets.iter().find(|s| s["name"] == n).map_or(10.0, |s| plan_mbps(&s["profile"]));
        self.active.lock().unwrap().iter().map(|a| (a["name"].clone(), profile_of(&a["name"]))).collect()
    }

    /// `/interface/print` tiruan: ether1 (uplink), ether2 (LAN), dan satu antarmuka `pppoe-in` per sesi aktif.
    /// Arah sudut pandang router: unduhan pelanggan = `tx` di antarmuka PPPoE dan `rx` di uplink.
    fn interfaces(&self) -> Vec<Row> {
        let now = now_ms();
        let plans = self.active_plans();
        let mut st = self.ifaces.lock().unwrap();
        let dt = if st.last_ms == 0 { 0.0 } else { (now - st.last_ms).max(0) as f64 / 1000.0 };
        st.last_ms = now;
        let (mut down_total, mut up_total) = (0.0, 0.0);
        let mut rows = Vec::new();
        let mut live: Vec<String> = Vec::new();
        for (name, mbps) in &plans {
            let down = user_down_bps(name, *mbps, now);
            let up = down * 0.12;
            down_total += down;
            up_total += up;
            let key = format!("<pppoe-{name}>");
            let e = st.bytes.entry(key.clone()).or_insert((0.0, 0.0));
            e.0 += up / 8.0 * dt;
            e.1 += down / 8.0 * dt;
            let (rx, tx) = *e;
            rows.push(row(&[("name", &key), ("type", "pppoe-in"), ("running", "true"), ("disabled", "false"), ("dynamic", "true"), ("rx-byte", &(rx as u64).to_string()), ("tx-byte", &(tx as u64).to_string())]));
            live.push(key);
        }
        // Sesi yang sudah putus tidak lagi ada di router.
        st.bytes.retain(|k, _| !k.starts_with("<pppoe-") || live.contains(k));
        let fixed = [("ether1", "ether", down_total * 1.03, up_total * 1.03), ("ether2", "ether", up_total * 0.9, down_total * 0.9), ("bridge-lan", "bridge", up_total * 0.9, down_total * 0.9)];
        for (name, kind, rx_bps, tx_bps) in fixed {
            let e = st.bytes.entry(name.to_string()).or_insert((0.0, 0.0));
            e.0 += rx_bps / 8.0 * dt;
            e.1 += tx_bps / 8.0 * dt;
            let (rx, tx) = *e;
            rows.insert(0, row(&[("name", name), ("type", kind), ("running", "true"), ("disabled", "false"), ("rx-byte", &(rx as u64).to_string()), ("tx-byte", &(tx as u64).to_string())]));
        }
        rows
    }

    /// Riwayat 24 jam (satu sampel per menit) supaya grafik tidak kosong saat demo dibuka.
    fn history(&self, now: i64) -> (Vec<IfaceInfo>, Vec<(i64, Vec<Sample>)>) {
        let plans = self.active_plans();
        let ifaces = ["ether1", "ether2", "bridge-lan"]
            .iter()
            .map(|n| IfaceInfo { name: (*n).into(), kind: if *n == "bridge-lan" { "bridge".into() } else { "ether".into() }, running: true })
            .chain(std::iter::once(IfaceInfo { name: "@pppoe".into(), kind: "pppoe-aggregate".into(), running: true }))
            .collect();
        let series = (1..=1440)
            .rev()
            .map(|m| {
                let at = now - m * 60_000;
                let down: f64 = plans.iter().map(|(n, mbps)| user_down_bps(n, *mbps, at)).sum();
                let up = down * 0.12;
                let sessions = (plans.len() as i64 - ((at / 600_000) % 3)).max(0);
                let s = |iface: &str, rx: f64, tx: f64, sessions: Option<i64>| Sample { iface: iface.into(), rx_bps: rx as i64, tx_bps: tx as i64, sessions };
                (at, vec![s("ether1", down * 1.03, up * 1.03, None), s("ether2", up * 0.9, down * 0.9, None), s("bridge-lan", up * 0.9, down * 0.9, None), s("@pppoe", up, down, Some(sessions))])
            })
            .collect();
        (ifaces, series)
    }

    fn log_line(&self, topics: &str, msg: &str) {
        let id = self.seq.fetch_add(1, Ordering::SeqCst);
        let mut l = self.log.lock().unwrap();
        l.push(row(&[(".id", &format!("*{id:x}")), ("topics", topics), ("message", msg)]));
        let n = l.len();
        if n > 200 {
            l.drain(0..n - 200);
        }
    }

    /// Satu langkah dunia tiruan: sesi putus/tersambung dan sesekali gagal autentikasi.
    fn tick(&self, rng: &Rng) {
        let secrets = self.secrets.lock().unwrap().clone();
        if secrets.is_empty() {
            return;
        }
        let s = &secrets[rng.below(secrets.len())];
        let name = s["name"].clone();
        let mut active = self.active.lock().unwrap();
        match rng.below(10) {
            0..=3 => {
                if let Some(i) = active.iter().position(|a| a["name"] == name) {
                    active.remove(i);
                } else if s["disabled"] != "true" {
                    let id = rng.below(1_000_000);
                    active.push(row(&[(".id", &format!("*a{id:x}")), ("name", &name), ("service", "pppoe"), ("address", &format!("100.64.9.{}", 2 + id % 250)), ("caller-id", &format!("AA:BB:CC:00:{:02X}:{:02X}", id / 256 % 256, id % 256)), ("uptime", "0s")]));
                    drop(active);
                    self.log_line("pppoe,ppp,info", &format!("{name}: authenticated"));
                }
            }
            4 | 5 => {
                drop(active);
                self.log_line("pppoe,ppp,error", &format!("{name}: authentication failed"));
            }
            _ => {}
        }
    }
}

#[derive(Clone)]
struct DemoApi(Arc<World>);

impl RouterApi for DemoApi {
    async fn print(&self, path: &str) -> Result<Vec<Row>, Error> {
        let w = &self.0;
        Ok(match path {
            "/ppp/secret" => w.secrets.lock().unwrap().clone(),
            "/ppp/profile" => w.profiles.lock().unwrap().clone(),
            "/ppp/active" => w.active.lock().unwrap().clone(),
            "/log" => w.log.lock().unwrap().clone(),
            "/system/resource" => vec![row(&[("board-name", "RB4011iGS+ (demo)"), ("version", "7.15 (stable)"), ("cpu-load", "17"), ("free-memory", "412000000"), ("total-memory", "1000000000"), ("uptime", "3d4h12m")])],
            "/interface" => w.interfaces(),
            "/user" => vec![row(&[("name", "api"), ("group", "demo")])],
            "/user/group" => vec![row(&[("name", "demo"), ("policy", "read,write,api,sensitive")])],
            _ => vec![],
        })
    }

    async fn run(&self, cmd: &str, attrs: &[(String, String)]) -> Result<Vec<Row>, Error> {
        let w = &self.0;
        let get = |k: &str| attrs.iter().find(|(a, _)| a == k).map(|(_, v)| v.clone());
        let table = |p: &str| if p == "/ppp/secret" { &w.secrets } else { &w.profiles };
        match cmd {
            "/ppp/secret/add" | "/ppp/profile/add" => {
                let path = cmd.trim_end_matches("/add");
                let mut t = table(path).lock().unwrap();
                let name = get("name").unwrap_or_default();
                if t.iter().any(|r| r["name"] == name) {
                    return Err(Error::Trap("failure: already have such entry".into()));
                }
                let mut r: Row = attrs.iter().cloned().collect();
                r.insert(".id".into(), format!("*n{}", t.len() + 100));
                r.entry("disabled".into()).or_insert_with(|| "false".into());
                t.push(r);
            }
            "/ppp/secret/set" | "/ppp/profile/set" => {
                let path = cmd.trim_end_matches("/set");
                let id = get(".id").unwrap_or_default();
                let mut t = table(path).lock().unwrap();
                let r = t.iter_mut().find(|r| r[".id"] == id).ok_or_else(|| Error::Trap("no such item".into()))?;
                for (k, v) in attrs.iter().filter(|(k, _)| k != ".id") {
                    if v.is_empty() {
                        r.remove(k);
                    } else {
                        r.insert(k.clone(), if k == "disabled" { (v == "yes").to_string() } else { v.clone() });
                    }
                }
            }
            "/ppp/active/remove" => {
                let id = get(".id").unwrap_or_default();
                w.active.lock().unwrap().retain(|r| r[".id"] != id);
            }
            _ => return Err(Error::Trap("unknown command".into())),
        }
        Ok(vec![])
    }
}

struct DemoConnector(HashMap<String, Arc<World>>);

impl Connector for DemoConnector {
    type Api = DemoApi;
    async fn connect(&self, addr: &str, _user: &str, _pass: &str, _mode: TlsMode) -> Result<DemoApi, Error> {
        let host = addr.split(':').next().unwrap_or(addr);
        self.0.get(host).cloned().map(DemoApi).ok_or_else(|| Error::Connect(format!("router demo {host} tidak dikenal")))
    }
}

fn arg(name: &str, default: &str) -> String {
    let a: Vec<String> = std::env::args().collect();
    a.iter().position(|x| x == name).and_then(|i| a.get(i + 1)).cloned().unwrap_or_else(|| default.to_string())
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let bind: std::net::IpAddr = arg("--bind", "127.0.0.1").parse().expect("--bind harus alamat IP");
    let port: u16 = arg("--port", "7878").parse().expect("--port harus angka");
    let static_dir = arg("--static", "");
    let rng = Arc::new(Rng(AtomicU64::new(0x9E37_79B9_7F4A_7C15)));

    let mut worlds = HashMap::new();
    worlds.insert("demo-utara".to_string(), Arc::new(World::new("u", 40, &rng)));
    worlds.insert("demo-selatan".to_string(), Arc::new(World::new("s", 25, &rng)));
    let all: Vec<Arc<World>> = worlds.values().cloned().collect();
    let by_host = worlds.clone();

    eprintln!("[demo] membuat penyimpanan");
    let mut store = Store::open_in_memory().expect("sqlite");
    store.unlock(&telemon_core::generate_key().expect("acak")).expect("unlock");
    let telemon = Arc::new(Telemon::new(store, Arc::new(MemorySecrets::default()), DemoConnector(worlds), RuntimeConfig { poll_every: Duration::from_secs(3), sync_every: Some(Duration::from_secs(30)), traffic: Some(TrafficConfig { every: Duration::from_secs(5), retention: Duration::from_secs(2 * 86_400) }) }));
    let mut router_ids: Vec<String> = Vec::new();
    for (name, host, can_write) in [("RB-Utara", "demo-utara", true), ("RB-Selatan", "demo-selatan", false)] {
        let r = telemon.add_router(&NewRouter { name: name.into(), host: host.into(), port: 8728, tls_pin: None, username: "api".into(), can_write }, "demo").expect("router");
        eprintln!("[demo] impor awal {name}");
        telemon.import_now(&r.id).await.expect("impor awal");
        let now = now_ms();
        let (ifaces, series) = by_host[host].history(now);
        telemon.with_store(|s| {
            for (at, samples) in &series {
                s.insert_traffic(&r.id, *at, &ifaces, samples).expect("riwayat bandwidth");
            }
        });
        telemon.start_monitor(&r.id).expect("pemantau");
        router_ids.push(r.id.clone());
    }
    // Tata letak Ringkasan demo: susunan bawaan ditambah widget bandwidth supaya langsung terlihat saat dicoba.
    {
        let w = |id: &str, ty: &str, x: u32, y: u32, w: u32, h: u32, config: serde_json::Value| serde_json::json!({ "id": id, "type": ty, "x": x, "y": y, "w": w, "h": h, "config": config });
        let mut widgets = vec![
            w("default-routers-online", "stat-routers-online", 0, 0, 4, 24, serde_json::json!({})),
            w("default-users-active", "stat-users-active", 4, 0, 4, 24, serde_json::json!({})),
            w("default-users-inactive", "stat-users-inactive", 8, 0, 4, 24, serde_json::json!({})),
            w("default-users-disabled", "stat-users-disabled", 12, 0, 4, 24, serde_json::json!({})),
            w("default-auth-fails", "stat-auth-fails", 16, 0, 4, 24, serde_json::json!({})),
            w("bw-chart-pppoe", "bandwidth-chart", 0, 24, 12, 84, serde_json::json!({ "router": "all", "iface": "@pppoe", "range": "24h" })),
            w("bw-total", "bandwidth-total", 12, 24, 8, 48, serde_json::json!({ "range": "6h" })),
            w("bw-per-user", "bandwidth-per-user", 12, 72, 8, 36, serde_json::json!({ "router": "all", "range": "6h" })),
            w("bw-chart-uplink", "bandwidth-chart", 0, 108, 10, 84, serde_json::json!({ "router": router_ids[0], "iface": "ether1", "range": "6h" })),
            w("default-recent-events", "recent-events", 10, 108, 10, 84, serde_json::json!({})),
            w("default-router-cards", "router-cards", 0, 192, 20, 48, serde_json::json!({})),
            w("default-top-auth-fails", "top-auth-fails", 0, 240, 7, 96, serde_json::json!({})),
            w("users-per-profile", "users-per-profile", 7, 240, 7, 96, serde_json::json!({})),
            w("user-status-mix", "user-status-mix", 14, 240, 6, 48, serde_json::json!({})),
        ];
        widgets.shrink_to_fit();
        let layout = serde_json::json!({ "version": 1, "widgets": widgets });
        telemon.with_store(|s| s.kv_set("dashboard.layout", &layout.to_string())).expect("tata letak demo");
    }
    telemon.start_purge_from_settings(Duration::from_secs(3600));

    eprintln!("[demo] membuat akun");
    let auth = Arc::new(Auth::new(telemon.store(), HashParams::default()).expect("auth"));
    // Monitor yang juga boleh mengatur widget Ringkasan dan memakai AI (chat dan dasbor AI), supaya fitur itu bisa dicoba.
    let monitor_plus = auth
        .create_role(
            None,
            &RoleInput {
                name: "Monitor Plus".into(),
                description: "Demo: layar pantau yang boleh mengatur widget Ringkasan, memakai AI chat, dan mengatur AI di Pengaturan.".into(),
                permissions: Permissions::default().with(Resource::Ringkasan, Level::Update).with(Resource::Event, Level::Read).with(Resource::Router, Level::Read).with(Resource::Pengaturan, Level::Update).with(Resource::Ai, Level::Read),
            },
        )
        .expect("peran demo");
    for (user, name, role) in [("admin", "Administrator Demo", "role-admin".to_string()), ("operator", "Operator Demo", "role-operator".to_string()), ("pantau", "Layar Pantau", monitor_plus.id)] {
        auth.create_account(None, &NewAccount { username: user.into(), display_name: name.into(), password: "demo-telemon".into(), role_id: role }).expect("akun");
    }

    let cfg = HttpConfig { static_dir: (!static_dir.is_empty()).then(|| static_dir.clone().into()), db_path: "(memori, demo)".into(), relax_update_2fa: true, extensions: telemon_http::ExtConfig::from_env(std::env::temp_dir().join("telemon-demo-extensions"), true), ..Default::default() };
    eprintln!("[demo] menyiapkan server");
    let st = AppState::new(telemon.clone(), auth, cfg);
    // AI untuk mencoba UI tanpa kunci API: model offline berbasis kata kunci yang memakai alat baca asli.
    let ai = st.enable_ai(Arc::new(demo_llm::DemoLlm));
    if st.cfg.extensions.is_some() {
        telemon_http::spawn_scheduler(st.clone());
        // Toko demo: katalog dan paket dari folder lokal (`TELEMON_DEMO_STORE_DIR`), tanpa jaringan.
        if let Ok(dir) = std::env::var("TELEMON_DEMO_STORE_DIR") {
            st.set_ext_fetcher(Arc::new(DirFetcher(std::path::PathBuf::from(dir))));
        }
    }
    ai.save_provider(None, telemon_ai::AiProviderInput { name: "Demo (offline)".into(), kind: telemon_ai::ProviderKind::Openai, model: "demo-offline".into(), base_url: None, api_key: Some("demo-tidak-dipakai".into()), price_in_per_mtok: Some(1.0), price_out_per_mtok: Some(2.0) }).expect("provider demo");

    // Dunia tiruan berubah tiap 2 detik.
    let ticker = rng.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(2)).await;
            for w in &all {
                w.tick(&ticker);
            }
        }
    });

    let addr = SocketAddr::new(bind, port);
    println!("Telemon demo di http://{addr}  (UI: {})", if static_dir.is_empty() { "tidak dilayani; pakai --static" } else { &static_dir });
    println!("Akun: admin / operator / pantau, kata sandi: demo-telemon (pantau = Monitor Plus: widget, AI chat, dan pengaturan AI; 2FA dilonggarkan hanya di demo)");
    println!("PERINGATAN: hanya untuk mencoba UI; data tiruan, kata sandi diketahui publik.");
    telemon_http::serve(st, addr, async {
        let _ = tokio::signal::ctrl_c().await;
    })
    .await?;
    telemon.shutdown().await;
    Ok(())
}

/// Pengunduh toko untuk demo: membaca berkas bernama sama dengan segmen terakhir alamat dari satu folder.
struct DirFetcher(std::path::PathBuf);

impl telemon_http::ExtFetch for DirFetcher {
    fn get(&self, url: &str, max_bytes: usize) -> Result<Vec<u8>, String> {
        let name = url.rsplit('/').next().unwrap_or("");
        if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_')) || name.contains("..") {
            return Err(format!("nama berkas tidak valid: {name}"));
        }
        let bytes = std::fs::read(self.0.join(name)).map_err(|e| format!("{name}: {e}"))?;
        if bytes.len() > max_bytes {
            return Err("berkas terlalu besar".into());
        }
        Ok(bytes)
    }
}
