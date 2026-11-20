//! Server demo: menjalankan `telemon-http` dengan **router tiruan** yang berubah sendiri, supaya UI bisa
//! dicoba di browser dengan data dari server sungguhan. Basis data di memori; hilang saat berhenti.
//! JANGAN dipakai untuk produksi: kata sandi akun demo diketahui publik.
//!
//! Pakai: `telemon-demo [--port 7878] [--static ../../dist] [--bind 127.0.0.1]`

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use telemon_core::auth::{Auth, HashParams, NewAccount};
use telemon_core::store::{NewRouter, Store};
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
        World { secrets: Mutex::new(secrets), profiles: Mutex::new(profiles), active: Mutex::new(active), log: Mutex::new(Vec::new()), seq: AtomicU64::new(1) }
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
            "/system/resource" => vec![row(&[("board-name", "RB4011iGS+ (demo)"), ("version", "7.15 (stable)")])],
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

    eprintln!("[demo] membuat penyimpanan");
    let mut store = Store::open_in_memory().expect("sqlite");
    store.unlock(&telemon_core::generate_key().expect("acak")).expect("unlock");
    let telemon = Arc::new(Telemon::new(store, Arc::new(MemorySecrets::default()), DemoConnector(worlds), RuntimeConfig { poll_every: Duration::from_secs(3), sync_every: Some(Duration::from_secs(30)) }));
    for (name, host, can_write) in [("RB-Utara", "demo-utara", true), ("RB-Selatan", "demo-selatan", false)] {
        let r = telemon.add_router(&NewRouter { name: name.into(), host: host.into(), port: 8728, tls_pin: None, username: "api".into(), can_write }, "demo").expect("router");
        eprintln!("[demo] impor awal {name}");
        telemon.import_now(&r.id).await.expect("impor awal");
        telemon.start_monitor(&r.id).expect("pemantau");
    }
    telemon.start_purge_from_settings(Duration::from_secs(3600));

    eprintln!("[demo] membuat akun");
    let auth = Arc::new(Auth::new(telemon.store(), HashParams::default()).expect("auth"));
    for (user, name, role) in [("admin", "Administrator Demo", "role-admin"), ("operator", "Operator Demo", "role-operator"), ("pantau", "Layar Pantau", "role-monitor")] {
        auth.create_account(None, &NewAccount { username: user.into(), display_name: name.into(), password: "demo-telemon".into(), role_id: role.into() }).expect("akun");
    }

    let cfg = HttpConfig { static_dir: (!static_dir.is_empty()).then(|| static_dir.clone().into()), db_path: "(memori, demo)".into(), ..Default::default() };
    eprintln!("[demo] menyiapkan server");
    let st = AppState::new(telemon.clone(), auth, cfg);
    // AI untuk mencoba UI tanpa kunci API: model offline berbasis kata kunci yang memakai alat baca asli.
    let ai = st.enable_ai(Arc::new(demo_llm::DemoLlm));
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
    println!("Akun: admin / operator / pantau, kata sandi: demo-telemon (admin dan operator wajib mendaftarkan 2FA saat masuk)");
    println!("PERINGATAN: hanya untuk mencoba UI; data tiruan, kata sandi diketahui publik.");
    telemon_http::serve(st, addr, async {
        let _ = tokio::signal::ctrl_c().await;
    })
    .await?;
    telemon.shutdown().await;
    Ok(())
}
