//! Uji integrasi server HTTP: permintaan lewat `tower::oneshot` (tanpa jaringan) dan satu uji
//! WebSocket lewat TCP sungguhan. Router dan kredensial memakai tiruan.

use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{HeaderMap, Method, Request, StatusCode, header};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use telemon_core::auth::{Auth, HashParams, Level, LoginOutcome, NewAccount, Permissions, Resource, RoleInput, Via, base32_decode, totp_code};
use telemon_core::store::Store;
use telemon_core::{Connector, Error, MemorySecrets, RouterApi, Row, RuntimeConfig, SecretStore, Telemon, TlsMode};
use telemon_http::{AppState, HttpConfig, Tunnel, UpdateInfo, Updater, app, app_desktop};
use tower::ServiceExt;

const T0: i64 = 1_000_000_000_000;
const LAN: &str = "10.0.0.5:4000";
const LOOP: &str = "127.0.0.1:4001";

fn row(kv: &[(&str, &str)]) -> Row {
    kv.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

struct World {
    secrets: Mutex<Vec<Row>>,
    profiles: Vec<Row>,
    active: Mutex<Vec<Row>>,
}

#[derive(Clone)]
struct FakeApi(Arc<World>);

impl RouterApi for FakeApi {
    async fn print(&self, path: &str) -> Result<Vec<Row>, Error> {
        let w = &self.0;
        Ok(match path {
            "/ppp/secret" => w.secrets.lock().unwrap().clone(),
            "/ppp/profile" => w.profiles.clone(),
            "/ppp/active" => w.active.lock().unwrap().clone(),
            "/system/resource" => vec![row(&[("board-name", "RB4011"), ("version", "7.15")])],
            "/user" => vec![row(&[("name", "api"), ("group", "g")])],
            "/user/group" => vec![row(&[("name", "g"), ("policy", "read,write,api,sensitive")])],
            _ => vec![],
        })
    }

    async fn run(&self, cmd: &str, attrs: &[(String, String)]) -> Result<Vec<Row>, Error> {
        let w = &self.0;
        let get = |k: &str| attrs.iter().find(|(a, _)| a == k).map(|(_, v)| v.clone());
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

struct FakeConnector {
    world: Arc<World>,
    connects: AtomicUsize,
    reachable: Arc<AtomicBool>,
}

impl Connector for FakeConnector {
    type Api = FakeApi;
    async fn connect(&self, _a: &str, _u: &str, _p: &str, _m: TlsMode) -> Result<FakeApi, Error> {
        self.connects.fetch_add(1, Ordering::SeqCst);
        if !self.reachable.load(Ordering::SeqCst) {
            return Err(Error::Connect("tidak terjangkau".into()));
        }
        Ok(FakeApi(self.world.clone()))
    }
}

struct T {
    app: Router,
    st: Arc<AppState<FakeConnector>>,
    auth: Arc<Auth>,
    telemon: Arc<Telemon<FakeConnector>>,
    clock: Arc<AtomicI64>,
    world: Arc<World>,
    reachable: Arc<AtomicBool>,
}

fn harness_with(cfg: HttpConfig) -> T {
    let world = Arc::new(World {
        secrets: Mutex::new(vec![row(&[(".id", "*1"), ("name", "budi"), ("profile", "10M"), ("disabled", "false"), ("password", "rahasia123")])]),
        profiles: vec![row(&[(".id", "*p1"), ("name", "10M"), ("rate-limit", "10M/10M")]), row(&[(".id", "*p2"), ("name", "20M")])],
        active: Mutex::new(vec![row(&[(".id", "*a1"), ("name", "budi"), ("service", "pppoe"), ("uptime", "5m")])]),
    });
    let mut store = Store::open_in_memory().unwrap();
    store.unlock(&telemon_core::generate_key().unwrap()).unwrap();
    let secrets: Arc<dyn SecretStore> = Arc::new(MemorySecrets::default());
    let reachable = Arc::new(AtomicBool::new(true));
    let conn = FakeConnector { world: world.clone(), connects: AtomicUsize::new(0), reachable: reachable.clone() };
    let telemon = Arc::new(Telemon::new(store, secrets, conn, RuntimeConfig { poll_every: Duration::from_millis(20), sync_every: None, traffic: None }));
    let clock = Arc::new(AtomicI64::new(T0));
    let (c1, c2) = (clock.clone(), clock.clone());
    let auth = Arc::new(Auth::with_clock(telemon.store(), HashParams { memory_kib: 8, iterations: 1, parallelism: 1 }, Box::new(move || c1.load(Ordering::SeqCst))).unwrap());
    let st = AppState::with_clock(telemon.clone(), auth.clone(), cfg, Arc::new(move || c2.load(Ordering::SeqCst)));
    T { app: app(st.clone()), st, auth, telemon, clock, world, reachable }
}

fn harness() -> T {
    harness_with(HttpConfig::default())
}

struct Resp {
    status: StatusCode,
    headers: HeaderMap,
    body: Value,
}

impl Resp {
    fn code(&self) -> &str {
        self.body["error"]["code"].as_str().unwrap_or("")
    }
    fn cookie(&self) -> Option<String> {
        self.headers.get_all(header::SET_COOKIE).iter().filter_map(|v| v.to_str().ok()).find_map(|c| c.strip_prefix("telemon_session=").map(|r| r.split(';').next().unwrap().to_string()))
    }
    fn set_cookie(&self) -> String {
        self.headers.get(header::SET_COOKIE).map(|v| v.to_str().unwrap().to_string()).unwrap_or_default()
    }
}

impl T {
    fn st_workers(&self, key: &str) -> telemon_http::WorkerState {
        self.st.worker_state(key)
    }

    async fn call(&self, method: Method, uri: &str, body: Option<Value>, cookie: Option<&str>, extra: &[(&str, &str)], peer: &str) -> Resp {
        let mut b = Request::builder().method(method).uri(uri);
        if body.is_some() {
            b = b.header(header::CONTENT_TYPE, "application/json");
        }
        if let Some(c) = cookie {
            b = b.header(header::COOKIE, format!("telemon_session={c}"));
        }
        for (k, v) in extra {
            b = b.header(*k, *v);
        }
        let mut req = b.body(body.map_or(Body::empty(), |v| Body::from(v.to_string()))).unwrap();
        req.extensions_mut().insert(ConnectInfo(peer.parse::<SocketAddr>().unwrap()));
        let res = self.app.clone().oneshot(req).await.unwrap();
        let (parts, body) = res.into_parts();
        let bytes = body.collect().await.unwrap().to_bytes();
        let body = serde_json::from_slice(&bytes).unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into()));
        Resp { status: parts.status, headers: parts.headers, body }
    }
    async fn get(&self, uri: &str, cookie: Option<&str>) -> Resp {
        self.call(Method::GET, uri, None, cookie, &[], LAN).await
    }
    async fn post(&self, uri: &str, body: Value, cookie: Option<&str>) -> Resp {
        self.call(Method::POST, uri, Some(body), cookie, &[], LAN).await
    }

    fn account(&self, user: &str, role: &str) -> String {
        self.auth.create_account(None, &NewAccount { username: user.into(), display_name: user.into(), password: "password-uji".into(), role_id: role.into() }).unwrap().id
    }

    /// Masuk lewat HTTP (dengan pendaftaran 2FA bila diwajibkan); mengembalikan cookie.
    async fn login(&self, user: &str) -> String {
        let r = self.post("/api/login", json!({"username": user, "password": "password-uji"}), None).await;
        assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
        match r.body["status"].as_str().unwrap() {
            "ok" => r.cookie().expect("cookie"),
            "2fa_setup_required" => {
                let code = self.code(r.body["secret"].as_str().unwrap());
                let s = self.post("/api/login/2fa-setup", json!({"challengeId": r.body["challengeId"], "code": code}), None).await;
                assert_eq!(s.status, StatusCode::OK, "{:?}", s.body);
                s.cookie().expect("cookie")
            }
            other => panic!("status tak terduga {other}"),
        }
    }

    fn code(&self, secret_b32: &str) -> String {
        format!("{:06}", totp_code(&base32_decode(secret_b32).unwrap(), (self.clock.load(Ordering::SeqCst) / 30_000) as u64))
    }

    fn router(&self, can_write: bool) -> String {
        let n = telemon_core::store::NewRouter { name: "pusat".into(), host: "10.1.1.1".into(), port: 8728, tls_pin: None, username: "api".into(), can_write };
        let r = self.telemon.add_router(&n, "pw-router").unwrap();
        r.id
    }
}

// ---------------- dasar dan header ----------------

#[tokio::test]
async fn kesehatan_404_json_dan_header_keamanan() {
    let t = harness();
    let r = t.get("/api/health", None).await;
    assert_eq!((r.status, r.body["ok"].as_bool()), (StatusCode::OK, Some(true)));
    assert_eq!(r.headers[header::CACHE_CONTROL], "no-store");
    assert_eq!(r.headers[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
    assert_eq!(r.headers[header::X_FRAME_OPTIONS], "DENY");
    let r = t.get("/api/tidak-ada", None).await;
    assert_eq!((r.status, r.code()), (StatusCode::NOT_FOUND, "not_found"));
    // Tanpa folder UI, jalur lain 404 tanpa CSP pada API.
    assert_eq!(t.get("/", None).await.status, StatusCode::NOT_FOUND);
    assert!(t.get("/api/health", None).await.headers.get(header::CONTENT_SECURITY_POLICY).is_none());
}

#[tokio::test]
async fn halaman_ui_divalidasi_ulang_agar_tidak_menampilkan_versi_lama() {
    let dir = std::env::temp_dir().join(format!("telemon-ui-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("assets")).unwrap();
    std::fs::write(dir.join("index.html"), "<html></html>").unwrap();
    std::fs::write(dir.join("assets/app-abc123.js"), "x").unwrap();
    let t = harness_with(HttpConfig { static_dir: Some(dir.clone()), ..Default::default() });
    let page = t.get("/", None).await;
    assert_eq!(page.headers[header::CACHE_CONTROL], "no-cache", "index.html harus selalu divalidasi ulang");
    assert!(page.headers.get(header::CONTENT_SECURITY_POLICY).is_some());
    // Rute SPA yang jatuh ke index.html juga.
    assert_eq!(t.get("/pengaturan/extension", None).await.headers[header::CACHE_CONTROL], "no-cache");
    // Aset bernama hash tidak diberi no-cache dan tetap mendapat CSP.
    let asset = t.get("/assets/app-abc123.js", None).await;
    assert!(asset.headers.get(header::CACHE_CONTROL).is_none());
    assert!(asset.headers.get(header::CONTENT_SECURITY_POLICY).is_some());
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn badan_terlalu_besar_dan_json_rusak_ditolak() {
    let t = harness();
    let big = json!({"username": "a", "password": "x".repeat(70_000)});
    assert_eq!(t.post("/api/login", big, None).await.status, StatusCode::PAYLOAD_TOO_LARGE);
    let mut req = Request::builder().method("POST").uri("/api/login").header(header::CONTENT_TYPE, "application/json").body(Body::from("{rusak")).unwrap();
    req.extensions_mut().insert(ConnectInfo(LAN.parse::<SocketAddr>().unwrap()));
    assert!(t.app.clone().oneshot(req).await.unwrap().status().is_client_error());
    // Tanpa Content-Type JSON.
    let mut req = Request::builder().method("POST").uri("/api/login").body(Body::from("{}")).unwrap();
    req.extensions_mut().insert(ConnectInfo(LAN.parse::<SocketAddr>().unwrap()));
    assert_eq!(t.app.clone().oneshot(req).await.unwrap().status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
}

// ---------------- masuk, cookie, via ----------------

#[tokio::test]
async fn login_cookie_sesi_dan_keluar() {
    let t = harness();
    t.account("pantau", "role-monitor");
    let bad = t.post("/api/login", json!({"username": "pantau", "password": "salah"}), None).await;
    assert_eq!((bad.status, bad.code()), (StatusCode::UNAUTHORIZED, "bad_credentials"));
    assert!(bad.cookie().is_none());

    let ok = t.post("/api/login", json!({"username": "pantau", "password": "password-uji"}), None).await;
    assert_eq!(ok.status, StatusCode::OK);
    let sc = ok.set_cookie();
    for flag in ["HttpOnly", "SameSite=Strict", "Path=/", "Max-Age=43200"] {
        assert!(sc.contains(flag), "{flag} hilang: {sc}");
    }
    assert!(!sc.contains("Secure"), "LAN tanpa HTTPS tidak boleh Secure (cookie tidak akan terkirim): {sc}");
    assert_eq!(ok.body["session"]["via"], "lan");
    let c = ok.cookie().unwrap();

    assert_eq!(t.get("/api/session", None).await.status, StatusCode::UNAUTHORIZED);
    assert_eq!(t.get("/api/session", Some("ngawur")).await.status, StatusCode::UNAUTHORIZED);
    let s = t.get("/api/session", Some(&c)).await;
    assert_eq!((s.status, s.body["username"].as_str(), s.body["permissions"]["event"].as_str()), (StatusCode::OK, Some("pantau"), Some("read")));
    assert_eq!(s.body["permissions"]["admin"], "none");

    let out = t.post("/api/logout", json!({}), Some(&c)).await;
    assert_eq!(out.status, StatusCode::NO_CONTENT);
    assert!(out.set_cookie().contains("Max-Age=0"));
    assert_eq!(t.get("/api/session", Some(&c)).await.status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn akses_publik_dikenali_hanya_dari_loopback() {
    let t = harness();
    t.account("pantau", "role-monitor");
    let body = json!({"username": "pantau", "password": "password-uji"});
    // Sidecar tunnel (loopback + CF-Connecting-IP): publik, cookie Secure.
    let r = t.call(Method::POST, "/api/login", Some(body.clone()), None, &[("cf-connecting-ip", "203.0.113.9")], LOOP).await;
    assert_eq!(r.body["session"]["via"], "publik");
    assert!(r.set_cookie().contains("Secure"));
    // Klien LAN yang memalsukan header: diabaikan, tetap LAN.
    let r = t.call(Method::POST, "/api/login", Some(body), None, &[("cf-connecting-ip", "203.0.113.9")], LAN).await;
    assert_eq!(r.body["session"]["via"], "lan");
    assert!(!r.set_cookie().contains("Secure"));
}

#[tokio::test]
async fn pembatasan_laju_login_per_ip() {
    let t = harness_with(HttpConfig { login_per_minute: 3, ..Default::default() });
    for _ in 0..3 {
        assert_eq!(t.post("/api/login", json!({"username": "x", "password": "y"}), None).await.status, StatusCode::UNAUTHORIZED);
    }
    let r = t.post("/api/login", json!({"username": "x", "password": "y"}), None).await;
    assert_eq!((r.status, r.code()), (StatusCode::TOO_MANY_REQUESTS, "rate_limited"));
    assert!(r.headers.get(header::RETRY_AFTER).is_some());
    // IP lain tidak terpengaruh; jendela berikutnya pulih.
    let other = t.call(Method::POST, "/api/login", Some(json!({"username": "x", "password": "y"})), None, &[], "10.0.0.9:1").await;
    assert_eq!(other.status, StatusCode::UNAUTHORIZED);
    t.clock.fetch_add(61_000, Ordering::SeqCst);
    assert_eq!(t.post("/api/login", json!({"username": "x", "password": "y"}), None).await.status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn penguncian_akun_menjadi_429() {
    let t = harness_with(HttpConfig { login_per_minute: 1000, ..Default::default() });
    t.account("budi", "role-viewer");
    for _ in 0..5 {
        let _ = t.post("/api/login", json!({"username": "budi", "password": "salah"}), None).await;
    }
    let r = t.post("/api/login", json!({"username": "budi", "password": "password-uji"}), None).await;
    assert_eq!((r.status, r.code()), (StatusCode::TOO_MANY_REQUESTS, "locked"));
    assert_eq!(r.headers[header::RETRY_AFTER], "300");
}

#[tokio::test]
async fn alur_2fa_lewat_http() {
    let t = harness();
    t.account("operator", "role-operator");
    let r = t.post("/api/login", json!({"username": "operator", "password": "password-uji"}), None).await;
    assert_eq!(r.body["status"], "2fa_setup_required");
    assert!(r.cookie().is_none(), "belum boleh ada sesi sebelum 2FA");
    let secret = r.body["secret"].as_str().unwrap().to_string();
    assert!(r.body["otpauthUrl"].as_str().unwrap().starts_with("otpauth://totp/"));
    let bad = t.post("/api/login/2fa-setup", json!({"challengeId": r.body["challengeId"], "code": "000000"}), None).await;
    assert_eq!((bad.status, bad.code()), (StatusCode::UNAUTHORIZED, "bad_code"));
    let ok = t.post("/api/login/2fa-setup", json!({"challengeId": r.body["challengeId"], "code": t.code(&secret)}), None).await;
    assert_eq!(ok.status, StatusCode::OK);
    assert_eq!(ok.body["recoveryCodes"].as_array().unwrap().len(), 8);
    let cookie = ok.cookie().unwrap();
    assert_eq!(t.get("/api/session", Some(&cookie)).await.body["twoFactor"], true);

    // Masuk berikutnya: minta kode (di langkah waktu berikutnya agar tidak dianggap pakai ulang).
    let r = t.post("/api/login", json!({"username": "operator", "password": "password-uji"}), None).await;
    assert_eq!(r.body["status"], "2fa_required");
    t.clock.fetch_add(30_000, Ordering::SeqCst);
    let v = t.post("/api/login/2fa", json!({"challengeId": r.body["challengeId"], "code": t.code(&secret)}), None).await;
    assert_eq!(v.status, StatusCode::OK);
    assert!(v.cookie().is_some());
    // Kode pemulihan.
    let r = t.post("/api/login", json!({"username": "operator", "password": "password-uji"}), None).await;
    let rc = ok.body["recoveryCodes"][0].as_str().unwrap();
    let v = t.post("/api/login/2fa", json!({"challengeId": r.body["challengeId"], "code": rc, "recovery": true}), None).await;
    assert_eq!(v.status, StatusCode::OK);
}

// ---------------- CSRF ----------------

#[tokio::test]
async fn permintaan_lintas_asal_ditolak() {
    let t = harness();
    t.account("pantau", "role-monitor");
    let body = json!({"username": "pantau", "password": "password-uji"});
    let call = |extra: &'static [(&'static str, &'static str)]| t.call(Method::POST, "/api/login", Some(body.clone()), None, extra, LAN);
    assert_eq!(call(&[("host", "telemon.lan:7878"), ("origin", "http://evil.example")]).await.status, StatusCode::FORBIDDEN);
    assert_eq!(call(&[("host", "telemon.lan:7878"), ("origin", "http://telemon.lan:7878")]).await.status, StatusCode::OK);
    assert_eq!(call(&[("sec-fetch-site", "cross-site")]).await.status, StatusCode::FORBIDDEN);
    assert_eq!(call(&[("host", "x"), ("sec-fetch-site", "same-origin"), ("origin", "https://x")]).await.status, StatusCode::OK);
    // GET tidak terkena pemeriksaan (tidak mengubah keadaan).
    assert_eq!(t.call(Method::GET, "/api/health", None, None, &[("origin", "http://evil.example"), ("host", "x")], LAN).await.status, StatusCode::OK);
}

// ---------------- izin dan data ----------------

#[tokio::test]
async fn snapshot_mengikuti_izin_dan_password_tidak_pernah_ikut() {
    let t = harness();
    let rid = t.router(false);
    t.telemon.import_now(&rid).await.unwrap();
    t.account("pantau", "role-monitor");
    t.account("viewer", "role-viewer");
    let (cm, cv) = (t.login("pantau").await, t.login("viewer").await);

    let m = t.get("/api/snapshot", Some(&cm)).await;
    assert_eq!(m.status, StatusCode::OK);
    assert_eq!(m.body["routers"].as_array().unwrap().len(), 1);
    assert!(m.body["users"].as_array().unwrap().is_empty(), "Monitor tidak punya izin Users");
    assert!(m.body["profiles"].as_array().unwrap().is_empty());
    let v = t.get("/api/snapshot", Some(&cv)).await;
    assert_eq!(v.body["users"].as_array().unwrap().len(), 1);
    assert_eq!(v.body["profiles"].as_array().unwrap().len(), 2);
    let dump = v.body.to_string();
    assert!(!dump.contains("rahasia123") && !dump.contains("pw-router") && !dump.contains("password\""), "password bocor: {dump}");
    assert_eq!(v.body["routers"][0]["tls"], false);
    assert_eq!(v.body["users"][0]["status"], "active");
    assert_eq!(t.get("/api/snapshot", None).await.status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn endpoint_admin_dan_audit_butuh_izin() {
    let t = harness();
    t.account("viewer", "role-viewer");
    let cv = t.login("viewer").await;
    for path in ["/api/accounts", "/api/roles", "/api/audit"] {
        let r = t.get(path, Some(&cv)).await;
        assert_eq!((r.status, r.code()), (StatusCode::FORBIDDEN, "forbidden"), "{path}");
    }
    let create = t.post("/api/accounts", json!({"username": "baru", "displayName": "B", "password": "password-uji", "roleId": "role-viewer"}), Some(&cv)).await;
    assert_eq!(create.status, StatusCode::FORBIDDEN);

    t.account("admin", "role-admin");
    let ca = t.login("admin").await;
    let created = t.post("/api/accounts", json!({"username": "baru", "displayName": "B", "password": "password-uji", "roleId": "role-viewer"}), Some(&ca)).await;
    assert_eq!((created.status, created.body["username"].as_str()), (StatusCode::CREATED, Some("baru")));
    assert_eq!(t.get("/api/accounts", Some(&ca)).await.body.as_array().unwrap().len(), 3);
    let roles = t.get("/api/roles", Some(&ca)).await;
    assert_eq!(roles.body.as_array().unwrap().len(), 4);
    // Riwayat memuat pembuatan akun tanpa rahasia.
    let a = t.get("/api/audit?limit=100", Some(&ca)).await;
    let dump = a.body.to_string();
    assert!(dump.contains("account.create") && !dump.contains("password-uji"));
}

#[tokio::test]
async fn pengaman_anti_kenaikan_izin_lewat_http() {
    let t = harness();
    let role = t.auth.create_role(None, &RoleInput { name: "kecil".into(), description: String::new(), permissions: Permissions::default().with(Resource::Admin, Level::Update).with(Resource::User, Level::Read) }).unwrap();
    t.account("kecil", &role.id);
    let owner = t.account("owner", "role-admin");
    let c = t.login("kecil").await;
    let r = t.post("/api/accounts", json!({"username": "naik", "displayName": "N", "password": "password-uji", "roleId": "role-admin"}), Some(&c)).await;
    assert_eq!((r.status, r.code()), (StatusCode::FORBIDDEN, "forbidden"));
    let r = t.post(&format!("/api/accounts/{owner}/password"), json!({"password": "password-baru"}), Some(&c)).await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    let r = t.call(Method::DELETE, &format!("/api/accounts/{owner}"), None, Some(&c), &[], LAN).await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    // Izin peran tidak diterima jika menu/level tidak dikenal.
    let r = t.post("/api/roles", json!({"name": "x", "permissions": {"gaib": "read"}}), Some(&c)).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    let r = t.post("/api/roles", json!({"name": "x", "permissions": {"user": "super"}}), Some(&c)).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn cookie_lan_dipakai_lewat_publik_tidak_boleh_menulis() {
    let t = harness();
    let rid = t.router(true);
    t.account("operator", "role-operator");
    let c = t.login("operator").await;
    let plan = json!({"action": "user.soft_delete", "name": "budi"});
    // Lewat LAN: boleh membuat rencana.
    assert_eq!(t.post(&format!("/api/routers/{rid}/plans"), plan.clone(), Some(&c)).await.status, StatusCode::OK);
    // Cookie yang sama lewat tunnel publik: tulis ditolak (kebijakan publik), baca tetap jalan.
    let via_public = |m: Method, p: String, b: Option<Value>| {
        let c = c.clone();
        let t = &t;
        async move { t.call(m, &p, b, Some(&c), &[("cf-connecting-ip", "203.0.113.9")], LOOP).await }
    };
    let w = via_public(Method::POST, format!("/api/routers/{rid}/plans"), Some(plan.clone())).await;
    assert_eq!((w.status, w.code()), (StatusCode::FORBIDDEN, "forbidden"));
    assert_eq!(via_public(Method::GET, "/api/snapshot".into(), None).await.status, StatusCode::OK);
    // Pemilik mengizinkan tulis lewat publik.
    let mut p = t.auth.policy();
    p.allow_public_write = true;
    t.auth.set_policy(p);
    assert_eq!(via_public(Method::POST, format!("/api/routers/{rid}/plans"), Some(plan)).await.status, StatusCode::OK);
}

// ---------------- tulis dengan tinjauan ----------------

#[tokio::test]
async fn rencana_tulis_sekali_pakai_milik_pembuat() {
    let t = harness();
    let rid = t.router(true);
    t.telemon.import_now(&rid).await.unwrap();
    t.account("operator", "role-operator");
    t.account("operator2", "role-operator");
    let (c1, c2) = (t.login("operator").await, t.login("operator2").await);

    let nu = json!({"action": "user.add", "name": "tika", "password": "tika-rahasia", "profile": "20M"});
    let plan = t.post(&format!("/api/routers/{rid}/plans"), nu, Some(&c1)).await;
    assert_eq!(plan.status, StatusCode::OK, "{:?}", plan.body);
    let pid = plan.body["planId"].as_str().unwrap().to_string();
    assert!(plan.body["changes"].as_array().unwrap().iter().any(|c| c["field"] == "profile" && c["to"] == "20M"));
    assert!(!plan.body.to_string().contains("tika-rahasia"), "password tidak boleh ada di tinjauan");

    // Akun lain tidak bisa menerapkan rencana orang lain; rencana tetap utuh untuk pembuatnya.
    let stolen = t.post(&format!("/api/plans/{pid}/apply"), json!({}), Some(&c2)).await;
    assert_eq!((stolen.status, stolen.code()), (StatusCode::GONE, "plan_expired"));
    let ok = t.post(&format!("/api/plans/{pid}/apply"), json!({}), Some(&c1)).await;
    assert_eq!((ok.status, ok.body["stepsDone"].as_u64()), (StatusCode::OK, Some(1)));
    assert!(t.world.secrets.lock().unwrap().iter().any(|r| r["name"] == "tika"));
    // Sekali pakai.
    assert_eq!(t.post(&format!("/api/plans/{pid}/apply"), json!({}), Some(&c1)).await.status, StatusCode::GONE);
    // Klien tidak punya cara mengirim langkah sendiri: id acak tidak dikenal juga 410.
    assert_eq!(t.post("/api/plans/tebakan/apply", json!({}), Some(&c1)).await.status, StatusCode::GONE);

    // Kedaluwarsa.
    let plan = t.post(&format!("/api/routers/{rid}/plans"), json!({"action": "user.soft_delete", "name": "budi"}), Some(&c1)).await;
    t.clock.fetch_add(601_000, Ordering::SeqCst);
    assert_eq!(t.post(&format!("/api/plans/{}/apply", plan.body["planId"].as_str().unwrap()), json!({}), Some(&c1)).await.status, StatusCode::GONE);
}

#[tokio::test]
async fn izin_diperiksa_ulang_saat_menerapkan_dan_router_baca_saja_ditolak() {
    let t = harness();
    let ro = t.router(false);
    t.account("operator", "role-operator");
    let c = t.login("operator").await;
    let r = t.post(&format!("/api/routers/{ro}/plans"), json!({"action": "user.soft_delete", "name": "budi"}), Some(&c)).await;
    let pid = r.body["planId"].as_str().unwrap().to_string();
    let a = t.post(&format!("/api/plans/{pid}/apply"), json!({}), Some(&c)).await;
    assert_eq!((a.status, a.code()), (StatusCode::FORBIDDEN, "forbidden"), "router tidak diizinkan ditulis");
    assert!(t.world.secrets.lock().unwrap()[0]["disabled"] == "false");

    // Peran diturunkan setelah tinjau: penerapan ditolak.
    let rw = {
        let n = telemon_core::store::NewRouter { name: "tulis".into(), host: "h".into(), port: 8728, tls_pin: None, username: "api".into(), can_write: true };
        t.telemon.add_router(&n, "pw").unwrap().id
    };
    let r = t.post(&format!("/api/routers/{rw}/plans"), json!({"action": "user.soft_delete", "name": "budi"}), Some(&c)).await;
    let pid = r.body["planId"].as_str().unwrap().to_string();
    let uid = t.auth.list_accounts(None).unwrap().into_iter().find(|a| a.username == "operator").unwrap().id;
    t.auth.change_account_role(None, &uid, "role-viewer").unwrap();
    let a = t.post(&format!("/api/plans/{pid}/apply"), json!({}), Some(&c)).await;
    assert_eq!(a.status, StatusCode::FORBIDDEN);
    assert!(t.world.secrets.lock().unwrap()[0]["disabled"] == "false", "tidak boleh tertulis");
}

#[tokio::test]
async fn validasi_rencana_dan_router_tidak_ada() {
    let t = harness();
    let rid = t.router(true);
    t.account("operator", "role-operator");
    let c = t.login("operator").await;
    let p = |b: Value| {
        let (uri, c) = (format!("/api/routers/{rid}/plans"), c.clone());
        let t = &t;
        async move { t.post(&uri, b, Some(&c)).await }
    };
    assert_eq!(p(json!({"action": "user.add", "name": "a", "password": "rahasia123", "profile": "10M"})).await.status, StatusCode::UNPROCESSABLE_ENTITY, "validasi isi = 422");
    assert_eq!(p(json!({"action": "tidak.ada"})).await.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(p(json!({"action": "user.soft_delete", "name": "hantu"})).await.status, StatusCode::UNPROCESSABLE_ENTITY);
    // Router tidak ada.
    let r = t.post("/api/routers/r-xxxx/plans", json!({"action": "user.soft_delete", "name": "budi"}), Some(&c)).await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
    // null mengosongkan kolom, tidak ada = tidak diubah.
    let e = p(json!({"action": "user.edit", "name": "budi", "comment": null, "remoteAddress": "100.64.0.9"})).await;
    assert_eq!(e.status, StatusCode::OK, "{:?}", e.body);
    let fields: Vec<&str> = e.body["changes"].as_array().unwrap().iter().map(|c| c["field"].as_str().unwrap()).collect();
    assert_eq!(fields, vec!["remote-address"], "comment sudah kosong: tidak ada perubahan");
}

#[tokio::test]
async fn password_pelanggan_hanya_untuk_izin_ubah_dan_tercatat() {
    let t = harness();
    let rid = t.router(true);
    t.telemon.with_store(|s| s.set_user_password(&format!("{rid}:budi"), "rahasia123", 1)).unwrap();
    t.account("viewer", "role-viewer");
    t.account("operator", "role-operator");
    let (cv, co) = (t.login("viewer").await, t.login("operator").await);
    let uri = format!("/api/users/{rid}%3Abudi/password");
    assert_eq!(t.get(&uri, Some(&cv)).await.status, StatusCode::FORBIDDEN);
    let r = t.get(&uri, Some(&co)).await;
    assert_eq!((r.status, r.body["password"].as_str()), (StatusCode::OK, Some("rahasia123")));
    assert_eq!(r.headers[header::CACHE_CONTROL], "no-store");
    assert_eq!(t.get(&format!("/api/users/{rid}%3Atidakada/password"), Some(&co)).await.status, StatusCode::NOT_FOUND);
    // Tampilnya password tercatat; nilainya tidak.
    let log = t.telemon.with_store(|s| s.list_audit(&telemon_core::store::AuditQuery { limit: 50, ..Default::default() })).unwrap();
    let view = log.iter().find(|l| l.entry.action == "user.password_view").expect("tercatat");
    assert_eq!((view.entry.actor.as_str(), view.entry.target.as_str()), ("operator", "budi"));
    assert!(!format!("{log:?}").contains("rahasia123"));
}

#[tokio::test]
async fn catatan_pelanggan() {
    let t = harness();
    let rid = t.router(true);
    t.telemon.import_now(&rid).await.unwrap();
    t.account("operator", "role-operator");
    t.account("viewer", "role-viewer");
    let (co, cv) = (t.login("operator").await, t.login("viewer").await);
    let uri = format!("/api/users/{rid}%3Abudi/meta");
    let body = json!({"customerName": " Budi S ", "phone": "0812", "note": "VIP", "tags": ["vip", "rt-3"]});
    assert_eq!(t.call(Method::PUT, &uri, Some(body.clone()), Some(&cv), &[], LAN).await.status, StatusCode::FORBIDDEN);
    let r = t.call(Method::PUT, &uri, Some(body), Some(&co), &[], LAN).await;
    assert_eq!((r.status, r.body["customerName"].as_str()), (StatusCode::OK, Some("Budi S")));
    let snap = t.get("/api/snapshot", Some(&cv)).await;
    assert_eq!(snap.body["metas"][format!("{rid}:budi")]["note"], "VIP");
    let many: Vec<String> = (0..11).map(|i| format!("t{i}")).collect();
    assert_eq!(t.call(Method::PUT, &uri, Some(json!({"tags": many})), Some(&co), &[], LAN).await.status, StatusCode::BAD_REQUEST);
    let hantu = format!("/api/users/{rid}%3Ahantu/meta");
    assert_eq!(t.call(Method::PUT, &hantu, Some(json!({"tags": []})), Some(&co), &[], LAN).await.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn rencana_tertunda_dibatasi_per_akun() {
    let t = harness();
    let rid = t.router(true);
    t.account("operator", "role-operator");
    let c = t.login("operator").await;
    let uri = format!("/api/routers/{rid}/plans");
    for i in 0..50 {
        let r = t.post(&uri, json!({"action": "user.soft_delete", "name": "budi"}), Some(&c)).await;
        assert_eq!(r.status, StatusCode::OK, "rencana ke-{i}");
    }
    let r = t.post(&uri, json!({"action": "user.soft_delete", "name": "budi"}), Some(&c)).await;
    assert_eq!((r.status, r.code()), (StatusCode::TOO_MANY_REQUESTS, "rate_limited"));
    // Kedaluwarsa melepas kuota.
    t.clock.fetch_add(601_000, Ordering::SeqCst);
    assert_eq!(t.post(&uri, json!({"action": "user.soft_delete", "name": "budi"}), Some(&c)).await.status, StatusCode::OK);
}

// ---------------- router ----------------

#[tokio::test]
async fn kelola_router_lewat_http() {
    let t = harness();
    t.account("operator", "role-operator");
    t.account("admin", "role-admin");
    let (co, ca) = (t.login("operator").await, t.login("admin").await);
    let add = json!({"name": "pusat", "host": "10.1.1.1", "port": 8728, "username": "api", "password": "pw-router", "allowWrite": true});
    // Operator hanya boleh melihat router.
    assert_eq!(t.post("/api/routers", add.clone(), Some(&co)).await.status, StatusCode::FORBIDDEN);
    let r = t.post("/api/routers", add.clone(), Some(&ca)).await;
    assert_eq!((r.status, r.body["canWrite"].as_bool(), r.body["tls"].as_bool()), (StatusCode::CREATED, Some(true), Some(false)));
    assert!(!r.body.to_string().contains("pw-router"));
    let id = r.body["id"].as_str().unwrap().to_string();
    // Nama ganda dan TLS tanpa sidik jari.
    assert_eq!(t.post("/api/routers", add, Some(&ca)).await.status, StatusCode::BAD_REQUEST);
    let tls = json!({"name": "aman", "host": "h", "port": 8729, "tls": true, "username": "api", "password": "pw"});
    assert_eq!(t.post("/api/routers", tls, Some(&ca)).await.status, StatusCode::BAD_REQUEST);
    let fp = "ab".repeat(32);
    let tls = json!({"name": "aman", "host": "h", "port": 8729, "tls": true, "fingerprint": fp, "username": "api", "password": "pw"});
    let r = t.post("/api/routers", tls, Some(&ca)).await;
    assert_eq!((r.status, r.body["tls"].as_bool()), (StatusCode::CREATED, Some(true)));

    // Uji sambungan: identitas dan hak akun API.
    let p = t.post("/api/routers/probe", json!({"host": "h", "port": 8728, "username": "api", "password": "x"}), Some(&ca)).await;
    assert_eq!((p.status, p.body["model"].as_str(), p.body["canWrite"].as_bool(), p.body["canReadPasswords"].as_bool()), (StatusCode::OK, Some("RB4011"), Some(true), Some(true)));
    t.reachable.store(false, Ordering::SeqCst);
    let p = t.post("/api/routers/probe", json!({"host": "h", "port": 8728, "username": "api", "password": "x"}), Some(&ca)).await;
    assert_eq!((p.status, p.code()), (StatusCode::BAD_GATEWAY, "router_unreachable"));

    // Ubah, impor, hapus.
    let e = t.call(Method::PATCH, &format!("/api/routers/{id}"), Some(json!({"name": "pusat2", "allowWrite": false})), Some(&ca), &[], LAN).await;
    assert_eq!((e.status, e.body["name"].as_str(), e.body["canWrite"].as_bool()), (StatusCode::OK, Some("pusat2"), Some(false)));
    let d = t.call(Method::DELETE, &format!("/api/routers/{id}"), None, Some(&ca), &[], LAN).await;
    assert_eq!(d.status, StatusCode::NO_CONTENT);
    assert_eq!(t.call(Method::DELETE, &format!("/api/routers/{id}"), None, Some(&ca), &[], LAN).await.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn impor_mengembalikan_hitungan_dan_profile_hapus_lokal() {
    let t = harness();
    let rid = t.router(true);
    t.account("operator", "role-operator");
    t.account("viewer", "role-viewer");
    let (co, cv) = (t.login("operator").await, t.login("viewer").await);
    let imp = |c: String| {
        let (uri, t) = (format!("/api/routers/{rid}/import"), &t);
        async move { t.post(&uri, json!({}), Some(&c)).await }
    };
    // Operator hanya boleh melihat router: impor termasuk Ubah.
    assert_eq!(imp(co.clone()).await.status, StatusCode::FORBIDDEN);
    t.account("admin", "role-admin");
    let ca = t.login("admin").await;
    let r = imp(ca.clone()).await;
    assert_eq!((r.status, r.body["users"]["added"].as_u64(), r.body["profiles"]["added"].as_u64()), (StatusCode::OK, Some(1), Some(2)));
    let r = imp(ca).await;
    assert_eq!((r.body["users"]["added"].as_u64(), r.body["users"]["updated"].as_u64()), (Some(0), Some(1)));

    // Profile: ditolak bila dipakai user; bisa dihapus (lokal) dan dipulihkan bila bebas.
    let used = format!("/api/profiles/{rid}%3A10M");
    let free = format!("/api/profiles/{rid}%3A20M");
    assert_eq!(t.post(&format!("{used}/soft-delete"), json!({}), Some(&cv)).await.status, StatusCode::FORBIDDEN);
    let r = t.post(&format!("{used}/soft-delete"), json!({}), Some(&co)).await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY, "{:?}", r.body);
    assert!(r.body["error"]["message"].as_str().unwrap().contains("masih dipakai"));
    let r = t.post(&format!("{free}/soft-delete"), json!({}), Some(&co)).await;
    assert_eq!((r.status, r.body["deletedReason"].as_str()), (StatusCode::OK, Some("manual")));
    assert!(r.body["deletedAt"].as_i64().is_some());
    let r = t.post(&format!("{free}/restore"), json!({}), Some(&co)).await;
    assert_eq!((r.status, r.body["deletedAt"].is_null()), (StatusCode::OK, true));
    assert_eq!(t.post(&format!("/api/profiles/{rid}%3Atidakada/soft-delete"), json!({}), Some(&co)).await.status, StatusCode::NOT_FOUND);
    assert_eq!(t.post("/api/profiles/tanpa-titik-dua/restore", json!({}), Some(&co)).await.status, StatusCode::NOT_FOUND);
}

// ---------------- event ----------------

#[tokio::test]
async fn daftar_event_dan_kursor() {
    let t = harness();
    let rid = t.router(false);
    let evs: Vec<telemon_core::Event> = (0..6)
        .map(|i| telemon_core::Event { at_ms: i, kind: if i % 2 == 0 { telemon_core::EventKind::Connected } else { telemon_core::EventKind::AuthFailed }, username: Some("budi".into()), address: None, caller_id: None, message: format!("m{i}") })
        .collect();
    t.telemon.with_store(|s| s.insert_events(&rid, &evs)).unwrap();
    t.account("pantau", "role-monitor");
    let c = t.login("pantau").await;
    let p1 = t.get("/api/events?limit=4", Some(&c)).await;
    assert_eq!(p1.body["events"].as_array().unwrap().len(), 4);
    let next = p1.body["nextBefore"].as_str().unwrap();
    let p2 = t.get(&format!("/api/events?limit=4&before={next}"), Some(&c)).await;
    assert_eq!(p2.body["events"].as_array().unwrap().len(), 2);
    assert!(p2.body["nextBefore"].is_null());
    let f = t.get("/api/events?type=auth_failed,tidak_dikenal&limit=50", Some(&c)).await;
    assert_eq!(f.body["events"].as_array().unwrap().len(), 3);
    assert_eq!(f.body["events"][0]["type"], "auth_failed");
    // Tanpa izin Event.
    let role = t.auth.create_role(None, &RoleInput { name: "tanpa-event".into(), description: String::new(), permissions: Permissions::default().with(Resource::Ringkasan, Level::Read) }).unwrap();
    t.account("buta", &role.id);
    let cb = t.login("buta").await;
    assert_eq!(t.get("/api/events", Some(&cb)).await.status, StatusCode::FORBIDDEN);
}

// ---------------- pengaturan dan penyimpanan ----------------

#[tokio::test]
async fn pengaturan_izin_validasi_dan_berlaku_langsung() {
    let t = harness_with(HttpConfig { login_per_minute: 1000, db_path: "/data/telemon.db".into(), ..Default::default() });
    t.account("operator", "role-operator");
    t.account("admin", "role-admin");
    t.account("budi", "role-viewer");
    let (co, ca) = (t.login("operator").await, t.login("admin").await);
    assert_eq!(t.get("/api/settings", Some(&co)).await.status, StatusCode::FORBIDDEN, "Operator tidak punya izin Pengaturan");

    let cur = t.get("/api/settings", Some(&ca)).await;
    assert_eq!(cur.status, StatusCode::OK);
    assert_eq!((cur.body["retention"]["eventDays"].as_u64(), cur.body["polling"]["sessionSeconds"].as_u64(), cur.body["remote"]["maxFailures"].as_u64()), (Some(30), Some(30), Some(5)));
    assert_eq!(cur.body["remote"]["allowPublicWrite"], false);

    let put = |b: Value| t.call(Method::PUT, "/api/settings", Some(b), Some(&ca), &[], LAN);
    // Validasi: pesan siap tampil.
    let mut bad = cur.body.clone();
    bad["polling"]["sessionSeconds"] = json!(5);
    let r = put(bad).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    assert!(r.body["error"]["message"].as_str().unwrap().contains("Interval sesi aktif"));
    // Izin tulis lewat publik tidak bisa diubah dari sesi jarak jauh.
    let mut pw = cur.body.clone();
    pw["remote"]["allowPublicWrite"] = json!(true);
    assert_eq!(put(pw).await.status, StatusCode::FORBIDDEN);

    // Perubahan sah: tersimpan, dan kebijakan akun langsung berlaku.
    let mut next = cur.body.clone();
    next["remote"]["maxFailures"] = json!(3);
    next["remote"]["sessionHours"] = json!(2);
    next["retention"]["eventDays"] = Value::Null;
    let r = put(next).await;
    assert_eq!((r.status, r.body["remote"]["maxFailures"].as_u64(), r.body["retention"]["eventDays"].is_null()), (StatusCode::OK, Some(3), true));
    assert_eq!(t.get("/api/settings", Some(&ca)).await.body["remote"]["sessionHours"], 2);
    for _ in 0..3 {
        let _ = t.post("/api/login", json!({"username": "budi", "password": "salah"}), None).await;
    }
    let locked = t.post("/api/login", json!({"username": "budi", "password": "password-uji"}), None).await;
    assert_eq!((locked.status, locked.code()), (StatusCode::TOO_MANY_REQUESTS, "locked"), "batas gagal baru (3) berlaku tanpa memulai ulang");
    t.account("pantau", "role-monitor");
    let ok = t.post("/api/login", json!({"username": "pantau", "password": "password-uji"}), None).await;
    assert!(ok.set_cookie().contains("Max-Age=7200"), "masa sesi baru (2 jam): {}", ok.set_cookie());

    // Perubahan tercatat di riwayat dengan nilai lama dan baru.
    let log = t.get("/api/audit?limit=100", Some(&ca)).await;
    let entry = log.body["entries"].as_array().unwrap().iter().find(|e| e["action"] == "settings.update").expect("tercatat");
    let ch = entry["changes"].as_array().unwrap();
    assert!(ch.iter().any(|c| c["field"] == "remote.max_failures" && c["from"] == "5" && c["to"] == "3"));
    assert!(ch.iter().any(|c| c["field"] == "retention.event_days" && c["to"] == "null"));
    // Dari aplikasi desktop (tanpa sesi) izin tulis publik boleh diubah dan langsung berlaku.
    let mut s = t.telemon.with_store(|st| st.get_settings()).unwrap();
    s.remote.allow_public_write = true;
    t.telemon.with_store(|st| st.save_settings(&s)).unwrap();
    t.st.reload_settings().unwrap();
    assert!(t.auth.policy().allow_public_write);
}

#[tokio::test]
async fn status_berbagi_mewajibkan_2fa_dan_penyimpanan_dibersihkan() {
    let t = harness();
    t.account("pantau", "role-monitor");
    t.st.set_share(true, false);
    let r = t.post("/api/login", json!({"username": "pantau", "password": "password-uji"}), None).await;
    assert_eq!(r.body["status"], "2fa_setup_required", "saklar Wajib 2FA dari desktop");
    t.st.set_share(false, false);
    assert_eq!(t.post("/api/login", json!({"username": "pantau", "password": "password-uji"}), None).await.body["status"], "ok");

    // Penyimpanan: hitungan, pratinjau, lalu bersihkan.
    let rid = t.router(false);
    let old = telemon_core::Event { at_ms: 1, kind: telemon_core::EventKind::Connected, username: None, address: None, caller_id: None, message: "lama".into() };
    t.telemon.with_store(|s| s.insert_events(&rid, &[old])).unwrap();
    t.account("admin", "role-admin");
    let ca = t.login("admin").await;
    let st = t.get("/api/storage", Some(&ca)).await;
    assert_eq!(st.status, StatusCode::OK);
    assert_eq!((st.body["events"].as_u64(), st.body["purgeable"]["events"].as_u64(), st.body["chats"].as_u64()), (Some(1), Some(1), Some(0)));
    assert!(st.body["sizeBytes"].as_u64().unwrap() > 0);
    t.account("viewer", "role-viewer");
    let cv = t.login("viewer").await;
    assert_eq!(t.post("/api/storage/purge", json!({}), Some(&cv)).await.status, StatusCode::FORBIDDEN);
    let done = t.post("/api/storage/purge", json!({}), Some(&ca)).await;
    assert_eq!((done.status, done.body["events"].as_u64()), (StatusCode::OK, Some(1)));
    assert_eq!(t.get("/api/storage", Some(&ca)).await.body["events"], 0);
    let log = t.get("/api/audit?limit=100", Some(&ca)).await;
    assert!(log.body["entries"].as_array().unwrap().iter().any(|e| e["action"] == "storage.purge"));
}

// ---------------- permukaan desktop dan berbagi LAN ----------------

const DESK_TOKEN: &str = "token-desktop-uji-0123456789abcdef";

impl T {
    /// Permintaan ke permukaan desktop (router terpisah dengan token).
    async fn desk(&self, method: Method, uri: &str, body: Option<Value>, cookie: Option<&str>, peer: &str) -> Resp {
        let mut b = Request::builder().method(method).uri(uri);
        if body.is_some() {
            b = b.header(header::CONTENT_TYPE, "application/json");
        }
        if let Some(c) = cookie {
            b = b.header(header::COOKIE, c);
        }
        let mut req = b.body(body.map_or(Body::empty(), |v| Body::from(v.to_string()))).unwrap();
        req.extensions_mut().insert(ConnectInfo(peer.parse::<SocketAddr>().unwrap()));
        let res = app_desktop(self.st.clone(), DESK_TOKEN).oneshot(req).await.unwrap();
        let (parts, body) = res.into_parts();
        let bytes = body.collect().await.unwrap().to_bytes();
        let body = serde_json::from_slice(&bytes).unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into()));
        Resp { status: parts.status, headers: parts.headers, body }
    }
}

fn desk_cookie() -> String {
    format!("telemon_desktop={DESK_TOKEN}")
}

#[tokio::test]
async fn permukaan_desktop_butuh_token_dari_loopback() {
    let t = harness();
    // Tanpa cookie: tidak ada akses, dan sesi akun tidak diterima di permukaan desktop.
    assert_eq!(t.desk(Method::GET, "/api/snapshot", None, None, LOOP).await.status, StatusCode::UNAUTHORIZED);
    t.account("admin", "role-admin");
    let acct = t.login("admin").await;
    let as_account = format!("telemon_session={acct}");
    assert_eq!(t.desk(Method::GET, "/api/snapshot", None, Some(&as_account), LOOP).await.status, StatusCode::UNAUTHORIZED);
    // Pintu masuk: token salah ditolak, benar menukar dengan cookie lalu mengalihkan ke UI.
    assert_eq!(t.desk(Method::GET, "/desktop-entry?t=salah", None, None, LOOP).await.status, StatusCode::FORBIDDEN);
    let entry = t.desk(Method::GET, &format!("/desktop-entry?t={DESK_TOKEN}"), None, None, LOOP).await;
    assert_eq!(entry.status, StatusCode::SEE_OTHER);
    let sc = entry.set_cookie();
    assert!(sc.contains(&desk_cookie()) && sc.contains("HttpOnly") && sc.contains("SameSite=Strict"), "{sc}");
    // Token benar tetapi bukan dari loopback: ditolak (server desktop seharusnya tidak terjangkau, ini lapis kedua).
    assert_eq!(t.desk(Method::GET, &format!("/desktop-entry?t={DESK_TOKEN}"), None, None, LAN).await.status, StatusCode::FORBIDDEN);
    assert_eq!(t.desk(Method::GET, "/api/snapshot", None, Some(&desk_cookie()), LAN).await.status, StatusCode::UNAUTHORIZED);
    // Dengan cookie dari loopback: pemilik.
    let h = t.desk(Method::GET, "/api/health", None, Some(&desk_cookie()), LOOP).await;
    assert_eq!((h.body["ok"].as_bool(), h.body["desktop"].as_bool()), (Some(true), Some(true)));
    assert_eq!(t.desk(Method::GET, "/api/health", None, None, LOOP).await.body["desktop"], false);
    // Permukaan jarak jauh tidak pernah menganggap siapa pun desktop, bahkan dengan cookie dan token yang sama.
    let r = t.call(Method::GET, "/api/health", None, None, &[("cookie", &desk_cookie())], LOOP).await;
    assert_eq!(r.body["desktop"], false);
    assert_eq!(t.call(Method::GET, "/desktop-entry?t=x", None, None, &[], LOOP).await.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn pemilik_desktop_punya_semua_izin_dan_tercatat_sebagai_desktop() {
    let t = harness();
    let rid = t.router(true);
    t.telemon.import_now(&rid).await.unwrap();
    let c = desk_cookie();
    for path in ["/api/accounts", "/api/roles", "/api/settings", "/api/audit", "/api/storage"] {
        assert_eq!(t.desk(Method::GET, path, None, Some(&c), LOOP).await.status, StatusCode::OK, "{path}");
    }
    // Tulis ke router; izin tulis lewat publik hanya bisa diubah oleh desktop.
    let plan = t.desk(Method::POST, &format!("/api/routers/{rid}/plans"), Some(json!({"action": "user.soft_delete", "name": "budi"})), Some(&c), LOOP).await;
    assert_eq!(plan.status, StatusCode::OK, "{:?}", plan.body);
    let ap = t.desk(Method::POST, &format!("/api/plans/{}/apply", plan.body["planId"].as_str().unwrap()), Some(json!({})), Some(&c), LOOP).await;
    assert_eq!(ap.status, StatusCode::OK);
    let mut s = t.desk(Method::GET, "/api/settings", None, Some(&c), LOOP).await.body;
    s["remote"]["allowPublicWrite"] = json!(true);
    let r = t.desk(Method::PUT, "/api/settings", Some(s), Some(&c), LOOP).await;
    assert_eq!((r.status, r.body["remote"]["allowPublicWrite"].as_bool()), (StatusCode::OK, Some(true)));
    assert!(t.auth.policy().allow_public_write, "berlaku langsung");
    let log = t.desk(Method::GET, "/api/audit?limit=50", None, Some(&c), LOOP).await;
    let entries = log.body["entries"].as_array().unwrap();
    assert!(entries.iter().any(|e| e["action"] == "user.soft_delete" && e["actor"] == "desktop"));
    assert!(entries.iter().any(|e| e["action"] == "settings.update" && e["actor"] == "desktop"));
    // Mengelola akun dari desktop: tidak terhalang aturan "akun sendiri".
    let created = t.desk(Method::POST, "/api/accounts", Some(json!({"username": "baru", "displayName": "B", "password": "password-uji", "roleId": "role-admin"})), Some(&c), LOOP).await;
    assert_eq!(created.status, StatusCode::CREATED);
    let id = created.body["id"].as_str().unwrap().to_string();
    assert_eq!(t.desk(Method::DELETE, &format!("/api/accounts/{id}"), None, Some(&c), LOOP).await.status, StatusCode::NO_CONTENT);
    // Peran bawaan boleh diedit dari desktop.
    let role = t.desk(Method::POST, "/api/roles/role-viewer/reset", Some(json!({})), Some(&c), LOOP).await;
    assert_eq!(role.status, StatusCode::OK);
}

#[tokio::test]
async fn berbagi_hanya_desktop_dan_lan_mewajibkan_login() {
    let t = harness_with(HttpConfig { login_per_minute: 1000, ..Default::default() });
    t.account("admin", "role-admin");
    let ca = t.login("admin").await;
    let c = desk_cookie();
    // Akun jarak jauh (bahkan admin) tidak boleh mengatur berbagi; permukaan jarak jauh tidak punya rute berbagi untuk desktop.
    for (m, p, b) in [(Method::GET, "/api/share", None), (Method::POST, "/api/share/lan", Some(json!({"enabled": true, "bind": "localhost", "port": 0}))), (Method::POST, "/api/share/require-2fa", Some(json!({"required": true})))] {
        let r = t.call(m, p, b, Some(&ca), &[], LAN).await;
        assert_eq!((r.status, r.code()), (StatusCode::FORBIDDEN, "forbidden"), "{p}");
    }
    // Desktop: status awal.
    let s = t.desk(Method::GET, "/api/share", None, Some(&c), LOOP).await;
    assert_eq!((s.body["lan"]["enabled"].as_bool(), s.body["lan"]["port"].as_u64(), s.body["auth"]["require2fa"].as_bool()), (Some(false), Some(7878), Some(false)));
    // Validasi.
    assert_eq!(t.desk(Method::POST, "/api/share/lan", Some(json!({"enabled": true, "bind": "lan", "port": 80})), Some(&c), LOOP).await.status, StatusCode::BAD_REQUEST);
    assert_eq!(t.desk(Method::POST, "/api/share/lan", Some(json!({"enabled": true, "bind": "semua", "port": 7878})), Some(&c), LOOP).await.status, StatusCode::BAD_REQUEST);
    // Tanpa penyedia tunnel (aplikasi tidak memasangnya): jujur 501.
    assert_eq!(t.desk(Method::POST, "/api/share/publik", Some(json!({"enabled": true, "durationHours": 8})), Some(&c), LOOP).await.status, StatusCode::NOT_IMPLEMENTED);

    // Port dipakai pihak lain: galat jelas, status tetap mati.
    let blocker = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let busy = blocker.local_addr().unwrap().port();
    if busy >= 1024 {
        let r = t.desk(Method::POST, "/api/share/lan", Some(json!({"enabled": true, "bind": "localhost", "port": busy})), Some(&c), LOOP).await;
        assert_eq!((r.status, r.code()), (StatusCode::CONFLICT, "port_in_use"), "{:?}", r.body);
        assert_eq!(t.desk(Method::GET, "/api/share", None, Some(&c), LOOP).await.body["lan"]["enabled"], false);
    }
    drop(blocker);

    // Nyalakan di port bebas: server sungguhan dengan login akun.
    let free = { let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap(); l.local_addr().unwrap().port() };
    let on = t.desk(Method::POST, "/api/share/lan", Some(json!({"enabled": true, "bind": "localhost", "port": free})), Some(&c), LOOP).await;
    assert_eq!((on.status, on.body["lan"]["enabled"].as_bool()), (StatusCode::OK, Some(true)), "{:?}", on.body);
    assert_eq!(on.body["lan"]["url"], format!("http://127.0.0.1:{free}"));
    let base = format!("http://127.0.0.1:{free}");
    let get = |path: &str, cookie: Option<String>| {
        let url = format!("{base}{path}");
        async move {
            let mut req = tokio::net::TcpStream::connect(("127.0.0.1", free)).await.unwrap();
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            let ck = cookie.map(|c| format!("Cookie: {c}\r\n")).unwrap_or_default();
            let path = url.split(&format!("{free}")).nth(1).unwrap().to_string();
            req.write_all(format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{free}\r\n{ck}Connection: close\r\n\r\n").as_bytes()).await.unwrap();
            let mut buf = String::new();
            req.read_to_string(&mut buf).await.unwrap();
            buf
        }
    };
    // Tanpa login: tidak ada data. Cookie desktop di server LAN tidak berarti apa-apa (permukaan jarak jauh).
    assert!(get("/api/snapshot", None).await.starts_with("HTTP/1.1 401"));
    assert!(get("/api/snapshot", Some(desk_cookie())).await.starts_with("HTTP/1.1 401"), "cookie desktop tidak boleh dikenali di server LAN");
    let health = get("/api/health", Some(desk_cookie())).await;
    assert!(health.contains("\"desktop\":false"), "{health}");
    assert!(get("/api/snapshot", Some(format!("telemon_session={ca}"))).await.starts_with("HTTP/1.1 200"), "akun yang login boleh");

    // Matikan: port dilepas dan tidak lagi menjawab.
    let off = t.desk(Method::POST, "/api/share/lan", Some(json!({"enabled": false, "bind": "localhost", "port": free})), Some(&c), LOOP).await;
    assert_eq!(off.body["lan"]["enabled"], false);
    assert!(tokio::net::TcpStream::connect(("127.0.0.1", free)).await.is_err(), "port harus tertutup");
    // Nyalakan lagi di port yang sama (dilepas bersih).
    let again = t.desk(Method::POST, "/api/share/lan", Some(json!({"enabled": true, "bind": "localhost", "port": free})), Some(&c), LOOP).await;
    assert_eq!(again.body["lan"]["enabled"], true, "{:?}", again.body);
    t.st.shutdown_lan().await;
    assert!(tokio::net::TcpStream::connect(("127.0.0.1", free)).await.is_err());

    // Wajib 2FA dari desktop: berlaku langsung dan bertahan (tersimpan).
    t.account("pantau", "role-monitor");
    let r = t.desk(Method::POST, "/api/share/require-2fa", Some(json!({"required": true})), Some(&c), LOOP).await;
    assert_eq!(r.body["auth"]["require2fa"], true);
    assert_eq!(t.post("/api/login", json!({"username": "pantau", "password": "password-uji"}), None).await.body["status"], "2fa_setup_required");
    assert_eq!(t.telemon.with_store(|s| s.kv_get("share.require_2fa")).unwrap().as_deref(), Some("true"));
    // Snapshot desktop memuat status asli; jarak jauh tidak memuat alamat server.
    let snap = t.desk(Method::GET, "/api/snapshot", None, Some(&c), LOOP).await;
    assert_eq!(snap.body["share"]["auth"]["require2fa"], true);
    assert!(t.get("/api/snapshot", Some(&ca)).await.body["share"]["lan"]["url"].is_null());
}

// ---------------- akses Publik (tunnel) ----------------

#[derive(Default)]
struct FakeTunnel {
    starts: Mutex<Vec<u16>>,
    stops: AtomicUsize,
    fail: Mutex<Option<String>>,
    dead: std::sync::atomic::AtomicBool,
}

impl Tunnel for FakeTunnel {
    fn start(&self, port: u16) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String, String>> + Send + '_>> {
        Box::pin(async move {
            self.starts.lock().unwrap().push(port);
            match self.fail.lock().unwrap().clone() {
                Some(e) => Err(e),
                None => Ok("https://contoh-uji.trycloudflare.com".to_string()),
            }
        })
    }
    fn stop(&self) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + '_>> {
        Box::pin(async move {
            self.stops.fetch_add(1, Ordering::SeqCst);
        })
    }
    fn alive(&self) -> std::pin::Pin<Box<dyn std::future::Future<Output = bool> + Send + '_>> {
        Box::pin(async move { !self.dead.load(Ordering::SeqCst) })
    }
}

async fn lan_on(t: &T, c: &str) -> u16 {
    let free = { let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap(); l.local_addr().unwrap().port() };
    let r = t.desk(Method::POST, "/api/share/lan", Some(json!({"enabled": true, "bind": "localhost", "port": free})), Some(c), LOOP).await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    free
}

#[tokio::test]
async fn akses_publik_syarat_status_dan_dua_fa() {
    let t = harness_with(HttpConfig { login_per_minute: 1000, ..Default::default() });
    let tunnel = Arc::new(FakeTunnel::default());
    t.st.set_tunnel(tunnel.clone());
    let c = desk_cookie();
    let on = |h: u32| json!({"enabled": true, "durationHours": h});
    let pub_call = |body: Value| t.desk(Method::POST, "/api/share/publik", Some(body), Some(&c), LOOP);

    // Akun jarak jauh tidak boleh.
    t.account("pantau", "role-monitor");
    let ca = t.login("pantau").await;
    assert_eq!(t.call(Method::POST, "/api/share/publik", Some(on(8)), Some(&ca), &[], LAN).await.status, StatusCode::FORBIDDEN);
    // Syarat: server LAN menyala; durasi sah.
    let r = pub_call(on(8)).await;
    assert_eq!((r.status, r.code()), (StatusCode::CONFLICT, "lan_off"), "{:?}", r.body);
    let port = lan_on(&t, &c).await;
    assert_eq!(pub_call(on(3)).await.status, StatusCode::BAD_REQUEST);
    assert!(tunnel.starts.lock().unwrap().is_empty(), "tunnel tidak boleh dinyalakan sebelum syarat terpenuhi");

    // Gagal menyalakan: galat jelas, status error, 2FA tidak dipaksa, bisa dicoba lagi.
    *tunnel.fail.lock().unwrap() = Some("unduhan gagal".into());
    let r = pub_call(on(8)).await;
    assert_eq!((r.status, r.code()), (StatusCode::BAD_GATEWAY, "tunnel_failed"));
    assert!(r.body["error"]["message"].as_str().unwrap().contains("unduhan gagal"));
    let s = t.desk(Method::GET, "/api/share", None, Some(&c), LOOP).await.body;
    assert_eq!((s["publik"]["status"].as_str(), s["publik"]["error"].as_str()), (Some("error"), Some("unduhan gagal")));
    assert_eq!(t.post("/api/login", json!({"username": "pantau", "password": "password-uji"}), None).await.body["status"], "ok", "belum publik: 2FA tidak dipaksa");
    assert_eq!(tunnel.stops.load(Ordering::SeqCst), 1, "tunnel setengah jalan dibersihkan");

    // Berhasil: URL, kedaluwarsa, 2FA wajib untuk semua, URL tidak terlihat akun jarak jauh.
    *tunnel.fail.lock().unwrap() = None;
    let r = pub_call(on(8)).await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(*tunnel.starts.lock().unwrap().last().unwrap(), port, "tunnel mengarah ke port server LAN");
    let p = &r.body["publik"];
    assert_eq!((p["status"].as_str(), p["url"].as_str(), p["durationHours"].as_u64()), (Some("on"), Some("https://contoh-uji.trycloudflare.com"), Some(8)));
    assert!(p["expiresAt"].as_i64().unwrap() > T0 + 7 * 3_600_000);
    assert_eq!(t.post("/api/login", json!({"username": "pantau", "password": "password-uji"}), None).await.body["status"], "2fa_setup_required", "selama publik aktif 2FA wajib");
    let remote_view = t.get("/api/snapshot", Some(&ca)).await.body;
    assert_eq!(remote_view["share"]["publik"]["status"], "on");
    assert!(remote_view["share"]["publik"]["url"].is_null(), "alamat tunnel tidak ditampilkan ke akun jarak jauh");
    // Ganda ditolak.
    assert_eq!(pub_call(on(8)).await.status, StatusCode::CONFLICT);

    // Matikan: tunnel dihentikan dan 2FA tidak lagi dipaksa.
    let stops = tunnel.stops.load(Ordering::SeqCst);
    let off = pub_call(json!({"enabled": false})).await;
    assert_eq!(off.body["publik"]["status"], "off");
    assert_eq!(tunnel.stops.load(Ordering::SeqCst), stops + 1);
    assert_eq!(t.post("/api/login", json!({"username": "pantau", "password": "password-uji"}), None).await.body["status"], "ok");
    // Mematikan yang sudah mati tidak menghentikan tunnel lagi.
    pub_call(json!({"enabled": false})).await;
    assert_eq!(tunnel.stops.load(Ordering::SeqCst), stops + 1);

    // Mematikan server LAN saat publik aktif memutus tunnel.
    assert_eq!(pub_call(on(4)).await.status, StatusCode::OK);
    let stops = tunnel.stops.load(Ordering::SeqCst);
    t.desk(Method::POST, "/api/share/lan", Some(json!({"enabled": false, "bind": "localhost", "port": port})), Some(&c), LOOP).await;
    assert_eq!(tunnel.stops.load(Ordering::SeqCst), stops + 1, "tunnel harus berhenti bersama server LAN");
    assert_eq!(t.desk(Method::GET, "/api/share", None, Some(&c), LOOP).await.body["publik"]["status"], "off");
}

#[tokio::test(start_paused = true)]
async fn tunnel_yang_mati_sendiri_tidak_ditampilkan_aktif() {
    let t = harness();
    let tunnel = Arc::new(FakeTunnel::default());
    t.st.set_tunnel(tunnel.clone());
    let c = desk_cookie();
    lan_on(&t, &c).await;
    t.account("pantau", "role-monitor");
    let r = t.desk(Method::POST, "/api/share/publik", Some(json!({"enabled": true, "durationHours": 8})), Some(&c), LOOP).await;
    assert_eq!(r.body["publik"]["status"], "on");
    for _ in 0..5 {
        tokio::task::yield_now().await;
    }
    tunnel.dead.store(true, Ordering::SeqCst);
    tokio::time::advance(Duration::from_secs(11)).await;
    tokio::time::sleep(Duration::from_millis(50)).await;
    for _ in 0..20 {
        tokio::task::yield_now().await;
    }
    let s = t.desk(Method::GET, "/api/share", None, Some(&c), LOOP).await.body;
    assert_eq!(s["publik"]["status"], "error");
    assert!(s["publik"]["error"].as_str().unwrap().contains("terputus"));
    assert_eq!(tunnel.stops.load(Ordering::SeqCst), 1);
    assert!(!t.auth.policy().require_2fa);
    t.st.shutdown_lan().await;
}

#[tokio::test(start_paused = true)]
async fn akses_publik_mati_otomatis_setelah_durasi() {
    let t = harness();
    let tunnel = Arc::new(FakeTunnel::default());
    t.st.set_tunnel(tunnel.clone());
    let c = desk_cookie();
    lan_on(&t, &c).await;
    let r = t.desk(Method::POST, "/api/share/publik", Some(json!({"enabled": true, "durationHours": 1})), Some(&c), LOOP).await;
    assert_eq!(r.body["publik"]["status"], "on", "{:?}", r.body);
    // Tugas timer baru terdaftar saat pertama dijadwalkan: beri kesempatan sebelum memajukan jam virtual.
    for _ in 0..5 {
        tokio::task::yield_now().await;
    }
    tokio::time::advance(Duration::from_secs(3599)).await;
    tokio::task::yield_now().await;
    assert_eq!(t.desk(Method::GET, "/api/share", None, Some(&c), LOOP).await.body["publik"]["status"], "on", "belum lewat satu jam");
    tokio::time::advance(Duration::from_secs(2)).await;
    // Beri kesempatan tugas timer berjalan (jam virtual: tidur singkat memajukan waktu bila semua tugas menganggur).
    tokio::time::sleep(Duration::from_millis(50)).await;
    for _ in 0..20 {
        tokio::task::yield_now().await;
    }
    let s = t.desk(Method::GET, "/api/share", None, Some(&c), LOOP).await.body;
    assert_eq!(s["publik"]["status"], "off", "harus mati otomatis setelah durasi");
    assert_eq!(tunnel.stops.load(Ordering::SeqCst), 1);
    t.st.shutdown_lan().await;
}

// ---------------- tata letak widget Ringkasan ----------------

#[tokio::test]
async fn tata_letak_dashboard_global_dengan_izin_dan_validasi() {
    let t = harness();
    let c = desk_cookie();
    let widget = |id: &str| json!({ "id": id, "type": "stat-router-online", "x": 0, "y": 0, "w": 4, "h": 2, "config": { "router": "r1" } });
    let layout = json!({ "version": 1, "widgets": [widget("w1"), widget("w2")] });

    // Belum ada tata letak tersimpan: null, UI memakai susunan bawaan.
    let g = t.desk(Method::GET, "/api/dashboard", None, Some(&c), LOOP).await;
    assert_eq!((g.status, g.body["layout"].is_null()), (StatusCode::OK, true));

    // Simpan dari desktop; dibaca kembali sama persis.
    let r = t.desk(Method::PUT, "/api/dashboard", Some(json!({ "layout": layout })), Some(&c), LOOP).await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(t.desk(Method::GET, "/api/dashboard", None, Some(&c), LOOP).await.body["layout"], layout);

    // Bentuk yang salah ditolak dan tidak menimpa yang tersimpan.
    let long_cfg = json!({ "k": "x".repeat(3000) });
    let too_many: Vec<Value> = (0..61).map(|i| widget(&format!("w{i}"))).collect();
    let bads = [
        ("versi", json!({ "version": 2, "widgets": [] })),
        ("id ganda", json!({ "version": 1, "widgets": [widget("a"), widget("a")] })),
        ("id spasi", json!({ "version": 1, "widgets": [widget("a b")] })),
        ("jenis huruf besar", json!({ "version": 1, "widgets": [{ "id": "a", "type": "Stat", "x": 0, "y": 0, "w": 1, "h": 1 }] })),
        ("terlalu tinggi", json!({ "version": 1, "widgets": [{ "id": "a", "type": "stat", "x": 0, "y": 0, "w": 1, "h": 2001 }] })),
        ("lebar nol", json!({ "version": 1, "widgets": [{ "id": "a", "type": "stat", "x": 0, "y": 0, "w": 0, "h": 1 }] })),
        ("x negatif", json!({ "version": 1, "widgets": [{ "id": "a", "type": "stat", "x": -1, "y": 0, "w": 1, "h": 1 }] })),
        ("config besar", json!({ "version": 1, "widgets": [{ "id": "a", "type": "stat", "x": 0, "y": 0, "w": 1, "h": 1, "config": long_cfg }] })),
        ("config bukan objek", json!({ "version": 1, "widgets": [{ "id": "a", "type": "stat", "x": 0, "y": 0, "w": 1, "h": 1, "config": [1] }] })),
        ("terlalu banyak", json!({ "version": 1, "widgets": too_many })),
        ("tanpa daftar", json!({ "version": 1 })),
    ];
    for (why, bad) in bads {
        let r = t.desk(Method::PUT, "/api/dashboard", Some(json!({ "layout": bad })), Some(&c), LOOP).await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{why}: {:?}", r.body);
    }
    assert_eq!(t.desk(Method::GET, "/api/dashboard", None, Some(&c), LOOP).await.body["layout"], layout, "tata letak tidak berubah oleh isi yang ditolak");
    // Jenis widget yang tak dikenal diterima: daftar jenis dikelola UI.
    let future = json!({ "version": 1, "widgets": [{ "id": "z", "type": "jenis-masa-depan", "x": 0, "y": 0, "w": 3, "h": 3 }] });
    assert_eq!(t.desk(Method::PUT, "/api/dashboard", Some(json!({ "layout": future })), Some(&c), LOOP).await.status, StatusCode::OK);
    // Widget dari extension (ADR 0028): `ext:<id>:<jenis>`; bentuk lain dengan `:` atau `.` ditolak.
    let ext = |ty: &str| json!({ "version": 1, "widgets": [{ "id": "e", "type": ty, "x": 0, "y": 0, "w": 3, "h": 3 }] });
    assert_eq!(t.desk(Method::PUT, "/api/dashboard", Some(json!({ "layout": ext("ext:net.teliti.demo:hello") })), Some(&c), LOOP).await.status, StatusCode::OK);
    for bad in ["ext:net.teliti.demo", "ext::hello", "ext:a:b:c", "ext:Net.Demo:hello", "ext:a b:c", "net.teliti.demo:hello", &format!("ext:{}:x", "a".repeat(140))] {
        let r = t.desk(Method::PUT, "/api/dashboard", Some(json!({ "layout": ext(bad) })), Some(&c), LOOP).await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{bad}");
    }
    t.desk(Method::PUT, "/api/dashboard", Some(json!({ "layout": layout })), Some(&c), LOOP).await;

    // Akun jarak jauh: tata letak sama untuk semua (global). Viewer dan Operator hanya melihat; Administrator boleh mengubah.
    t.account("viewer", "role-viewer");
    t.account("operator", "role-operator");
    t.account("admin", "role-admin");
    let (cv, co, ca) = (t.login("viewer").await, t.login("operator").await, t.login("admin").await);
    for (who, ck) in [("viewer", &cv), ("operator", &co), ("admin", &ca)] {
        let g = t.call(Method::GET, "/api/dashboard", None, Some(ck), &[], LAN).await;
        assert_eq!((g.status, g.body["layout"].clone()), (StatusCode::OK, layout.clone()), "{who} membaca tata letak yang sama");
    }
    for (who, ck) in [("viewer", &cv), ("operator", &co)] {
        let r = t.call(Method::PUT, "/api/dashboard", Some(json!({ "layout": { "version": 1, "widgets": [] } })), Some(ck), &[], LAN).await;
        assert_eq!((r.status, r.code()), (StatusCode::FORBIDDEN, "forbidden"), "{who} tidak boleh mengubah");
    }
    let mine = json!({ "version": 1, "widgets": [widget("dari-admin")] });
    let r = t.call(Method::PUT, "/api/dashboard", Some(json!({ "layout": mine })), Some(&ca), &[], LAN).await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(t.call(Method::GET, "/api/dashboard", None, Some(&cv), &[], LAN).await.body["layout"], mine, "perubahan admin terlihat oleh akun lain");

    // Kembalikan ke bawaan.
    let r = t.call(Method::PUT, "/api/dashboard", Some(json!({ "layout": null })), Some(&ca), &[], LAN).await;
    assert_eq!(r.status, StatusCode::OK);
    assert!(t.call(Method::GET, "/api/dashboard", None, Some(&cv), &[], LAN).await.body["layout"].is_null());
}

// ---------------- bandwidth ----------------

#[tokio::test]
async fn bandwidth_antarmuka_deret_waktu_validasi_dan_izin() {
    let t = harness();
    let rid = t.router(false);
    let c = desk_cookie();
    // Waktu sampel diselaraskan ke batas ember 6 menit (rentang 24 jam) supaya hasil tidak bergantung pada kapan tes jalan.
    let base = (t.st.now() / 360_000) * 360_000 - 360_000;
    let ifc = |n: &str, running: bool| telemon_core::traffic::IfaceInfo { name: n.into(), kind: "ether".into(), running };
    let smp = |i: &str, rx: i64, tx: i64, sessions: Option<i64>| telemon_core::traffic::Sample { iface: i.into(), rx_bps: rx, tx_bps: tx, sessions };
    t.telemon
        .with_store(|s| {
            s.insert_traffic(&rid, base + 10_000, &[ifc("ether1", true), ifc("@pppoe", true)], &[smp("ether1", 1_000, 2_000, None), smp("@pppoe", 4_000, 40_000, Some(8))]).unwrap();
            s.insert_traffic(&rid, base + 70_000, &[ifc("ether1", true), ifc("@pppoe", true)], &[smp("ether1", 3_000, 4_000, None), smp("@pppoe", 6_000, 60_000, Some(10))]).unwrap();
        });
    let get = |path: String| {
        let (t, c) = (&t, &c);
        async move { t.desk(Method::GET, &path, None, Some(c), LOOP).await }
    };

    // Daftar antarmuka + pengaturan yang relevan.
    let l = get(format!("/api/traffic/interfaces?router={rid}")).await;
    assert_eq!(l.status, StatusCode::OK, "{:?}", l.body);
    let names: Vec<&str> = l.body["interfaces"].as_array().unwrap().iter().map(|i| i["name"].as_str().unwrap()).collect();
    assert_eq!(names, vec!["@pppoe", "ether1"]);
    assert_eq!((l.body["enabled"].as_bool(), l.body["intervalSeconds"].as_i64()), (Some(true), Some(60)));
    assert_eq!(get("/api/traffic/interfaces?router=tidak-ada".into()).await.status, StatusCode::NOT_FOUND);

    // Deret satu antarmuka: rata-rata per ember; ember minimal sama dengan interval (60 detik) sehingga dua sampel terpisah.
    let s = get(format!("/api/traffic/series?router={rid}&iface=ether1&range=1h")).await;
    assert_eq!(s.status, StatusCode::OK, "{:?}", s.body);
    let pts = s.body["points"].as_array().unwrap();
    let vals: Vec<(i64, i64)> = pts.iter().map(|p| (p["rxBps"].as_i64().unwrap(), p["txBps"].as_i64().unwrap())).collect();
    assert_eq!(vals, vec![(1_000, 2_000), (3_000, 4_000)]);
    assert!(pts[0].get("sessions").is_none());
    assert_eq!(s.body["bucketSeconds"], 60);

    // Antarmuka virtual PPPoE: ada jumlah sesi; semua router = total pelanggan.
    let p = get(format!("/api/traffic/series?router={rid}&iface=%40pppoe&range=24h")).await;
    assert_eq!(p.body["bucketSeconds"], 360);
    assert_eq!(p.body["points"].as_array().unwrap().len(), 1, "rentang 24 jam: ember 6 menit menggabungkan dua sampel");
    assert_eq!(p.body["points"][0]["sessions"], 9.0);
    let all = get("/api/traffic/series?router=all&iface=%40pppoe&range=1h".into()).await;
    assert_eq!(all.status, StatusCode::OK, "{:?}", all.body);
    assert_eq!(all.body["points"].as_array().unwrap().len(), 2);
    // Total lintas router hanya untuk @pppoe (antarmuka fisik terhitung ganda).
    assert_eq!(get("/api/traffic/series?router=all&iface=ether1&range=1h".into()).await.status, StatusCode::BAD_REQUEST);
    for bad in ["range=2h", "range=", "iface=&range=1h"] {
        let q = if bad.starts_with("iface") { format!("/api/traffic/series?router={rid}&{bad}") } else { format!("/api/traffic/series?router={rid}&iface=ether1&{bad}") };
        assert_eq!(get(q).await.status, StatusCode::BAD_REQUEST, "{bad}");
    }
    assert_eq!(get("/api/traffic/series?router=tidak-ada&iface=ether1&range=1h".into()).await.status, StatusCode::NOT_FOUND);

    // Interval di Pengaturan menentukan lebar ember minimum, dan bisa dibaca/diubah lewat /api/settings.
    let mut cfg = t.desk(Method::GET, "/api/settings", None, Some(&c), LOOP).await.body;
    assert_eq!((cfg["bandwidth"]["enabled"].as_bool(), cfg["bandwidth"]["intervalSeconds"].as_i64(), cfg["bandwidth"]["retentionDays"].as_i64()), (Some(true), Some(60), Some(7)));
    cfg["bandwidth"]["intervalSeconds"] = json!(300);
    let r = t.desk(Method::PUT, "/api/settings", Some(cfg.clone()), Some(&c), LOOP).await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(get(format!("/api/traffic/series?router={rid}&iface=ether1&range=1h")).await.body["bucketSeconds"], 300);
    cfg["bandwidth"]["intervalSeconds"] = json!(5);
    assert_eq!(t.desk(Method::PUT, "/api/settings", Some(cfg), Some(&c), LOOP).await.status, StatusCode::BAD_REQUEST, "interval di luar 10-3600 ditolak");

    // Izin: perlu Router Lihat. Peran yang hanya punya Ringkasan ditolak.
    let role = t.auth.create_role(None, &RoleInput { name: "tanpa-router".into(), description: String::new(), permissions: Permissions::default().with(Resource::Ringkasan, Level::Read) }).unwrap();
    t.account("nirrouter", &role.id);
    let cn = t.login("nirrouter").await;
    for path in [format!("/api/traffic/interfaces?router={rid}"), format!("/api/traffic/series?router={rid}&iface=ether1&range=1h")] {
        let r = t.call(Method::GET, &path, None, Some(&cn), &[], LAN).await;
        assert_eq!((r.status, r.code()), (StatusCode::FORBIDDEN, "forbidden"), "{path}");
    }
    t.account("pemantau", "role-monitor");
    let cm = t.login("pemantau").await;
    assert_eq!(t.call(Method::GET, &format!("/api/traffic/series?router={rid}&iface=ether1&range=1h"), None, Some(&cm), &[], LAN).await.status, StatusCode::OK);
}

// ---------------- 2FA untuk peran berizin Ubah ----------------

#[tokio::test]
async fn demo_boleh_melonggarkan_2fa_peran_ubah_tetapi_publik_dan_saklar_memaksa() {
    // Bawaan: Administrator diminta mendaftarkan 2FA.
    let t = harness();
    t.account("admin", "role-admin");
    let r = t.post("/api/login", json!({"username": "admin", "password": "password-uji"}), None).await;
    assert_eq!(r.body["status"], "2fa_setup_required", "{:?}", r.body);

    // Server demo: masuk langsung, dan izin Ubah Ringkasan berlaku.
    let t = harness_with(HttpConfig { relax_update_2fa: true, ..Default::default() });
    t.account("admin", "role-admin");
    let r = t.post("/api/login", json!({"username": "admin", "password": "password-uji"}), None).await;
    assert_eq!(r.body["status"], "ok", "{:?}", r.body);
    let ck = r.cookie().expect("cookie");
    let put = t.call(Method::PUT, "/api/dashboard", Some(json!({ "layout": null })), Some(&ck), &[], LAN).await;
    assert_eq!(put.status, StatusCode::OK, "{:?}", put.body);

    // Saklar Wajib 2FA tetap memaksa, walau dilonggarkan.
    t.st.set_share(true, false);
    t.account("admin2", "role-admin");
    let r = t.post("/api/login", json!({"username": "admin2", "password": "password-uji"}), None).await;
    assert_eq!(r.body["status"], "2fa_setup_required", "saklar pemilik tetap memaksa: {:?}", r.body);
}

// ---------------- pembaruan aplikasi ----------------

#[derive(Default)]
struct FakeUpdater {
    /// Versi yang ditawarkan per kanal.
    offer: Mutex<std::collections::HashMap<String, String>>,
    checks: Mutex<Vec<String>>,
    installs: AtomicUsize,
    check_error: Mutex<Option<String>>,
    install_error: Mutex<Option<String>>,
}

type Fut<'a, T> = std::pin::Pin<Box<dyn std::future::Future<Output = T> + Send + 'a>>;

impl Updater for FakeUpdater {
    fn current_version(&self) -> String {
        "0.1.0".into()
    }
    fn check(&self, channel: &str) -> Fut<'_, Result<Option<UpdateInfo>, String>> {
        let channel = channel.to_string();
        Box::pin(async move {
            self.checks.lock().unwrap().push(channel.clone());
            if let Some(e) = self.check_error.lock().unwrap().clone() {
                return Err(e);
            }
            Ok(self.offer.lock().unwrap().get(&channel).map(|v| UpdateInfo { version: v.clone(), notes: Some("catatan rilis".into()), date: None }))
        })
    }
    fn install(&self) -> Fut<'_, Result<(), String>> {
        Box::pin(async move {
            if let Some(e) = self.install_error.lock().unwrap().clone() {
                return Err(e);
            }
            self.installs.fetch_add(1, Ordering::SeqCst);
            Ok(())
        })
    }
}

fn update_harness(name: &str) -> (T, Arc<FakeUpdater>, PathBuf) {
    let dir = std::env::temp_dir().join(format!("telemon-upd-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let t = harness_with(HttpConfig { db_path: dir.join("telemon.db").display().to_string(), ..Default::default() });
    let u = Arc::new(FakeUpdater::default());
    t.st.set_updater(u.clone());
    (t, u, dir)
}

use std::path::PathBuf;

#[tokio::test]
async fn pembaruan_hanya_desktop_dan_tanpa_penyedia_tidak_didukung() {
    let t = harness();
    let c = desk_cookie();
    let g = t.desk(Method::GET, "/api/update", None, Some(&c), LOOP).await;
    assert_eq!((g.status, g.body["supported"].as_bool()), (StatusCode::OK, Some(false)));
    assert_eq!(t.desk(Method::POST, "/api/update/check", Some(json!({})), Some(&c), LOOP).await.status, StatusCode::NOT_IMPLEMENTED);
    assert_eq!(t.desk(Method::POST, "/api/update/install", Some(json!({})), Some(&c), LOOP).await.status, StatusCode::NOT_IMPLEMENTED);
    // Akun jarak jauh, bahkan admin, tidak boleh memeriksa atau memasang.
    t.account("admin", "role-admin");
    let ca = t.login("admin").await;
    for (m, p, b) in [(Method::GET, "/api/update", None), (Method::POST, "/api/update/check", Some(json!({}))), (Method::POST, "/api/update/install", Some(json!({}))), (Method::POST, "/api/update/channel", Some(json!({"channel": "rc"})))] {
        let r = t.call(m, p, b, Some(&ca), &[], LAN).await;
        assert_eq!((r.status, r.code()), (StatusCode::FORBIDDEN, "forbidden"), "{p}");
    }
}

#[tokio::test]
async fn periksa_kanal_dan_status() {
    let (t, u, _dir) = update_harness("check");
    let c = desk_cookie();
    let call = |m: Method, p: &'static str, b: Option<Value>| t.desk(m, p, b, Some(&c), LOOP);
    let g = call(Method::GET, "/api/update", None).await.body;
    assert_eq!((g["supported"].as_bool(), g["currentVersion"].as_str(), g["channel"].as_str(), g["available"].is_null(), g["checkedAt"].is_null()), (Some(true), Some("0.1.0"), Some("stable"), true, true));
    // Tidak ada pembaruan.
    let r = call(Method::POST, "/api/update/check", Some(json!({}))).await;
    assert_eq!((r.status, r.body["available"].is_null(), r.body["checkedAt"].is_null()), (StatusCode::OK, true, false));
    // Ada di kanal stabil.
    u.offer.lock().unwrap().insert("stable".into(), "0.2.0".into());
    u.offer.lock().unwrap().insert("rc".into(), "0.3.0-rc.1".into());
    let r = call(Method::POST, "/api/update/check", Some(json!({}))).await;
    assert_eq!((r.body["available"]["version"].as_str(), r.body["available"]["notes"].as_str()), (Some("0.2.0"), Some("catatan rilis")));
    // Ganti kanal: hasil lama dihapus, pemeriksaan berikutnya memakai kanal baru, dan pilihan bertahan.
    assert_eq!(call(Method::POST, "/api/update/channel", Some(json!({"channel": "beta"}))).await.status, StatusCode::BAD_REQUEST);
    let r = call(Method::POST, "/api/update/channel", Some(json!({"channel": "rc"}))).await;
    assert_eq!((r.body["channel"].as_str(), r.body["available"].is_null()), (Some("rc"), true));
    assert_eq!(call(Method::POST, "/api/update/check", Some(json!({}))).await.body["available"]["version"], "0.3.0-rc.1");
    assert_eq!(u.checks.lock().unwrap().as_slice(), ["stable", "stable", "rc"]);
    assert_eq!(t.telemon.with_store(|s| s.kv_get("update.channel")).unwrap().as_deref(), Some("rc"));
    // Gagal memeriksa (mis. tanpa internet): galat jelas, status memuat galatnya, berhasil berikutnya membersihkan.
    *u.check_error.lock().unwrap() = Some("tanpa internet".into());
    let r = call(Method::POST, "/api/update/check", Some(json!({}))).await;
    assert_eq!((r.status, r.code()), (StatusCode::BAD_GATEWAY, "update_check_failed"));
    assert!(r.body["error"]["message"].as_str().unwrap().contains("tanpa internet"));
    assert_eq!(call(Method::GET, "/api/update", None).await.body["error"], "tanpa internet");
    *u.check_error.lock().unwrap() = None;
    assert!(call(Method::POST, "/api/update/check", Some(json!({}))).await.body["error"].is_null());
}

#[tokio::test]
async fn pasang_mencadangkan_dulu_dan_membatasi_jumlah_cadangan() {
    let (t, u, dir) = update_harness("install");
    let rid = t.router(false);
    t.telemon.with_store(|s| s.insert_events(&rid, &[telemon_core::Event { at_ms: 1, kind: telemon_core::EventKind::Connected, username: None, address: None, caller_id: None, message: "m".into() }])).unwrap();
    let c = desk_cookie();
    let post = |p: &'static str| t.desk(Method::POST, p, Some(json!({})), Some(&c), LOOP);
    // Belum diperiksa: tidak ada yang dipasang.
    let r = post("/api/update/install").await;
    assert_eq!((r.status, r.code()), (StatusCode::CONFLICT, "no_update"));
    assert_eq!(u.installs.load(Ordering::SeqCst), 0);

    u.offer.lock().unwrap().insert("stable".into(), "0.2.0".into());
    post("/api/update/check").await;
    let r = post("/api/update/install").await;
    assert_eq!((r.status, r.body["installed"].as_str()), (StatusCode::OK, Some("0.2.0")), "{:?}", r.body);
    assert_eq!(u.installs.load(Ordering::SeqCst), 1);
    // Cadangan ada, utuh, dan bernama jelas.
    let backups: Vec<_> = std::fs::read_dir(dir.join("backups")).unwrap().flatten().map(|e| e.path()).collect();
    assert_eq!(backups.len(), 1);
    assert!(backups[0].file_name().unwrap().to_string_lossy().contains("sebelum-0.2.0"), "{backups:?}");
    assert_eq!(r.body["backup"].as_str().unwrap(), backups[0].display().to_string());
    assert_eq!(telemon_core::store::Store::open(&backups[0]).unwrap().count_events().unwrap(), 1, "cadangan harus bisa dibuka dan berisi data");

    // Lebih banyak pasang: hanya KEEP_BACKUPS terbaru yang disimpan.
    for v in ["0.2.1", "0.2.2", "0.2.3", "0.2.4"] {
        u.offer.lock().unwrap().insert("stable".into(), v.into());
        t.clock.fetch_add(1000, Ordering::SeqCst);
        post("/api/update/check").await;
        assert_eq!(post("/api/update/install").await.status, StatusCode::OK);
    }
    let mut names: Vec<String> = std::fs::read_dir(dir.join("backups")).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into()).collect();
    names.sort();
    assert_eq!(names.len(), telemon_http::KEEP_BACKUPS);
    assert!(names.last().unwrap().contains("0.2.4") && !names.iter().any(|n| n.contains("sebelum-0.2.0")), "{names:?}");
}

#[tokio::test]
async fn gagal_cadangan_membatalkan_pasang_dan_gagal_pasang_menjaga_data() {
    let (t, u, dir) = update_harness("fail");
    let c = desk_cookie();
    let post = |p: &'static str| t.desk(Method::POST, p, Some(json!({})), Some(&c), LOOP);
    u.offer.lock().unwrap().insert("stable".into(), "0.2.0".into());
    post("/api/update/check").await;
    // Folder cadangan tidak bisa dibuat (sudah ada berkas bernama `backups`): pasang dibatalkan.
    std::fs::write(dir.join("backups"), b"bukan folder").unwrap();
    let r = post("/api/update/install").await;
    assert_eq!(r.status, StatusCode::INTERNAL_SERVER_ERROR, "{:?}", r.body);
    assert_eq!(u.installs.load(Ordering::SeqCst), 0, "tanpa cadangan tidak boleh memasang");
    std::fs::remove_file(dir.join("backups")).unwrap();
    // Gagal memasang: galat jelas memuat lokasi cadangan; pembaruan tetap tersedia untuk dicoba lagi.
    *u.install_error.lock().unwrap() = Some("tanda tangan tidak valid".into());
    let r = post("/api/update/install").await;
    assert_eq!((r.status, r.code()), (StatusCode::BAD_GATEWAY, "update_install_failed"));
    let m = r.body["error"]["message"].as_str().unwrap();
    assert!(m.contains("tanda tangan tidak valid") && m.contains("cadangan") && m.contains("backups"), "{m}");
    assert_eq!(t.desk(Method::GET, "/api/update", None, Some(&c), LOOP).await.body["available"]["version"], "0.2.0");
    *u.install_error.lock().unwrap() = None;
    assert_eq!(post("/api/update/install").await.status, StatusCode::OK);
}

#[tokio::test(start_paused = true)]
async fn pemeriksaan_otomatis_setelah_dibuka_lalu_tiap_24_jam() {
    let (t, u, _dir) = update_harness("auto");
    u.offer.lock().unwrap().insert("stable".into(), "0.2.0".into());
    t.st.start_update_checks();
    for _ in 0..5 {
        tokio::task::yield_now().await;
    }
    assert!(u.checks.lock().unwrap().is_empty(), "tidak langsung saat dibuka");
    tokio::time::advance(Duration::from_secs(21)).await;
    tokio::time::sleep(Duration::from_millis(10)).await;
    assert_eq!(u.checks.lock().unwrap().len(), 1, "sekali setelah jeda awal");
    assert_eq!(t.desk(Method::GET, "/api/update", None, Some(&desk_cookie()), LOOP).await.body["available"]["version"], "0.2.0");
    tokio::time::advance(Duration::from_secs(24 * 3600)).await;
    tokio::time::sleep(Duration::from_millis(10)).await;
    assert_eq!(u.checks.lock().unwrap().len(), 2, "lalu tiap 24 jam");
    t.st.stop_update_checks();
    tokio::time::advance(Duration::from_secs(48 * 3600)).await;
    tokio::time::sleep(Duration::from_millis(10)).await;
    assert_eq!(u.checks.lock().unwrap().len(), 2, "berhenti setelah dihentikan");
}

// ---------------- UI statis ----------------

#[tokio::test]
async fn ui_statis_dengan_fallback_spa_dan_csp() {
    let dir = std::env::temp_dir().join(format!("telemon-ui-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("assets")).unwrap();
    std::fs::write(dir.join("index.html"), "<!doctype html><title>Telemon</title>").unwrap();
    std::fs::write(dir.join("assets/app.js"), "console.log(1)").unwrap();
    let t = harness_with(HttpConfig { static_dir: Some(dir.clone()), ..Default::default() });
    let r = t.get("/", None).await;
    assert_eq!(r.status, StatusCode::OK);
    assert!(r.body.as_str().unwrap().contains("Telemon"));
    assert!(r.headers[header::CONTENT_SECURITY_POLICY].to_str().unwrap().contains("frame-ancestors 'none'"));
    assert!(t.get("/assets/app.js", None).await.body.as_str().unwrap().contains("console.log"));
    // Rute SPA tanpa berkas: kembali ke index.html.
    assert!(t.get("/user/budi/edit", None).await.body.as_str().unwrap().contains("Telemon"));
    // API tetap JSON, bukan halaman UI, dan tanpa CSP.
    let r = t.get("/api/tidak-ada", None).await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
    assert_eq!(r.body["error"]["code"], "not_found");
    assert!(r.headers.get(header::CONTENT_SECURITY_POLICY).is_none());
    // Penelusuran jalur keluar folder tidak boleh membaca berkas lain.
    let r = t.get("/..%2f..%2fetc%2fpasswd", None).await;
    assert!(!r.body.to_string().contains("root:"));
    std::fs::remove_dir_all(dir).unwrap();
}

// ---------------- WebSocket ----------------

#[tokio::test]
async fn websocket_event_live_dengan_cookie_dan_izin() {
    use futures_util::StreamExt;
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;

    let t = harness();
    let rid = t.router(false);
    t.account("pantau", "role-monitor");
    let role = t.auth.create_role(None, &RoleInput { name: "tanpa-event".into(), description: String::new(), permissions: Permissions::default().with(Resource::Ringkasan, Level::Read) }).unwrap();
    t.account("buta", &role.id);
    let token = |u: &str| match t.auth.login(u, "password-uji", Via::Lan, "uji", "ip").unwrap() {
        LoginOutcome::Ok { token, .. } => token,
        other => panic!("{other:?}"),
    };
    let (tok, tok_buta) = (token("pantau"), token("buta"));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let st = t.st.clone();
    tokio::spawn(async move { telemon_http::serve_on(st, listener, std::future::pending()).await });

    let connect = |cookie: Option<&str>, origin: Option<&str>| {
        let mut req = format!("ws://{addr}/api/ws").into_client_request().unwrap();
        if let Some(c) = cookie {
            req.headers_mut().insert("cookie", format!("telemon_session={c}").parse().unwrap());
        }
        if let Some(o) = origin {
            req.headers_mut().insert("origin", o.parse().unwrap());
        }
        tokio_tungstenite::connect_async(req)
    };
    // Tanpa cookie, asal asing, atau tanpa izin Event: ditolak sebelum upgrade.
    assert!(connect(None, None).await.is_err());
    assert!(connect(Some(&tok), Some("http://evil.example")).await.is_err());
    assert!(connect(Some(&tok_buta), None).await.is_err());

    let (mut ws, _) = connect(Some(&tok), Some(&format!("http://{addr}"))).await.expect("terhubung");
    // Pemantau berjalan; sesi baru muncul di router => event Connected sampai ke WebSocket.
    t.telemon.start_monitor(&rid).unwrap();
    tokio::time::sleep(Duration::from_millis(150)).await;
    t.world.active.lock().unwrap().push(row(&[(".id", "*a9"), ("name", "sari"), ("service", "pppoe")]));
    let msg = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let m = ws.next().await.unwrap().unwrap();
            if let tokio_tungstenite::tungstenite::Message::Text(txt) = m {
                let v: Value = serde_json::from_str(txt.as_str()).unwrap();
                if v["kind"] == "event" && v["event"]["type"] == "connected" && v["event"]["username"] == "sari" {
                    return v;
                }
            }
        }
    })
    .await
    .expect("event diterima");
    assert_eq!(msg["event"]["routerId"], rid);
    // Id live = id di basis data (tanpa duplikat saat klien memuat daftar event).
    let stored = t.telemon.with_store(|s| s.list_events(&telemon_core::store::EventQuery { limit: 50, ..Default::default() })).unwrap();
    assert!(stored.iter().any(|e| e.id.to_string() == msg["event"]["id"].as_str().unwrap() && e.event.username.as_deref() == Some("sari")), "id live tidak cocok dengan basis data: {msg}");
    t.telemon.shutdown().await;
}

// ---------------------------------------------------------------- AI (ADR 0007, 0024)

mod ai_tests {
    use super::*;
    use std::collections::VecDeque;
    use telemon_ai::{AiService, Completion, Llm, LlmConfig, LlmError, LlmFuture, Msg, ToolCall, ToolSpec, Usage};

    struct Fake(Mutex<VecDeque<Result<Completion, LlmError>>>);

    impl Llm for Fake {
        fn complete<'a>(&'a self, _c: &'a LlmConfig, _s: &'a str, _m: &'a [Msg], _t: &'a [ToolSpec]) -> LlmFuture<'a> {
            let next = self.0.lock().unwrap().pop_front().unwrap_or_else(|| Err(LlmError("skrip habis".into())));
            Box::pin(async move { next })
        }
    }

    fn say(t: &str) -> Result<Completion, LlmError> {
        Ok(Completion { text: t.into(), tool_calls: vec![], usage: Usage { input: 10, output: 5 } })
    }

    fn tool(name: &str, args: Value) -> Result<Completion, LlmError> {
        Ok(Completion { text: String::new(), tool_calls: vec![ToolCall { id: "c1".into(), name: name.into(), args }], usage: Usage { input: 10, output: 5 } })
    }

    fn with_ai(t: &T, script: Vec<Result<Completion, LlmError>>) {
        let svc = AiService::new(t.telemon.store(), t.telemon.secrets().clone(), t.auth.clone(), Arc::new(Fake(Mutex::new(script.into())))).with_clock(Arc::new(|| T0));
        t.st.set_ai(Arc::new(svc));
    }

    const PROVIDER: fn() -> Value = || json!({ "name": "Utama", "kind": "openai", "model": "gpt-x", "apiKey": "sk-RAHASIA-JANGAN-BOCOR-9999" });

    fn lines(r: &Resp) -> Vec<Value> {
        r.body.as_str().unwrap_or("").lines().filter(|l| !l.is_empty()).map(|l| serde_json::from_str(l).unwrap()).collect()
    }

    #[tokio::test]
    async fn usulan_tata_letak_ai_sampai_ke_klien_tanpa_menulis_dan_konteks_tampilan_dibatasi() {
        let t = harness();
        with_ai(&t, vec![tool("propose_layout_change", json!({ "changes": [{ "op": "add", "type": "bandwidth-total" }] })), say("Saya usulkan; tekan Terapkan."), tool("propose_layout_change", json!({ "changes": [{ "op": "add", "type": "bandwidth-total" }] })), say("Tidak bisa.")]);
        t.account("admin", "role-admin");
        t.account("operator", "role-operator");
        let ca = t.login("admin").await;
        t.post("/api/ai/providers", PROVIDER(), Some(&ca)).await;
        let ui = json!({ "widgets": [], "catalog": [{ "type": "bandwidth-total", "title": "Total", "description": "", "configurable": true }] });
        let msg = json!([{ "id": "m1", "role": "user", "text": "tambah total bandwidth", "at": 1 }]);

        // Administrator (izin Ubah Ringkasan): usulan datang sebagai widget `proposal`; tidak ada yang tersimpan.
        let q = t.post("/api/ai/ask", json!({ "messages": msg, "lang": "id", "ui": ui }), Some(&ca)).await;
        assert_eq!(q.status, StatusCode::OK, "{:?}", q.body);
        let ev = lines(&q);
        let proposal = ev.iter().find(|e| e["type"] == "widget" && e["widget"]["kind"] == "proposal").unwrap_or_else(|| panic!("tidak ada usulan: {ev:?}"));
        assert_eq!(proposal["widget"]["ops"], json!([{ "op": "add", "type": "bandwidth-total", "config": { "range": "6h" } }]));
        assert!(t.get("/api/dashboard", Some(&ca)).await.body["layout"].is_null(), "AI tidak boleh menulis tata letak");

        // Operator (hanya Lihat Ringkasan): alat menolak, tidak ada usulan.
        let co = t.login("operator").await;
        let q = t.post("/api/ai/ask", json!({ "messages": msg, "lang": "id", "ui": ui }), Some(&co)).await;
        assert_eq!(q.status, StatusCode::OK, "{:?}", q.body);
        let ev = lines(&q);
        assert!(!ev.iter().any(|e| e["type"] == "widget" && e["widget"]["kind"] == "proposal"), "{ev:?}");
        assert!(ev.iter().any(|e| e["type"] == "step" && e["text"].as_str().unwrap_or("").contains("tidak ada izin")), "{ev:?}");

        // Konteks tampilan yang terlalu besar atau salah bentuk ditolak sebelum model dipanggil.
        let big = json!({ "widgets": (0..100).map(|i| json!({ "id": format!("w{i}"), "type": "x" })).collect::<Vec<_>>(), "catalog": [] });
        assert_eq!(t.post("/api/ai/ask", json!({ "messages": msg, "ui": big }), Some(&ca)).await.status, StatusCode::BAD_REQUEST);
        let bad = json!({ "widgets": [{ "id": "spasi tidak boleh", "type": "x" }], "catalog": [] });
        assert_eq!(t.post("/api/ai/ask", json!({ "messages": msg, "ui": bad }), Some(&ca)).await.status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn tanpa_layanan_rute_ai_menjawab_501_dan_fitur_mati() {
        let t = harness();
        t.account("admin", "role-admin");
        let c = t.login("admin").await;
        let r = t.get("/api/ai/dashboard", Some(&c)).await;
        assert_eq!((r.status, r.code()), (StatusCode::NOT_IMPLEMENTED, "not_implemented"));
        let snap = t.get("/api/snapshot", Some(&c)).await;
        assert_eq!(snap.body["share"]["features"]["ai"], false);
    }

    #[tokio::test]
    async fn pertanyaan_dialirkan_sebagai_ndjson_dan_fitur_menyala_setelah_provider_ada() {
        let t = harness();
        with_ai(&t, vec![tool("get_summary", json!({})), say("Semua normal.")]);
        t.account("admin", "role-admin");
        let c = t.login("admin").await;
        assert_eq!(t.get("/api/snapshot", Some(&c)).await.body["share"]["features"]["ai"], false, "belum ada provider");
        let p = t.post("/api/ai/providers", PROVIDER(), Some(&c)).await;
        assert_eq!(p.status, StatusCode::OK, "{:?}", p.body);
        assert!(!p.body.to_string().contains("RAHASIA"), "kunci tidak boleh dikembalikan");
        assert_eq!(p.body["keyHint"], "sk-…9999");
        assert_eq!(t.get("/api/snapshot", Some(&c)).await.body["share"]["features"]["ai"], true);
        let q = t.post("/api/ai/ask", json!({ "messages": [{ "id": "m1", "role": "user", "text": "ringkas", "at": 1 }], "lang": "id" }), Some(&c)).await;
        assert_eq!(q.status, StatusCode::OK);
        assert_eq!(q.headers.get(header::CONTENT_TYPE).unwrap(), "application/x-ndjson");
        let ev = lines(&q);
        let kinds: Vec<&str> = ev.iter().map(|e| e["type"].as_str().unwrap()).collect();
        assert_eq!(&kinds[..1], ["step"], "{kinds:?}");
        assert!(kinds.contains(&"widget") && kinds.contains(&"text") && kinds.contains(&"sources"), "{kinds:?}");
        assert_eq!(ev.iter().find(|e| e["type"] == "text").unwrap()["text"], "Semua normal.");
        let d = t.get("/api/ai/dashboard?days=7", Some(&c)).await;
        assert_eq!(d.status, StatusCode::OK);
        assert_eq!(d.body["usage"].as_array().unwrap().iter().map(|x| x["requests"].as_i64().unwrap()).sum::<i64>(), 2);
        assert!(!d.body.to_string().contains("RAHASIA"));
    }

    #[tokio::test]
    async fn izin_menentukan_siapa_boleh_bertanya_dan_mengatur() {
        let t = harness();
        with_ai(&t, vec![say("x"), say("y")]);
        t.account("admin", "role-admin");
        t.account("budi", "role-viewer");
        t.account("pantau", "role-monitor");
        let admin = t.login("admin").await;
        t.post("/api/ai/providers", PROVIDER(), Some(&admin)).await;
        let ask = json!({ "messages": [{ "id": "m1", "role": "user", "text": "hai", "at": 1 }] });
        for u in ["budi", "pantau"] {
            let c = t.login(u).await;
            assert_eq!(t.post("/api/ai/ask", ask.clone(), Some(&c)).await.status, StatusCode::FORBIDDEN, "{u} tanpa izin AI chat");
            assert_eq!(t.post("/api/ai/providers", PROVIDER(), Some(&c)).await.status, StatusCode::FORBIDDEN, "{u} tidak boleh mengatur provider");
            assert_eq!(t.get("/api/ai/chats", Some(&c)).await.status, StatusCode::FORBIDDEN);
        }
        assert_eq!(t.post("/api/ai/ask", ask, None).await.status, StatusCode::UNAUTHORIZED, "tanpa sesi");
    }

    #[tokio::test]
    async fn galat_sebelum_jawaban_dimulai_menjadi_respons_http_biasa() {
        let t = harness();
        with_ai(&t, vec![]);
        t.account("admin", "role-admin");
        let c = t.login("admin").await;
        let ask = json!({ "messages": [{ "id": "m1", "role": "user", "text": "hai", "at": 1 }] });
        let r = t.post("/api/ai/ask", ask.clone(), Some(&c)).await;
        assert_eq!((r.status, r.code()), (StatusCode::CONFLICT, "ai_unavailable"), "{:?}", r.body);
        t.post("/api/ai/providers", PROVIDER(), Some(&c)).await;
        let r = t.post("/api/ai/ask", json!({ "messages": [] }), Some(&c)).await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST);
        // Model gagal di langkah pertama: tetap respons galat biasa (belum ada event).
        let r = t.post("/api/ai/ask", ask, Some(&c)).await;
        assert_eq!((r.status, r.code()), (StatusCode::BAD_GATEWAY, "ai_upstream"));
    }

    #[tokio::test]
    async fn galat_setelah_event_pertama_menjadi_event_error() {
        let t = harness();
        with_ai(&t, vec![tool("get_summary", json!({})), Err(LlmError("penyedia menjawab 500: boom".into()))]);
        t.account("admin", "role-admin");
        let c = t.login("admin").await;
        t.post("/api/ai/providers", PROVIDER(), Some(&c)).await;
        let r = t.post("/api/ai/ask", json!({ "messages": [{ "id": "m1", "role": "user", "text": "hai", "at": 1 }] }), Some(&c)).await;
        assert_eq!(r.status, StatusCode::OK);
        let ev = lines(&r);
        assert_eq!(ev.last().unwrap()["type"], "error");
        assert!(ev.last().unwrap()["message"].as_str().unwrap().contains("boom"));
    }

    #[tokio::test]
    async fn pertanyaan_dibatasi_per_akun_per_menit() {
        let t = harness();
        with_ai(&t, (0..40).map(|_| say("ok")).collect());
        t.account("admin", "role-admin");
        let c = t.login("admin").await;
        t.post("/api/ai/providers", PROVIDER(), Some(&c)).await;
        let ask = json!({ "messages": [{ "id": "m1", "role": "user", "text": "hai", "at": 1 }] });
        let mut limited = 0;
        for _ in 0..25 {
            if t.post("/api/ai/ask", ask.clone(), Some(&c)).await.status == StatusCode::TOO_MANY_REQUESTS {
                limited += 1;
            }
        }
        assert_eq!(limited, 5, "20 pertanyaan per menit per akun");
    }

    #[tokio::test]
    async fn riwayat_chat_hanya_milik_akun_sendiri_lewat_http() {
        let t = harness();
        with_ai(&t, vec![]);
        t.account("admin", "role-admin");
        t.account("oper", "role-operator");
        let (a, o) = (t.login("admin").await, t.login("oper").await);
        let chat = json!({ "id": "c-1", "title": "Rahasia admin", "pinned": false, "createdAt": 1, "updatedAt": 1, "messages": [{ "id": "m1", "role": "user", "text": "hai", "at": 1 }] });
        let put = t.call(Method::PUT, "/api/ai/chats/c-1", Some(chat.clone()), Some(&a), &[], LAN).await;
        assert_eq!(put.status, StatusCode::OK, "{:?}", put.body);
        assert_eq!(t.get("/api/ai/chats", Some(&a)).await.body.as_array().unwrap().len(), 1);
        assert!(t.get("/api/ai/chats", Some(&o)).await.body.as_array().unwrap().is_empty());
        assert_eq!(t.get("/api/ai/chats/c-1", Some(&o)).await.status, StatusCode::NOT_FOUND);
        assert_eq!(t.call(Method::PUT, "/api/ai/chats/c-1", Some(chat), Some(&o), &[], LAN).await.status, StatusCode::FORBIDDEN, "tidak boleh menimpa riwayat orang lain");
        assert_eq!(t.call(Method::DELETE, "/api/ai/chats/c-1", None, Some(&o), &[], LAN).await.status, StatusCode::NOT_FOUND);
        assert_eq!(t.call(Method::DELETE, "/api/ai/chats/c-1", None, Some(&a), &[], LAN).await.status, StatusCode::OK);
    }

    #[tokio::test]
    async fn url_dasar_provider_hanya_bisa_diubah_dari_desktop() {
        let t = harness();
        with_ai(&t, vec![]);
        t.account("admin", "role-admin");
        let c = t.login("admin").await;
        let custom = json!({ "name": "Router9", "kind": "ninerouter", "model": "m", "baseUrl": "http://10.0.0.9:20128/v1", "apiKey": "kunci-uji-123456" });
        let r = t.post("/api/ai/providers", custom.clone(), Some(&c)).await;
        assert_eq!(r.status, StatusCode::FORBIDDEN, "akun jarak jauh tidak boleh menetapkan URL kustom: {:?}", r.body);
        // Provider resmi tanpa URL tetap boleh; edit tanpa mengubah URL juga.
        let ok = t.post("/api/ai/providers", PROVIDER(), Some(&c)).await;
        assert_eq!(ok.status, StatusCode::OK);
        let id = ok.body["id"].as_str().unwrap().to_string();
        let bad = t.call(Method::PUT, &format!("/api/ai/providers/{id}"), Some(json!({ "name": "X", "kind": "openai", "model": "m", "baseUrl": "http://169.254.169.254/" })), Some(&c), &[], LAN).await;
        assert_eq!(bad.status, StatusCode::FORBIDDEN, "SSRF ke alamat internal harus ditolak");
    }
}

// ---------------- extension (ADR 0028, prototipe 1) ----------------

mod extensions {
    use super::*;
    use telemon_ext::{TrustedKeys, pack, public_key_hex};

    const OFFICIAL: &str = "0101010101010101010101010101010101010101010101010101010101010101";
    const PUBLISHER: &str = "0202020202020202020202020202020202020202020202020202020202020202";

    fn dir() -> std::path::PathBuf {
        let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        std::env::temp_dir().join(format!("telemon-ext-test-{n}-{:?}", std::thread::current().id()).replace(['(', ')'], ""))
    }

    fn on(dir: &std::path::Path) -> HttpConfig {
        let keys = TrustedKeys { official: vec![public_key_hex(OFFICIAL).unwrap()], publishers: vec![public_key_hex(PUBLISHER).unwrap()], allow_unsigned: false };
        HttpConfig { extensions: Some(telemon_http::ExtConfig { dir: dir.into(), keys, store_url: Some("https://store.test/x/catalog.json".into()), fuel: 3_000_000 }), ..Default::default() }
    }

    fn manifest(version: &str, extra_perm: &str) -> String {
        format!(
            r#"{{"id":"net.teliti.demo","name":"Demo","version":"{version}","apiVersion":1,"publisher":"Teliti",
            "ui":{{"entry":"ui/index.js"}},"contributes":{{"widgets":[{{"type":"hello","title":"Hello"}}]}},
            "permissions":[{{"id":"data.events","required":true}},{{"id":"feature.ai","sensitive":true}}{extra_perm}]}}"#
        )
    }

    fn pkg(version: &str, extra_perm: &str, key: Option<&str>) -> Vec<u8> {
        pack(&[("manifest.json".into(), manifest(version, extra_perm).into_bytes()), ("ui/index.js".into(), b"export default { ok: true }".to_vec())], key).unwrap()
    }

    async fn send(t: &T, method: Method, uri: &str, body: Vec<u8>, cookie: &str, extra: &[(&str, &str)], peer: &str) -> Resp {
        let mut b = Request::builder().method(method).uri(uri).header(header::COOKIE, format!("telemon_session={cookie}")).header(header::CONTENT_TYPE, "application/octet-stream");
        for (k, v) in extra {
            b = b.header(*k, *v);
        }
        let mut req = b.body(Body::from(body)).unwrap();
        req.extensions_mut().insert(ConnectInfo(peer.parse::<SocketAddr>().unwrap()));
        let res = t.app.clone().oneshot(req).await.unwrap();
        let (parts, body) = res.into_parts();
        let bytes = body.collect().await.unwrap().to_bytes();
        let body = serde_json::from_slice(&bytes).unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into()));
        Resp { status: parts.status, headers: parts.headers, body }
    }

    async fn install(t: &T, bytes: Vec<u8>, c: &str) -> Resp {
        send(t, Method::POST, "/api/extensions", bytes, c, &[], LAN).await
    }

    async fn patch(t: &T, id: &str, body: Value, c: &str) -> Resp {
        t.call(Method::PATCH, &format!("/api/extensions/{id}"), Some(body), Some(c), &[], LAN).await
    }

    #[tokio::test]
    async fn daftar_melaporkan_jumlah_kunci_official_yang_dipercaya() {
        let d = dir();
        let t = harness_with(on(&d));
        t.account("admin", "role-admin");
        let a = t.login("admin").await;
        assert_eq!(t.get("/api/extensions", Some(&a)).await.body["officialKeys"], 1);
        let _ = std::fs::remove_dir_all(&d);
        // Tanpa kunci official: angka 0 (UI menjelaskan mengapa extension belum bisa dipasang).
        let mut cfg = on(&dir());
        if let Some(e) = cfg.extensions.as_mut() {
            e.keys.official.clear();
        }
        let t = harness_with(cfg);
        t.account("admin", "role-admin");
        let a = t.login("admin").await;
        assert_eq!(t.get("/api/extensions", Some(&a)).await.body["officialKeys"], 0);
    }

    #[tokio::test]
    async fn mati_secara_bawaan() {
        let t = harness();
        t.account("admin", "role-admin");
        let c = t.login("admin").await;
        let r = t.get("/api/extensions", Some(&c)).await;
        assert_eq!((r.status, r.body["available"].as_bool()), (StatusCode::OK, Some(false)));
        assert!(t.get("/api/extensions/active", Some(&c)).await.body["extensions"].as_array().unwrap().is_empty());
        let r = install(&t, pkg("1.0.0", "", Some(OFFICIAL)), &c).await;
        assert_eq!((r.status, r.code()), (StatusCode::NOT_IMPLEMENTED, "extensions_off"));
        assert_eq!(t.get("/ext/net.teliti.demo/ui/index.js", Some(&c)).await.status, StatusCode::NOT_FOUND);
    }

    /// Lisensi extension (ADR 0030) sampai ke pemakai: pratinjau paket (inspect) dan tampilan terpasang; tanpa lisensi = null.
    #[tokio::test]
    async fn lisensi_extension_tampil_di_pratinjau_dan_tampilan_terpasang() {
        let d = dir();
        let t = harness_with(on(&d));
        t.account("admin", "role-admin");
        let c = t.login("admin").await;
        let with = |lic: &str| {
            let m = format!(r#"{{"id":"net.teliti.lic","name":"Lic","version":"1.0.0","apiVersion":1,"publisher":"Teliti","ui":{{"entry":"ui/index.js"}}{lic}}}"#);
            pack(&[("manifest.json".into(), m.into_bytes()), ("ui/index.js".into(), b"export default {}".to_vec())], Some(OFFICIAL)).unwrap()
        };
        let lic = r#","license":{"kind":"freemium","summary":"Free for 5 routers","url":"https://example.com/pricing"}"#;
        let r = send(&t, Method::POST, "/api/extensions/inspect", with(lic), &c, &[], LAN).await;
        assert_eq!(r.body["package"]["license"]["kind"], "freemium", "{:?}", r.body);
        assert_eq!(r.body["package"]["license"]["url"], "https://example.com/pricing");
        let r = install(&t, with(lic), &c).await;
        assert_eq!((r.status, r.body["license"]["summary"].as_str()), (StatusCode::OK, Some("Free for 5 routers")), "{:?}", r.body);
        // Tanpa lisensi: null (UI menampilkan "tidak dinyatakan").
        let r = send(&t, Method::POST, "/api/extensions/inspect", with(""), &c, &[], LAN).await;
        assert!(r.body["package"]["license"].is_null(), "{:?}", r.body);
        // Lisensi tak valid ditolak saat pratinjau.
        let r = send(&t, Method::POST, "/api/extensions/inspect", with(r#","license":{"kind":"free","url":"http://x.test"}"#), &c, &[], LAN).await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{:?}", r.body);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[tokio::test]
    async fn pasang_aktifkan_sajikan_dan_hapus() {
        let d = dir();
        let t = harness_with(on(&d));
        t.account("admin", "role-admin");
        let c = t.login("admin").await;

        let r = send(&t, Method::POST, "/api/extensions/inspect", pkg("1.0.0", "", Some(OFFICIAL)), &c, &[], LAN).await;
        assert_eq!((r.status, r.body["package"]["tier"].as_str(), r.body["package"]["signed"].as_bool()), (StatusCode::OK, Some("official"), Some(true)), "{:?}", r.body);
        assert!(t.get("/api/extensions", Some(&c)).await.body["extensions"].as_array().unwrap().is_empty(), "inspect tidak memasang");

        let r = install(&t, pkg("1.0.0", "", Some(OFFICIAL)), &c).await;
        assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
        assert_eq!((r.body["enabled"].as_bool(), r.body["status"].as_str()), (Some(false), Some("disabled")));
        let perms = r.body["permissions"].as_array().unwrap();
        let g = |id: &str| perms.iter().find(|p| p["id"] == id).unwrap()["granted"].as_bool().unwrap();
        assert!(g("data.events") && !g("feature.ai"), "izin sensitif mulai mati, wajib menyala");

        // Belum aktif: tidak disajikan dan tidak dimuat UI.
        assert_eq!(t.get("/ext/net.teliti.demo/ui/index.js", Some(&c)).await.status, StatusCode::NOT_FOUND);
        assert!(t.get("/api/extensions/active", Some(&c)).await.body["extensions"].as_array().unwrap().is_empty());

        let r = patch(&t, "net.teliti.demo", json!({ "enabled": true }), &c).await;
        assert_eq!((r.status, r.body["status"].as_str()), (StatusCode::OK, Some("ok")), "{:?}", r.body);
        let a = t.get("/api/extensions/active", Some(&c)).await;
        let e = &a.body["extensions"][0];
        assert_eq!(e["id"], "net.teliti.demo");
        assert_eq!(e["entry"], "/ext/net.teliti.demo/ui/index.js?v=1.0.0");
        assert_eq!(e["permissions"], json!(["data.events"]));
        assert_eq!(e["widgets"][0]["type"], "hello");

        let f = t.get("/ext/net.teliti.demo/ui/index.js", Some(&c)).await;
        assert_eq!(f.status, StatusCode::OK);
        assert_eq!(f.body, Value::String("export default { ok: true }".into()));
        assert_eq!(f.headers[header::CONTENT_TYPE], "text/javascript; charset=utf-8");
        // Hanya ui/, tanpa manifest, tanpa jalur lolos.
        for p in ["manifest.json", "ui/tidak-ada.js", "signature.json", "ui/../manifest.json"] {
            assert_eq!(t.get(&format!("/ext/net.teliti.demo/{p}"), Some(&c)).await.status, StatusCode::NOT_FOUND, "{p}");
        }
        assert_eq!(t.get("/ext/net.teliti.demo/ui/index.js", None).await.status, StatusCode::UNAUTHORIZED);

        let r = t.call(Method::DELETE, "/api/extensions/net.teliti.demo", None, Some(&c), &[], LAN).await;
        assert_eq!(r.status, StatusCode::OK);
        assert_eq!(t.get("/ext/net.teliti.demo/ui/index.js", Some(&c)).await.status, StatusCode::NOT_FOUND);
        assert!(!d.join("net.teliti.demo.tmx").exists(), "berkas paket ikut terhapus");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[tokio::test]
    async fn paket_tak_tepercaya_ditolak() {
        let d = dir();
        let t = harness_with(on(&d));
        t.account("admin", "role-admin");
        let c = t.login("admin").await;
        // Tanpa tanda tangan, kunci asing, dan penerbit publik (non-official belum didukung).
        assert_eq!(install(&t, pkg("1.0.0", "", None), &c).await.status, StatusCode::BAD_REQUEST);
        let other = "0303030303030303030303030303030303030303030303030303030303030303";
        assert_eq!(install(&t, pkg("1.0.0", "", Some(other)), &c).await.status, StatusCode::BAD_REQUEST);
        let r = install(&t, pkg("1.0.0", "", Some(PUBLISHER)), &c).await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST);
        assert!(r.body["error"]["message"].as_str().unwrap().contains("non-official"));
        assert_eq!(install(&t, b"bukan zip".to_vec(), &c).await.status, StatusCode::BAD_REQUEST);
        assert!(t.get("/api/extensions", Some(&c)).await.body["extensions"].as_array().unwrap().is_empty());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[tokio::test]
    async fn izin_dimatikan_membatasi_dan_perubahan_dicatat() {
        let d = dir();
        let t = harness_with(on(&d));
        t.account("admin", "role-admin");
        let c = t.login("admin").await;
        install(&t, pkg("1.0.0", "", Some(OFFICIAL)), &c).await;
        patch(&t, "net.teliti.demo", json!({ "enabled": true }), &c).await;

        // Izin wajib dimatikan: terbatas dan tidak dimuat UI.
        let r = patch(&t, "net.teliti.demo", json!({ "perms": { "data.events": false } }), &c).await;
        assert_eq!(r.body["status"], "limited");
        assert!(t.get("/api/extensions/active", Some(&c)).await.body["extensions"].as_array().unwrap().is_empty());
        let r = patch(&t, "net.teliti.demo", json!({ "perms": { "data.events": true, "feature.ai": true } }), &c).await;
        assert_eq!(r.body["status"], "ok");
        assert_eq!(t.get("/api/extensions/active", Some(&c)).await.body["extensions"][0]["permissions"], json!(["data.events", "feature.ai"]));
        // Izin yang tidak dideklarasikan ditolak; extension tak dikenal 404.
        assert_eq!(patch(&t, "net.teliti.demo", json!({ "perms": { "feature.rahasia": true } }), &c).await.status, StatusCode::BAD_REQUEST);
        assert_eq!(patch(&t, "net.lain.x", json!({ "enabled": true }), &c).await.status, StatusCode::NOT_FOUND);

        let rows = t.telemon.with_store(|s| s.list_audit(&telemon_core::AuditQuery::default()).unwrap());
        let actions: Vec<&str> = rows.iter().map(|r| r.entry.action.as_str()).collect();
        assert!(actions.contains(&"extension.install") && actions.iter().filter(|a| **a == "extension.toggle").count() >= 3, "{actions:?}");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[tokio::test]
    async fn pembaruan_mempertahankan_status_dan_izin_baru_mati() {
        let d = dir();
        let t = harness_with(on(&d));
        t.account("admin", "role-admin");
        let c = t.login("admin").await;
        install(&t, pkg("1.0.0", "", Some(OFFICIAL)), &c).await;
        patch(&t, "net.teliti.demo", json!({ "enabled": true, "perms": { "feature.ai": true } }), &c).await;
        let r = install(&t, pkg("1.1.0", r#",{"id":"data.traffic"}"#, Some(OFFICIAL)), &c).await;
        assert_eq!((r.body["version"].as_str(), r.body["enabled"].as_bool()), (Some("1.1.0"), Some(true)));
        let perms = r.body["permissions"].as_array().unwrap();
        let g = |id: &str| perms.iter().find(|p| p["id"] == id).unwrap()["granted"].as_bool().unwrap();
        assert!(g("data.events") && g("feature.ai"), "keputusan lama bertahan");
        assert!(!g("data.traffic"), "izin baru mati sampai diputuskan admin");
        let f = t.get("/ext/net.teliti.demo/ui/index.js", Some(&c)).await;
        assert_eq!(f.status, StatusCode::OK);
        let _ = std::fs::remove_dir_all(&d);
    }

    fn pkg_json(id: &str, version: &str, extra: &str) -> Vec<u8> {
        let m = format!(r#"{{"id":"{id}","name":"{id}","version":"{version}","apiVersion":1,"publisher":"Teliti","ui":{{"entry":"ui/index.js"}}{extra}}}"#);
        pack(&[("manifest.json".into(), m.into_bytes()), ("ui/index.js".into(), b"export default {}".to_vec())], Some(OFFICIAL)).unwrap()
    }

    #[tokio::test]
    async fn kompatibilitas_telemon_dan_antar_extension() {
        let d = dir();
        let mut cfg = on(&d);
        cfg.host_version = "0.3.0-rc.2".into();
        let t = harness_with(cfg);
        t.account("admin", "role-admin");
        let c = t.login("admin").await;
        let (a, b) = ("net.teliti.a", "net.teliti.b");

        // Versi Telemon di luar rentang: ditolak, pesan menyebut keduanya.
        let r = install(&t, pkg_json(a, "1.0.0", r#","telemon":">=0.4.0""#), &c).await;
        assert_eq!((r.status, r.code()), (StatusCode::CONFLICT, "incompatible"));
        let msg = r.body["error"]["message"].as_str().unwrap();
        assert!(msg.contains(">=0.4.0") && msg.contains("0.3.0"), "{msg}");

        // A butuh B: tanpa B ditolak; inspect menjelaskan lewat `problems` dan `requires`.
        let need_b = format!(r#","telemon":">=0.2.0 <0.4.0","requires":[{{"id":"{b}","version":"^1.2.0"}}]"#);
        let r = send(&t, Method::POST, "/api/extensions/inspect", pkg_json(a, "1.0.0", &need_b), &c, &[], LAN).await;
        assert_eq!(r.body["problems"][0]["kind"], "missingDependency");
        assert_eq!(r.body["requires"][0]["state"], "missing");
        assert_eq!(install(&t, pkg_json(a, "1.0.0", &need_b), &c).await.status, StatusCode::CONFLICT);

        // B versi terlalu lama: tidak cocok; B 1.3.0: A terpasang.
        assert_eq!(install(&t, pkg_json(b, "1.0.0", ""), &c).await.status, StatusCode::OK);
        let r = install(&t, pkg_json(a, "1.0.0", &need_b), &c).await;
        assert_eq!((r.status, r.code()), (StatusCode::CONFLICT, "incompatible"), "B 1.0.0 di luar ^1.2.0");
        assert_eq!(install(&t, pkg_json(b, "1.3.0", ""), &c).await.status, StatusCode::OK);
        let r = install(&t, pkg_json(a, "1.0.0", &need_b), &c).await;
        assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
        assert_eq!(r.body["requires"][0]["state"], "ok");

        // Memperbarui B ke versi yang melanggar A ditolak; yang cocok boleh. B tahu siapa yang bergantung.
        assert_eq!(install(&t, pkg_json(b, "2.0.0", ""), &c).await.status, StatusCode::CONFLICT);
        assert_eq!(install(&t, pkg_json(b, "1.4.0", ""), &c).await.status, StatusCode::OK);
        let list = t.get("/api/extensions", Some(&c)).await.body;
        let bi = list["extensions"].as_array().unwrap().iter().find(|e| e["id"] == b).unwrap();
        assert_eq!(bi["dependents"], json!([a]));

        // Menghapus B selama A butuh B ditolak; menghapus A dulu lalu B boleh.
        async fn del(t: &T, id: &str, c: &str) -> Resp {
            t.call(Method::DELETE, &format!("/api/extensions/{id}"), None, Some(c), &[], LAN).await
        }
        let r = del(&t, b, &c).await;
        assert_eq!((r.status, r.code()), (StatusCode::CONFLICT, "has_dependents"));
        assert!(r.body["error"]["message"].as_str().unwrap().contains(a));

        // A hanya dimuat bila B aktif: B mati = A terbatas dan keluar dari daftar aktif.
        patch(&t, a, json!({ "enabled": true }), &c).await;
        assert_eq!(t.get("/api/extensions/active", Some(&c)).await.body["extensions"].as_array().unwrap().len(), 0, "B belum aktif");
        let r = patch(&t, a, json!({}), &c).await;
        assert_eq!((r.body["status"].as_str(), r.body["blockedBy"].clone()), (Some("limited"), json!([b])));
        patch(&t, b, json!({ "enabled": true }), &c).await;
        let ids: Vec<String> = t.get("/api/extensions/active", Some(&c)).await.body["extensions"].as_array().unwrap().iter().map(|e| e["id"].as_str().unwrap().into()).collect();
        assert_eq!(ids.len(), 2);
        patch(&t, b, json!({ "enabled": false }), &c).await;
        assert_eq!(t.get("/api/extensions/active", Some(&c)).await.body["extensions"].as_array().unwrap().len(), 0, "A ikut tidak dimuat saat B mati");

        assert_eq!(del(&t, a, &c).await.status, StatusCode::OK);
        assert_eq!(del(&t, b, &c).await.status, StatusCode::OK);
        let _ = std::fs::remove_dir_all(&d);
    }

    fn pkg_migr(version: &str, migrations: &[(&str, &str)]) -> Vec<u8> {
        let m = format!(r#"{{"id":"net.teliti.data","name":"Data","version":"{version}","apiVersion":1,"publisher":"Teliti"}}"#);
        let mut files: Vec<(String, Vec<u8>)> = vec![("manifest.json".into(), m.into_bytes())];
        for (n, body) in migrations {
            files.push((format!("migrations/{n}"), body.as_bytes().to_vec()));
        }
        pack(&files, Some(OFFICIAL)).unwrap()
    }

    const M1: &str = r#"{"ops":[{"op":"create_table","table":"x_net_teliti_data__items","columns":[{"name":"id","type":"integer","primaryKey":true,"autoIncrement":true},{"name":"label","type":"text","notNull":true}]}]}"#;
    const M2: &str = r#"{"ops":[{"op":"add_column","table":"x_net_teliti_data__items","column":{"name":"size","type":"integer","default":0}}]}"#;

    #[tokio::test]
    async fn migrasi_database_extension() {
        let d = dir();
        let t = harness_with(on(&d));
        t.account("admin", "role-admin");
        let c = t.login("admin").await;
        let id = "net.teliti.data";
        let tables = |t: &T| t.telemon.with_store(|s| s.ext_table_names("x_net_teliti_data__").unwrap());
        let cols = |t: &T| t.telemon.with_store(|s| s.ext_table_names("x_net_teliti_data__").unwrap().len());

        // Pasang: tabel dibuat, catatan migrasi ada, cadangan dibuat sebelum migrasi.
        let r = install(&t, pkg_migr("1.0.0", &[("0001-init.json", M1)]), &c).await;
        assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
        assert_eq!((tables(&t), r.body["hasData"].as_bool()), (vec!["x_net_teliti_data__items".to_string()], Some(true)));
        let backups: Vec<_> = std::fs::read_dir(d.join("backups")).unwrap().collect();
        assert_eq!(backups.len(), 1, "satu cadangan sebelum migrasi");

        // Isi data, lalu perbarui dengan migrasi baru: data tetap, kolom bertambah.
        t.telemon.with_store(|s| s.ext_apply_migrations(id, 1, &[]).unwrap());
        let rows_before = t.telemon.with_store(|s| s.ext_applied_migrations(id).unwrap().len());
        assert_eq!(rows_before, 1);
        let r = install(&t, pkg_migr("1.1.0", &[("0001-init.json", M1), ("0002-size.json", M2)]), &c).await;
        assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
        assert_eq!(t.telemon.with_store(|s| s.ext_applied_migrations(id).unwrap().len()), 2);
        // Pasang ulang versi yang sama: tidak ada migrasi baru, tidak ada cadangan baru.
        let n = std::fs::read_dir(d.join("backups")).unwrap().count();
        install(&t, pkg_migr("1.1.0", &[("0001-init.json", M1), ("0002-size.json", M2)]), &c).await;
        assert_eq!(std::fs::read_dir(d.join("backups")).unwrap().count(), n);

        // Migrasi terapan diedit: ditolak. Paket lebih lama daripada data: ditolak. Tak ada yang berubah.
        let edited = M1.replace("label", "title");
        let r = install(&t, pkg_migr("1.2.0", &[("0001-init.json", &edited), ("0002-size.json", M2)]), &c).await;
        assert_eq!((r.status, r.code()), (StatusCode::CONFLICT, "migration_conflict"));
        assert!(r.body["error"]["message"].as_str().unwrap().contains("berubah"));
        let r = install(&t, pkg_migr("1.0.0", &[("0001-init.json", M1)]), &c).await;
        assert_eq!((r.status, r.code()), (StatusCode::CONFLICT, "migration_conflict"), "penurunan versi data");
        let ver = t.get("/api/extensions", Some(&c)).await.body["extensions"][0]["version"].clone();
        assert_eq!(ver, "1.1.0", "versi tidak berubah oleh pemasangan yang ditolak");

        // Migrasi yang melanggar namespace ditolak sebelum menyentuh apa pun (di tingkat paket).
        let evil = r#"{"ops":[{"op":"drop_table","table":"accounts"}]}"#;
        let r = install(&t, pkg_migr("1.3.0", &[("0001-init.json", M1), ("0002-size.json", M2), ("0003-evil.json", evil)]), &c).await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST);
        assert!(t.telemon.with_store(|s| s.ext_list().is_ok()));

        // Migrasi yang gagal di database (kolom sudah ada): dibatalkan, tabel baru ikut batal, versi tetap.
        let dup = r#"{"ops":[{"op":"create_table","table":"x_net_teliti_data__more","columns":[{"name":"id","type":"integer"}]},{"op":"add_column","table":"x_net_teliti_data__items","column":{"name":"size","type":"integer","default":0}}]}"#;
        let r = install(&t, pkg_migr("1.3.0", &[("0001-init.json", M1), ("0002-size.json", M2), ("0003-dup.json", dup)]), &c).await;
        assert_eq!((r.status, r.code()), (StatusCode::UNPROCESSABLE_ENTITY, "migration_failed"), "{:?}", r.body);
        assert_eq!(cols(&t), 1, "tabel more tidak tersisa");
        assert_eq!(t.get("/api/extensions", Some(&c)).await.body["extensions"][0]["version"], "1.1.0");

        // Hapus tanpa data: tabel dan catatan tetap; pasang ulang tidak menjalankan migrasi lagi.
        let r = t.call(Method::DELETE, &format!("/api/extensions/{id}"), None, Some(&c), &[], LAN).await;
        assert_eq!((r.status, r.body["dataDeleted"].as_bool()), (StatusCode::OK, Some(false)));
        assert_eq!(tables(&t).len(), 1);
        let r = install(&t, pkg_migr("1.1.0", &[("0001-init.json", M1), ("0002-size.json", M2)]), &c).await;
        assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
        assert_eq!(t.telemon.with_store(|s| s.ext_applied_migrations(id).unwrap().len()), 2);

        // Hapus dengan data: tabel dan catatan hilang; tabel inti utuh.
        let r = t.call(Method::DELETE, &format!("/api/extensions/{id}?data=delete"), None, Some(&c), &[], LAN).await;
        assert_eq!((r.status, r.body["dataDeleted"].as_bool(), r.body["tablesDropped"].as_u64()), (StatusCode::OK, Some(true), Some(1)));
        assert!(tables(&t).is_empty());
        assert!(t.telemon.with_store(|s| s.ext_applied_migrations(id).unwrap().is_empty()));
        let _ = std::fs::remove_dir_all(&d);
    }

    fn pkg_actions(version: &str) -> Vec<u8> {
        let m = format!(
            r#"{{"id":"net.teliti.acl","name":"Acl","version":"{version}","apiVersion":1,"publisher":"Teliti","ui":{{"entry":"ui/index.js"}},
            "contributes":{{"pages":[{{"id":"home","title":"Beranda","kind":"dashboard"}}]}},
            "actions":[{{"id":"view-report","label":"Lihat laporan"}},{{"id":"run","label":"Jalankan","level":"update"}}]}}"#
        );
        pack(&[("manifest.json".into(), m.into_bytes()), ("ui/index.js".into(), b"export default {}".to_vec())], Some(OFFICIAL)).unwrap()
    }

    #[tokio::test]
    async fn halaman_akses_per_extension_dan_pengaman_kenaikan() {
        let d = dir();
        let t = harness_with(on(&d));
        let sub = t.auth.create_role(None, &RoleInput { name: "sub-admin".into(), description: String::new(), permissions: Permissions::default().with(Resource::Admin, Level::Update) }).unwrap();
        t.account("admin", "role-admin");
        t.account("sub", &sub.id);
        t.account("pantau", "role-monitor");
        let (a, s, p) = (t.login("admin").await, t.login("sub").await, t.login("pantau").await);
        let id = "net.teliti.acl";
        let access = |role: &str| format!("/api/extensions/{id}/access/{role}");
        let put = |ck: String, role: String, body: Value| {
            let t = &t;
            async move { t.call(Method::PUT, &access(&role), Some(body), Some(&ck), &[], LAN).await }
        };
        assert_eq!(install(&t, pkg_actions("1.0.0"), &a).await.status, StatusCode::OK);
        patch(&t, id, json!({ "enabled": true }), &a).await;

        // Bawaan: peran dengan Admin Ubah mendapat akses penuh; peran lain tidak.
        let listed = t.get("/api/extensions", Some(&a)).await.body;
        assert_eq!(listed["extensions"][0]["pages"][0]["id"], "home", "daftar memuat halaman");
        assert_eq!(listed["extensions"][0]["actions"].as_array().unwrap().len(), 2);
        let g = t.get(&format!("/api/extensions/{id}/access"), Some(&a)).await;
        assert_eq!(g.status, StatusCode::OK, "{:?}", g.body);
        let lvl = |g: &Resp, role: &str| g.body["roles"].as_array().unwrap().iter().find(|r| r["id"] == role).unwrap()["level"].as_str().unwrap().to_string();
        assert_eq!((lvl(&g, "role-admin"), lvl(&g, &sub.id), lvl(&g, "role-monitor")), ("update".into(), "update".into(), "none".into()));
        assert_eq!(g.body["actions"].as_array().unwrap().len(), 2);
        assert_eq!(g.body["me"]["actions"], json!(["view-report", "run"]));
        assert!(t.get("/api/extensions/active", Some(&p)).await.body["extensions"].as_array().unwrap().is_empty(), "pantau belum punya akses");

        // Beri Lihat + satu aksi: aktif muncul dengan akses efektif; aksi level update tidak berlaku di level Lihat.
        assert_eq!(put(a.clone(), "role-monitor".into(), json!({ "level": "read", "actions": ["view-report"] })).await.status, StatusCode::OK);
        let act = t.get("/api/extensions/active", Some(&p)).await.body;
        assert_eq!(act["extensions"][0]["access"], json!({ "level": "read", "actions": ["view-report"] }));
        assert_eq!(act["extensions"][0]["pages"][0]["kind"], "dashboard");

        // Validasi: level, aksi tak dikenal, aksi yang butuh level lebih tinggi, peran tak ada.
        for (why, role, body, code) in [
            ("level aneh", "role-monitor", json!({ "level": "owner" }), StatusCode::BAD_REQUEST),
            ("aksi tak dikenal", "role-monitor", json!({ "level": "read", "actions": ["x"] }), StatusCode::BAD_REQUEST),
            ("aksi butuh update", "role-monitor", json!({ "level": "read", "actions": ["run"] }), StatusCode::BAD_REQUEST),
            ("peran tak ada", "role-hantu", json!({ "level": "read" }), StatusCode::NOT_FOUND),
        ] {
            assert_eq!(put(a.clone(), role.into(), body, ).await.status, code, "{why}");
        }

        // Pengaman: tidak mengubah peran sendiri; tidak memberi lebih daripada milik sendiri; tidak mengelola peran lebih tinggi.
        assert_eq!(put(s.clone(), sub.id.clone(), json!({ "level": "none" })).await.status, StatusCode::FORBIDDEN, "peran sendiri");
        // Turunkan sub-admin ke Lihat (oleh admin), lalu sub-admin mencoba memberi Ubah ke pantau.
        assert_eq!(put(a.clone(), sub.id.clone(), json!({ "level": "read", "actions": ["view-report"] })).await.status, StatusCode::OK);
        assert_eq!(put(s.clone(), "role-monitor".into(), json!({ "level": "update" })).await.status, StatusCode::FORBIDDEN, "melebihi akses sendiri");
        assert_eq!(put(s.clone(), "role-monitor".into(), json!({ "level": "read", "actions": [] })).await.status, StatusCode::OK, "setara atau lebih rendah boleh");
        assert_eq!(put(a.clone(), "role-monitor".into(), json!({ "level": "update", "actions": ["view-report", "run"] })).await.status, StatusCode::OK);
        assert_eq!(put(s.clone(), "role-monitor".into(), json!({ "level": "none" })).await.status, StatusCode::FORBIDDEN, "peran yang aksesnya lebih tinggi");
        // Sub-admin tidak punya aksi `run`: tidak bisa memberikannya.
        assert_eq!(put(a.clone(), "role-monitor".into(), json!({ "level": "read", "actions": [] })).await.status, StatusCode::OK);
        assert_eq!(put(s.clone(), "role-monitor".into(), json!({ "level": "read", "actions": ["view-report"] })).await.status, StatusCode::OK);

        // Peran tanpa Admin tidak bisa melihat atau mengubah; lewat Publik pun tidak.
        assert_eq!(t.get(&format!("/api/extensions/{id}/access"), Some(&p)).await.status, StatusCode::FORBIDDEN);
        assert_eq!(put(p.clone(), "role-monitor".into(), json!({ "level": "update" })).await.status, StatusCode::FORBIDDEN);
        let r = t.call(Method::PUT, &access("role-monitor"), Some(json!({ "level": "none" })), Some(&a), &[("cf-connecting-ip", "203.0.113.9")], LOOP).await;
        assert_eq!(r.status, StatusCode::FORBIDDEN);

        // Perubahan tercatat; menghapus extension menghapus aksesnya.
        let rows = t.telemon.with_store(|s| s.list_audit(&telemon_core::AuditQuery::default()).unwrap());
        assert!(rows.iter().filter(|r| r.entry.action == "extension.access").count() >= 4);
        t.call(Method::DELETE, &format!("/api/extensions/{id}"), None, Some(&a), &[], LAN).await;
        assert!(t.telemon.with_store(|s| s.ext_access_list(id).unwrap().is_empty()));
        let _ = std::fs::remove_dir_all(&d);
    }

    struct FakeFetch(Mutex<std::collections::HashMap<String, Vec<u8>>>);

    impl telemon_http::ExtFetch for FakeFetch {
        fn get(&self, url: &str, max: usize) -> Result<Vec<u8>, String> {
            let b = self.0.lock().unwrap().get(url).cloned().ok_or_else(|| format!("404 {url}"))?;
            if b.len() > max { Err("terlalu besar".into()) } else { Ok(b) }
        }
    }

    /// Katalog bertanda tangan dengan paket: (id, versi, tambahan manifest).
    fn store(items: &[(&str, &str, &str)], sign_with: &str) -> FakeFetch {
        let mut files = std::collections::HashMap::new();
        let mut entries = Vec::new();
        for (id, version, extra) in items {
            let manifest = format!(r#"{{"id":"{id}","name":"{id}","version":"{version}","apiVersion":1,"publisher":"Teliti","ui":{{"entry":"ui/index.js"}}{extra}}}"#);
            let bytes = pack(&[("manifest.json".into(), manifest.clone().into_bytes()), ("ui/index.js".into(), b"export default {}".to_vec())], Some(OFFICIAL)).unwrap();
            let file = format!("{id}-{version}.tmx");
            entries.push(format!(r#"{{"manifest":{manifest},"file":"{file}","sha256":"{}","size":{}}}"#, telemon_ext::sha256_hex(&bytes), bytes.len()));
            files.insert(format!("https://store.test/x/{file}"), bytes);
        }
        let cat = format!(r#"{{"version":1,"generated":"2026-11-26","extensions":[{}]}}"#, entries.join(",")).into_bytes();
        files.insert("https://store.test/x/catalog.json.sig".into(), telemon_ext::sign_bytes(sign_with, &cat).unwrap().into_bytes());
        files.insert("https://store.test/x/catalog.json".into(), cat);
        FakeFetch(Mutex::new(files))
    }

    #[tokio::test]
    async fn toko_official_memasang_dependensi_lebih_dulu() {
        let d = dir();
        let t = harness_with(on(&d));
        t.account("admin", "role-admin");
        let c = t.login("admin").await;
        // Tanpa pengunduh: toko 501.
        let r = t.get("/api/extensions/store", Some(&c)).await;
        assert_eq!((r.status, r.code()), (StatusCode::NOT_IMPLEMENTED, "store_off"));

        let req_b = r#","requires":[{"id":"net.teliti.b","version":"^1.0.0"}]"#;
        let req_c = r#","requires":[{"id":"net.teliti.c","version":"^1.0.0"}]"#;
        t.st.set_ext_fetcher(Arc::new(store(&[("net.teliti.a", "1.0.0", &format!("{req_b}")), ("net.teliti.b", "1.2.0", req_c), ("net.teliti.c", "1.0.0", ""), ("net.teliti.alone", "1.0.0", "")], OFFICIAL)));
        let list = t.get("/api/extensions/store", Some(&c)).await;
        assert_eq!(list.status, StatusCode::OK, "{:?}", list.body);
        let a = list.body["extensions"].as_array().unwrap().iter().find(|e| e["id"] == "net.teliti.a").unwrap();
        assert_eq!(a["alsoInstalls"], json!(["net.teliti.c", "net.teliti.b"]), "urutan: dependensi dulu");
        assert!(a["permissions"].is_array() && a["widgets"].is_array() && a["actions"].is_array(), "detail untuk popup toko: izin, widget, aksi");
        assert_eq!((a["hasBackend"].as_bool(), a["routes"].as_u64()), (Some(false), Some(0)));

        // Pasang A: B dan C ikut, urut, semuanya mati dan siap diatur.
        let r = t.post("/api/extensions/store/install", json!({ "id": "net.teliti.a" }), Some(&c)).await;
        assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
        let ids: Vec<&str> = r.body["installed"].as_array().unwrap().iter().map(|e| e["id"].as_str().unwrap()).collect();
        assert_eq!(ids, vec!["net.teliti.c", "net.teliti.b", "net.teliti.a"]);
        let l = t.get("/api/extensions", Some(&c)).await.body;
        assert_eq!(l["extensions"].as_array().unwrap().len(), 3);

        // Sudah ada dan cocok: tidak dipasang ulang; daftar toko menandai yang terpasang.
        let list = t.get("/api/extensions/store", Some(&c)).await.body;
        let a = list["extensions"].as_array().unwrap().iter().find(|e| e["id"] == "net.teliti.a").unwrap();
        assert_eq!((a["upToDate"].as_bool(), a["alsoInstalls"].clone()), (Some(true), json!([])));
        let r = t.post("/api/extensions/store/install", json!({ "id": "net.teliti.alone" }), Some(&c)).await;
        assert_eq!(r.body["installed"].as_array().unwrap().len(), 1);

        // Id tak ada di katalog.
        let r = t.post("/api/extensions/store/install", json!({ "id": "net.teliti.hantu" }), Some(&c)).await;
        assert_eq!(r.status, StatusCode::NOT_FOUND);
        // Peran tanpa Admin Ubah ditolak.
        t.account("pantau", "role-monitor");
        let p = t.login("pantau").await;
        assert_eq!(t.post("/api/extensions/store/install", json!({ "id": "net.teliti.alone" }), Some(&p)).await.status, StatusCode::FORBIDDEN);
        assert_eq!(t.get("/api/extensions/store", Some(&p)).await.status, StatusCode::FORBIDDEN);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[tokio::test]
    async fn toko_menolak_katalog_dan_unduhan_yang_tidak_sah() {
        let d = dir();
        let t = harness_with(on(&d));
        t.account("admin", "role-admin");
        let c = t.login("admin").await;

        // Katalog bertanda tangan kunci lain: ditolak.
        let other = "0303030303030303030303030303030303030303030303030303030303030303";
        t.st.set_ext_fetcher(Arc::new(store(&[("net.teliti.a", "1.0.0", "")], other)));
        let r = t.get("/api/extensions/store", Some(&c)).await;
        assert_eq!((r.status, r.code()), (StatusCode::BAD_GATEWAY, "store_unreachable"));
        assert_eq!(t.post("/api/extensions/store/install", json!({ "id": "net.teliti.a" }), Some(&c)).await.status, StatusCode::BAD_GATEWAY);

        // Unduhan diubah setelah katalog dibuat: SHA-256 tidak cocok, tidak ada yang terpasang.
        let f = store(&[("net.teliti.a", "1.0.0", "")], OFFICIAL);
        f.0.lock().unwrap().insert("https://store.test/x/net.teliti.a-1.0.0.tmx".into(), pkg_json("net.teliti.a", "1.0.1", ""));
        t.st.set_ext_fetcher(Arc::new(f));
        let r = t.post("/api/extensions/store/install", json!({ "id": "net.teliti.a" }), Some(&c)).await;
        assert_eq!((r.status, r.code()), (StatusCode::BAD_GATEWAY, "store_corrupt"), "{:?}", r.body);
        assert!(t.get("/api/extensions", Some(&c)).await.body["extensions"].as_array().unwrap().is_empty());

        // Katalog menyebut Telemon yang tidak cocok: ditolak dengan alasan.
        let f = store(&[("net.teliti.n", "1.0.0", r#","telemon":">=9.0.0""#)], OFFICIAL);
        t.st.set_ext_fetcher(Arc::new(f));
        let r = t.post("/api/extensions/store/install", json!({ "id": "net.teliti.n" }), Some(&c)).await;
        assert_eq!((r.status, r.code()), (StatusCode::CONFLICT, "incompatible"));
        let l = t.get("/api/extensions/store", Some(&c)).await.body;
        assert!(l["extensions"][0]["problem"].as_str().unwrap().contains("9.0.0"));

        // Dependensi hilang dari katalog: ditolak, tidak ada yang terpasang sebagian.
        let f = store(&[("net.teliti.a", "1.0.0", r#","requires":[{"id":"net.teliti.b","version":"*"}]"#)], OFFICIAL);
        t.st.set_ext_fetcher(Arc::new(f));
        let r = t.post("/api/extensions/store/install", json!({ "id": "net.teliti.a" }), Some(&c)).await;
        assert_eq!(r.status, StatusCode::CONFLICT);
        assert!(t.get("/api/extensions", Some(&c)).await.body["extensions"].as_array().unwrap().is_empty());
        let _ = std::fs::remove_dir_all(&d);
    }

    // ---------------- backend WASM (ADR 0028) ----------------

    fn wat_str(s: &str) -> String {
        s.replace('\\', "\\\\").replace('"', "\\\"")
    }

    /// Modul yang memanggil satu operasi host dengan masukan tetap dan menjawab `{"status":200,"body":<jawaban host>}`.
    fn guest(op: &str, input: &str) -> Vec<u8> {
        wat::parse_str(format!(
            r#"(module
  (import "telemon" "call" (func $call (param i32 i32 i32 i32 i32 i32) (result i32)))
  (memory (export "memory") 2)
  (global $bump (mut i32) (i32.const 65536))
  (data (i32.const 0) "{op}")
  (data (i32.const 100) "{input}")
  (data (i32.const 2048) "{{\"status\":200,\"body\":")
  (func (export "telemon_alloc") (param $n i32) (result i32)
    (local $p i32) (local.set $p (global.get $bump)) (global.set $bump (i32.add (global.get $bump) (local.get $n))) (local.get $p))
  (func (export "telemon_handle") (param $ptr i32) (param $len i32) (result i64)
    (local $n i32)
    (local.set $n (call $call (i32.const 0) (i32.const {oplen}) (i32.const 100) (i32.const {inlen}) (i32.const 2069) (i32.const 60000)))
    (i32.store8 (i32.add (i32.const 2069) (local.get $n)) (i32.const 125))
    (i64.or (i64.shl (i64.const 2048) (i64.const 32)) (i64.extend_i32_u (i32.add (local.get $n) (i32.const 22))))))"#,
            op = wat_str(op),
            input = wat_str(input),
            oplen = op.len(),
            inlen = input.len()
        ))
        .unwrap()
    }

    fn guest_raw(body: &str) -> Vec<u8> {
        wat::parse_str(format!(
            r#"(module
  (import "telemon" "call" (func $call (param i32 i32 i32 i32 i32 i32) (result i32)))
  (memory (export "memory") 2)
  (func (export "telemon_alloc") (param i32) (result i32) (i32.const 65536))
  (func (export "telemon_handle") (param i32 i32) (result i64) {body}))"#
        ))
        .unwrap()
    }

    fn pkg_backend(id: &str, extra: &str, wasm: Vec<u8>, files: &[(&str, &str)]) -> Vec<u8> {
        let m = format!(r#"{{"id":"{id}","name":"{id}","version":"1.0.0","apiVersion":1,"publisher":"Teliti","backend":{{"entry":"backend.wasm"}}{extra}}}"#);
        let mut f: Vec<(String, Vec<u8>)> = vec![("manifest.json".into(), m.into_bytes()), ("backend.wasm".into(), wasm)];
        for (p, b) in files {
            f.push((p.to_string(), b.as_bytes().to_vec()));
        }
        pack(&f, Some(OFFICIAL)).unwrap()
    }

    async fn setup(t: &T, c: &str, id: &str, extra: &str, wasm: Vec<u8>, files: &[(&str, &str)]) {
        let r = install(t, pkg_backend(id, extra, wasm, files), c).await;
        assert_eq!(r.status, StatusCode::OK, "{id}: {:?}", r.body);
        patch(t, id, json!({ "enabled": true }), c).await;
    }

    const GO: &str = r#","contributes":{"routes":[{"id":"go","method":"GET","path":"/go"}]}"#;

    #[tokio::test]
    async fn backend_kv_dan_rute_dengan_akses_dan_aksi() {
        let d = dir();
        let t = harness_with(on(&d));
        t.account("admin", "role-admin");
        t.account("pantau", "role-monitor");
        let (a, p) = (t.login("admin").await, t.login("pantau").await);
        let id = "net.teliti.kv";
        let extra = r#","actions":[{"id":"write","label":"Tulis","level":"update"}],"contributes":{"routes":[
            {"id":"go","method":"GET","path":"/go"},
            {"id":"save","method":"POST","path":"/save","level":"update","action":"write"}]}"#;
        setup(&t, &a, id, extra, guest("kv.set", r#"{"key":"a","value":"1"}"#), &[]).await;

        let r = t.get(&format!("/api/ext/{id}/go"), Some(&a)).await;
        assert_eq!((r.status, r.body.clone()), (StatusCode::OK, json!({ "ok": null })), "{:?}", r.body);
        assert_eq!(t.telemon.with_store(|s| s.ext_kv_get(id, "a").unwrap()).as_deref(), Some("1"), "modul menulis lewat host API");

        // Rute tak dikenal, metode salah, dan jalur lain: 404.
        assert_eq!(t.get(&format!("/api/ext/{id}/tidak-ada"), Some(&a)).await.status, StatusCode::NOT_FOUND);
        assert_eq!(t.call(Method::POST, &format!("/api/ext/{id}/go"), Some(json!({})), Some(&a), &[], LAN).await.status, StatusCode::NOT_FOUND);
        assert_eq!(t.get("/api/ext/net.teliti.hantu/go", Some(&a)).await.status, StatusCode::NOT_FOUND);
        assert_eq!(t.get(&format!("/api/ext/{id}/go"), None).await.status, StatusCode::UNAUTHORIZED);

        // Peran tanpa akses ke extension: ditolak; dengan Lihat boleh GET; rute update butuh Ubah dan aksi.
        assert_eq!(t.get(&format!("/api/ext/{id}/go"), Some(&p)).await.status, StatusCode::FORBIDDEN);
        t.call(Method::PUT, &format!("/api/extensions/{id}/access/role-monitor"), Some(json!({ "level": "read" })), Some(&a), &[], LAN).await;
        assert_eq!(t.get(&format!("/api/ext/{id}/go"), Some(&p)).await.status, StatusCode::OK);
        let post = |ck: &str, extra: &[(&str, &str)], peer: &str| {
            let ck = ck.to_string();
            let extra: Vec<(String, String)> = extra.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect();
            let peer = peer.to_string();
            let t = &t;
            async move {
                let ex: Vec<(&str, &str)> = extra.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
                t.call(Method::POST, &format!("/api/ext/{id}/save"), Some(json!({ "x": 1 })), Some(&ck), &ex, &peer).await
            }
        };
        assert_eq!(post(&p, &[], LAN).await.status, StatusCode::FORBIDDEN, "level Lihat tidak cukup untuk rute update");
        t.call(Method::PUT, &format!("/api/extensions/{id}/access/role-monitor"), Some(json!({ "level": "update", "actions": [] })), Some(&a), &[], LAN).await;
        assert_eq!(post(&p, &[], LAN).await.status, StatusCode::FORBIDDEN, "tanpa aksi write");
        t.call(Method::PUT, &format!("/api/extensions/{id}/access/role-monitor"), Some(json!({ "level": "update", "actions": ["write"] })), Some(&a), &[], LAN).await;
        assert_eq!(post(&p, &[], LAN).await.status, StatusCode::OK);
        // Lewat akses Publik rute update ditolak walau punya aksi.
        assert_eq!(post(&a, &[("cf-connecting-ip", "203.0.113.9")], LOOP).await.status, StatusCode::FORBIDDEN);
        // Panggilan update tercatat; yang dimatikan tidak melayani.
        let rows = t.telemon.with_store(|s| s.list_audit(&telemon_core::AuditQuery::default()).unwrap());
        assert!(rows.iter().any(|r| r.entry.action == "extension.call" && r.entry.actor == "pantau"));
        patch(&t, id, json!({ "enabled": false }), &a).await;
        assert_eq!(t.get(&format!("/api/ext/{id}/go"), Some(&a)).await.status, StatusCode::NOT_FOUND);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[tokio::test]
    async fn host_api_menegakkan_izin_extension_dan_peran_pengguna() {
        let d = dir();
        let t = harness_with(on(&d));
        t.account("admin", "role-admin");
        let a = t.login("admin").await;
        let id = "net.teliti.ev";
        let extra = r#","permissions":[{"id":"data.events"},{"id":"data.routers","sensitive":true}],"contributes":{"routes":[{"id":"go","method":"GET","path":"/go"}]}"#;
        setup(&t, &a, id, extra, guest("events.list", r#"{"limit":5}"#), &[]).await;
        let go = |ck: &str| {
            let ck = ck.to_string();
            let t = &t;
            async move { t.get(&format!("/api/ext/{id}/go"), Some(&ck)).await }
        };
        // Toggle events menyala bawaan: jawaban `ok` (daftar kosong).
        assert_eq!(go(&a).await.body, json!({ "ok": [] }));
        // Toggle dimatikan: modul menerima permission_denied, tidak ada data.
        patch(&t, id, json!({ "perms": { "data.events": false } }), &a).await;
        assert_eq!(go(&a).await.body["error"]["code"], "permission_denied");
        patch(&t, id, json!({ "perms": { "data.events": true } }), &a).await;
        assert_eq!(go(&a).await.body, json!({ "ok": [] }));
        // Peran punya akses ke extension tetapi tidak punya izin Event: modul ditolak walau toggle menyala.
        let role = t.auth.create_role(None, &RoleInput { name: "tanpa-event".into(), description: String::new(), permissions: Permissions::default().with(Resource::Ringkasan, Level::Read) }).unwrap();
        t.account("buta", &role.id);
        let b = t.login("buta").await;
        t.call(Method::PUT, &format!("/api/extensions/{id}/access/{}", role.id), Some(json!({ "level": "read" })), Some(&a), &[], LAN).await;
        assert_eq!(go(&b).await.body["error"]["code"], "permission_denied");

        // Router: izin sensitif mati bawaan, daftar putih jalur, kata sandi tidak pernah keluar.
        let rid = t.router(false);
        let print = |path: &str| guest("router.print", &format!(r#"{{"router":"{rid}","path":"{path}"}}"#));
        setup(&t, &a, "net.teliti.rt", r#","permissions":[{"id":"data.routers","sensitive":true}],"contributes":{"routes":[{"id":"go","method":"GET","path":"/go"}]}"#, print("/system/resource"), &[]).await;
        let rt = |ck: &str| {
            let ck = ck.to_string();
            let t = &t;
            async move { t.get("/api/ext/net.teliti.rt/go", Some(&ck)).await }
        };
        assert_eq!(rt(&a).await.body["error"]["code"], "permission_denied", "izin sensitif mulai mati");
        patch(&t, "net.teliti.rt", json!({ "perms": { "data.routers": true } }), &a).await;
        let r = rt(&a).await;
        assert_eq!(r.body["ok"][0]["board-name"], "RB4011", "{:?}", r.body);
        setup(&t, &a, "net.teliti.rt2", r#","permissions":[{"id":"data.routers","sensitive":true}],"contributes":{"routes":[{"id":"go","method":"GET","path":"/go"}]}"#, print("/ppp/secret"), &[]).await;
        patch(&t, "net.teliti.rt2", json!({ "perms": { "data.routers": true } }), &a).await;
        let r = t.get("/api/ext/net.teliti.rt2/go", Some(&a)).await;
        assert_eq!(r.body["error"]["code"], "path_not_allowed", "kata sandi pelanggan tidak bisa dibaca: {:?}", r.body);
        assert!(!r.body.to_string().contains("rahasia123"));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[tokio::test]
    async fn host_api_sql_dan_batas_namespace() {
        let d = dir();
        let t = harness_with(on(&d));
        t.account("admin", "role-admin");
        let a = t.login("admin").await;
        let mig = r#"{"ops":[{"op":"create_table","table":"x_net_teliti_sql__notes","columns":[{"name":"id","type":"integer","primaryKey":true,"autoIncrement":true},{"name":"text","type":"text"}]}]}"#;
        // Tulis lalu baca lewat dua extension berbagi namespace? Tidak: tiap extension punya namespace sendiri.
        setup(&t, &a, "net.teliti.sql", GO, guest("db.exec", r#"{"sql":"INSERT INTO x_net_teliti_sql__notes (text) VALUES (?1)","params":["halo"]}"#), &[("migrations/0001-init.json", mig)]).await;
        let r = t.get("/api/ext/net.teliti.sql/go", Some(&a)).await;
        assert_eq!(r.body, json!({ "ok": { "changes": 1 } }), "{:?}", r.body);
        let rows = t.telemon.with_store(|s| s.ext_db_query("x_net_teliti_sql__", "SELECT text FROM x_net_teliti_sql__notes", &[], 10).unwrap());
        assert_eq!(rows.rows[0][0], telemon_core::DbValue::Text("halo".into()));
        // Mencoba membaca tabel inti: ditolak oleh authorizer.
        setup(&t, &a, "net.teliti.evil", GO, guest("db.query", r#"{"sql":"SELECT * FROM accounts"}"#), &[]).await;
        let r = t.get("/api/ext/net.teliti.evil/go", Some(&a)).await;
        assert_eq!(r.body["error"]["code"], "db", "{:?}", r.body);
        // Tabel milik extension lain juga tidak terbaca.
        setup(&t, &a, "net.teliti.evil2", GO, guest("db.query", r#"{"sql":"SELECT * FROM x_net_teliti_sql__notes"}"#), &[]).await;
        assert_eq!(t.get("/api/ext/net.teliti.evil2/go", Some(&a)).await.body["error"]["code"], "db");
        // Operasi host tak dikenal.
        setup(&t, &a, "net.teliti.unk", GO, guest("fs.read", r#"{}"#), &[]).await;
        assert_eq!(t.get("/api/ext/net.teliti.unk/go", Some(&a)).await.body["error"]["code"], "unknown_op");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[tokio::test]
    async fn backend_yang_rusak_tidak_menjatuhkan_server() {
        let d = dir();
        let t = harness_with(on(&d));
        t.account("admin", "role-admin");
        let a = t.login("admin").await;
        // Trap: 502 dan baris log; loop tanpa akhir: 504 (habis bahan bakar); jawaban bukan JSON: 502.
        setup(&t, &a, "net.teliti.trap", GO, guest_raw("unreachable"), &[]).await;
        let r = t.get("/api/ext/net.teliti.trap/go", Some(&a)).await;
        assert_eq!((r.status, r.code()), (StatusCode::BAD_GATEWAY, "backend_error"));
        let logs = t.get("/api/extensions/net.teliti.trap/logs", Some(&a)).await.body;
        assert!(logs["lines"][0].as_str().unwrap().contains("galat backend"), "{logs:?}");
        setup(&t, &a, "net.teliti.loop", GO, guest_raw("(loop $l (br $l)) (i64.const 0)"), &[]).await;
        let r = t.get("/api/ext/net.teliti.loop/go", Some(&a)).await;
        assert_eq!((r.status, r.code()), (StatusCode::GATEWAY_TIMEOUT, "backend_timeout"));
        setup(&t, &a, "net.teliti.junk", GO, guest_raw("(i64.or (i64.shl (i64.const 100) (i64.const 32)) (i64.const 4))"), &[]).await;
        assert_eq!(t.get("/api/ext/net.teliti.junk/go", Some(&a)).await.status, StatusCode::BAD_GATEWAY);
        // Server tetap melayani.
        assert_eq!(t.get("/api/extensions", Some(&a)).await.status, StatusCode::OK);
        // Modul tidak valid atau memakai impor terlarang ditolak saat pasang.
        let bad = pkg_backend("net.teliti.bad", "", b"bukan wasm".to_vec(), &[]);
        assert_eq!(install(&t, bad, &a).await.status, StatusCode::BAD_REQUEST);
        let wasi = wat::parse_str(r#"(module (import "wasi_snapshot_preview1" "fd_write" (func (param i32 i32 i32 i32) (result i32))) (memory (export "memory") 1) (func (export "telemon_alloc") (param i32) (result i32) (i32.const 0)) (func (export "telemon_handle") (param i32 i32) (result i64) (i64.const 0)))"#).unwrap();
        assert_eq!(install(&t, pkg_backend("net.teliti.wasi", "", wasi, &[]), &a).await.status, StatusCode::BAD_REQUEST);
        // Rute atau worker tanpa backend ditolak di manifest.
        let m = r#"{"id":"net.teliti.nob","name":"n","version":"1.0.0","apiVersion":1,"publisher":"T","contributes":{"routes":[{"id":"go","method":"GET","path":"/go"}]}}"#;
        let nob = pack(&[("manifest.json".into(), m.as_bytes().to_vec())], Some(OFFICIAL)).unwrap();
        assert_eq!(install(&t, nob, &a).await.status, StatusCode::BAD_REQUEST);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[tokio::test]
    async fn worker_terjadwal_berjalan_dijeda_dan_dibuka_manual() {
        let d = dir();
        let t = harness_with(on(&d));
        t.account("admin", "role-admin");
        let a = t.login("admin").await;
        let wk = r#","contributes":{"workers":[{"id":"tick","everySeconds":10}]}"#;
        setup(&t, &a, "net.teliti.wk", wk, guest("kv.set", r#"{"key":"ran","value":"yes"}"#), &[]).await;
        let state = |t: &T| t.st_workers("net.teliti.wk/tick");
        telemon_http::run_due_workers(&t.st).await;
        assert_eq!(t.telemon.with_store(|s| s.ext_kv_get("net.teliti.wk", "ran").unwrap()).as_deref(), Some("yes"), "worker menjalankan modul");
        let first = state(&t).last_run;
        // Belum jatuh tempo: tidak berjalan lagi. Setelah 10 detik: berjalan.
        telemon_http::run_due_workers(&t.st).await;
        assert_eq!(state(&t).last_run, first);
        t.clock.fetch_add(10_000, Ordering::SeqCst);
        telemon_http::run_due_workers(&t.st).await;
        assert_ne!(state(&t).last_run, first);
        assert_eq!(t.get("/api/extensions", Some(&a)).await.body["extensions"][0]["workers"][0]["state"]["lastOk"], true);

        // Worker yang gagal terus dijeda setelah 5 kali; menjalankan manual membuka jeda.
        setup(&t, &a, "net.teliti.bad", wk, guest_raw("unreachable"), &[]).await;
        for _ in 0..8 {
            t.clock.fetch_add(70_000, Ordering::SeqCst);
            telemon_http::run_due_workers(&t.st).await;
        }
        let s = t.st_workers("net.teliti.bad/tick");
        assert!(s.paused && s.failures == 5 && s.last_ok == Some(false), "{s:?}");
        let logs = t.get("/api/extensions/net.teliti.bad/logs", Some(&a)).await.body;
        assert!(logs["lines"].as_array().map_or(0, |l| l.len()) >= 5, "{logs:?}");
        let r = t.call(Method::POST, "/api/extensions/net.teliti.bad/workers/tick/run", Some(json!({})), Some(&a), &[], LAN).await;
        assert_eq!((r.status, r.body["ok"].as_bool()), (StatusCode::OK, Some(false)));
        assert!(!t.st_workers("net.teliti.bad/tick").paused || t.st_workers("net.teliti.bad/tick").failures == 1, "jeda dibuka lalu hitung ulang");
        // Worker extension yang dimatikan tidak berjalan; peran tanpa Ubah tidak boleh memicu.
        patch(&t, "net.teliti.wk", json!({ "enabled": false }), &a).await;
        let before = state(&t).last_run;
        t.clock.fetch_add(60_000, Ordering::SeqCst);
        telemon_http::run_due_workers(&t.st).await;
        assert_eq!(state(&t).last_run, before);
        t.account("pantau", "role-monitor");
        let p = t.login("pantau").await;
        assert_eq!(t.call(Method::POST, "/api/extensions/net.teliti.wk/workers/tick/run", Some(json!({})), Some(&p), &[], LAN).await.status, StatusCode::FORBIDDEN);
        let _ = std::fs::remove_dir_all(&d);
    }

    // ---------------- terminal SSH (ADR 0029) ----------------

    mod ssh {
        use russh::keys::{Algorithm, HashAlg, PrivateKey};
        use russh::server::{self, Auth, Msg, Server as _, Session};
        use russh::{Channel, ChannelId, Pty};

        #[derive(Clone)]
        pub struct Fake;

        impl server::Server for Fake {
            type Handler = Self;
            fn new_client(&mut self, _: Option<std::net::SocketAddr>) -> Self {
                self.clone()
            }
        }

        impl server::Handler for Fake {
            type Error = russh::Error;
            async fn auth_password(&mut self, user: &str, password: &str) -> Result<Auth, Self::Error> {
                Ok(if user == "api" && password == "rahasia" { Auth::Accept } else { Auth::reject() })
            }
            async fn channel_open_session(&mut self, _c: Channel<Msg>, reply: server::ChannelOpenHandle, _s: &mut Session) -> Result<(), Self::Error> {
                reply.accept().await;
                Ok(())
            }
            async fn pty_request(&mut self, channel: ChannelId, _t: &str, _c: u32, _r: u32, _pw: u32, _ph: u32, _m: &[(Pty, u32)], session: &mut Session) -> Result<(), Self::Error> {
                session.channel_success(channel)?;
                Ok(())
            }
            async fn shell_request(&mut self, channel: ChannelId, session: &mut Session) -> Result<(), Self::Error> {
                session.channel_success(channel)?;
                session.data(channel, b"[api@router] > ".to_vec())?;
                Ok(())
            }
            async fn data(&mut self, channel: ChannelId, data: &[u8], session: &mut Session) -> Result<(), Self::Error> {
                let mut out = b"echo:".to_vec();
                out.extend_from_slice(data);
                session.data(channel, out)?;
                Ok(())
            }
        }

        /// Server SSH tiruan di port acak; mengembalikan port dan sidik jari kunci host.
        pub async fn start() -> (u16, String) {
            let key = PrivateKey::random(&mut rand::rng(), Algorithm::Ed25519).unwrap();
            let fp = key.public_key().fingerprint(HashAlg::Sha256).to_string();
            let config = std::sync::Arc::new(server::Config { keys: vec![key], auth_rejection_time: std::time::Duration::from_millis(1), auth_rejection_time_initial: Some(std::time::Duration::from_millis(0)), ..Default::default() });
            let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
            let port = listener.local_addr().unwrap().port();
            let mut srv = Fake;
            tokio::spawn(async move {
                let _ = srv.run_on_socket(config, &listener).await;
            });
            (port, fp)
        }
    }

    const TERM_EXTRA: &str = r#","actions":[{"id":"terminal","label":"Terminal","level":"update"}],
        "permissions":[{"id":"feature.router-ssh","sensitive":true}],
        "contributes":{"sessions":[{"id":"ssh","kind":"router-ssh","action":"terminal"}]}"#;

    fn ssh_router(t: &T, can_write: bool) -> String {
        let n = telemon_core::store::NewRouter { name: format!("ssh-router-{can_write}"), host: "127.0.0.1".into(), port: 8728, tls_pin: None, username: "api".into(), can_write };
        t.telemon.add_router(&n, "rahasia").unwrap().id
    }

    async fn ws_connect(addr: std::net::SocketAddr, ext: &str, router: &str, cookie: &str) -> Result<tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>, u16> {
        use tokio_tungstenite::tungstenite::client::IntoClientRequest;
        let mut req = format!("ws://{addr}/api/ext-session/{ext}/ssh?router={router}&cols=100&rows=30").into_client_request().unwrap();
        req.headers_mut().insert("cookie", format!("telemon_session={cookie}").parse().unwrap());
        req.headers_mut().insert("origin", format!("http://{addr}").parse().unwrap());
        match tokio_tungstenite::connect_async(req).await {
            Ok((ws, _)) => Ok(ws),
            Err(tokio_tungstenite::tungstenite::Error::Http(r)) => Err(r.status().as_u16()),
            Err(e) => panic!("galat tak terduga: {e}"),
        }
    }

    async fn next_text(ws: &mut tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>, needle: &str) -> String {
        use futures_util::StreamExt;
        let mut all = String::new();
        let r = tokio::time::timeout(Duration::from_secs(5), async {
            while !all.contains(needle) {
                match ws.next().await {
                    Some(Ok(tokio_tungstenite::tungstenite::Message::Binary(b))) => all.push_str(&String::from_utf8_lossy(&b)),
                    Some(Ok(_)) => {}
                    other => panic!("sesi berakhir sebelum {needle:?}: {other:?} (diterima: {all:?})"),
                }
            }
        })
        .await;
        assert!(r.is_ok(), "waktu habis menunggu {needle:?}; diterima {all:?}");
        all
    }

    #[tokio::test]
    async fn terminal_ssh_pin_izin_dan_sesi() {
        use futures_util::SinkExt;
        use tokio_tungstenite::tungstenite::Message as M;
        let d = dir();
        let t = harness_with(on(&d));
        t.account("admin", "role-admin");
        t.account("pantau", "role-monitor");
        let (a, p) = (t.login("admin").await, t.login("pantau").await);
        let (port, fp) = ssh::start().await;
        let rid = ssh_router(&t, true);
        let id = "net.teliti.term";
        let manifest = format!(r#"{{"id":"{id}","name":"Term","version":"1.0.0","apiVersion":1,"publisher":"Teliti","ui":{{"entry":"ui/index.js"}}{TERM_EXTRA}}}"#);
        let bytes = pack(&[("manifest.json".into(), manifest.into_bytes()), ("ui/index.js".into(), b"export default {}".to_vec())], Some(OFFICIAL)).unwrap();
        assert_eq!(install(&t, bytes, &a).await.status, StatusCode::OK);
        patch(&t, id, json!({ "enabled": true }), &a).await;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let st = t.st.clone();
        tokio::spawn(async move { telemon_http::serve_on(st, listener, std::future::pending()).await });

        // Pin kunci host: butuh Router Ubah; sidik jari harus sama dengan yang router tawarkan sekarang.
        t.call(Method::PUT, &format!("/api/routers/{rid}/ssh"), Some(json!({ "port": port })), Some(&a), &[], LAN).await;
        let g = t.get(&format!("/api/routers/{rid}/ssh"), Some(&a)).await;
        assert_eq!((g.status, g.body["presented"].as_str(), g.body["pinned"].is_null()), (StatusCode::OK, Some(fp.as_str()), true), "{:?}", g.body);
        let other = t.get(&format!("/api/routers/{rid}/ssh?port=1"), Some(&a)).await;
        assert_eq!((other.status, other.body["presented"].is_null(), other.body["error"].is_string(), other.body["savedPort"].as_u64()), (StatusCode::OK, true, true, Some(port as u64)), "port lain diperiksa tanpa mengubah yang tersimpan");
        assert_eq!(t.get(&format!("/api/routers/{rid}/ssh"), Some(&p)).await.status, StatusCode::FORBIDDEN, "peran tanpa Router Ubah");
        let bad = t.call(Method::PUT, &format!("/api/routers/{rid}/ssh"), Some(json!({ "fingerprint": "SHA256:salah" })), Some(&a), &[], LAN).await;
        assert_eq!((bad.status, bad.code()), (StatusCode::CONFLICT, "ssh_host_key_changed"));

        // Sebelum pin, sesi ditolak (409). Setelah itu syarat lain diperiksa satu per satu.
        assert_eq!(ws_connect(addr, id, &rid, &a).await.err(), Some(403), "izin feature.router-ssh mulai mati");
        patch(&t, id, json!({ "perms": { "feature.router-ssh": true } }), &a).await;
        assert_eq!(ws_connect(addr, id, &rid, &a).await.err(), Some(409), "belum disematkan");
        let ok = t.call(Method::PUT, &format!("/api/routers/{rid}/ssh"), Some(json!({ "fingerprint": fp })), Some(&a), &[], LAN).await;
        assert_eq!((ok.status, ok.body["pinned"].as_str()), (StatusCode::OK, Some(fp.as_str())), "{:?}", ok.body);
        assert_eq!(ws_connect(addr, id, &rid, &p).await.err(), Some(403), "peran tanpa akses extension");
        assert_eq!(ws_connect(addr, "net.teliti.hantu", &rid, &a).await.err(), Some(404));
        let ro = ssh_router(&t, false);
        assert_eq!(ws_connect(addr, id, &ro, &a).await.err(), Some(403), "router tanpa izin tulis");

        // Sesi sungguhan: prompt, masukan, ubah ukuran, tutup; audit mencatat buka, tutup, dan baris perintah.
        let mut ws = ws_connect(addr, id, &rid, &a).await.expect("terhubung");
        next_text(&mut ws, "[api@router] > ").await;
        ws.send(M::Binary(b"/ip address print\r".to_vec().into())).await.unwrap();
        next_text(&mut ws, "echo:/ip address print").await;
        ws.send(M::Text(json!({ "type": "resize", "cols": 120, "rows": 40 }).to_string().into())).await.unwrap();
        ws.send(M::Close(None)).await.unwrap();
        drop(ws);
        let mut closed = false;
        for _ in 0..50 {
            tokio::time::sleep(Duration::from_millis(100)).await;
            let rows = t.telemon.with_store(|s| s.list_audit(&telemon_core::AuditQuery::default()).unwrap());
            if rows.iter().any(|r| r.entry.action == "extension.ssh") {
                closed = true;
                let open = rows.iter().find(|r| r.entry.action == "extension.ssh_open").expect("catatan buka");
                assert_eq!(open.entry.actor, "admin");
                let close = rows.iter().find(|r| r.entry.action == "extension.ssh").unwrap();
                assert!(close.entry.changes.iter().any(|c| c.field == "perintah" && c.to.as_deref() == Some("/ip address print")), "{:?}", close.entry.changes);
                assert!(!format!("{:?}", close.entry).contains("rahasia"), "kata sandi tidak boleh masuk audit");
                break;
            }
        }
        assert!(closed, "catatan penutupan tidak muncul");

        // Kunci host berubah (pin lama salah): ditolak sebelum kata sandi dikirim.
        t.telemon.with_store(|s| s.router_ssh_set(&rid, port, Some("SHA256:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"), 1)).unwrap();
        assert_eq!(ws_connect(addr, id, &rid, &a).await.err(), Some(409), "kunci host tidak cocok");
        t.telemon.with_store(|s| s.router_ssh_set(&rid, port, Some(&fp), 2)).unwrap();

        // Batas sesi per akun: 3 terbuka, yang keempat 429; setelah satu ditutup, bisa lagi.
        let mut open = Vec::new();
        for _ in 0..3 {
            let mut w = ws_connect(addr, id, &rid, &a).await.expect("sesi");
            next_text(&mut w, "[api@router] > ").await;
            open.push(w);
        }
        assert_eq!(ws_connect(addr, id, &rid, &a).await.err(), Some(429));
        let mut first = open.remove(0);
        first.send(M::Close(None)).await.unwrap();
        drop(first);
        let mut again = None;
        for _ in 0..50 {
            tokio::time::sleep(Duration::from_millis(100)).await;
            if let Ok(w) = ws_connect(addr, id, &rid, &a).await {
                again = Some(w);
                break;
            }
        }
        assert!(again.is_some(), "slot dilepas setelah sesi ditutup");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[tokio::test]
    async fn izin_akun_dan_akses_publik() {
        let d = dir();
        let t = harness_with(on(&d));
        t.account("admin", "role-admin");
        t.account("pantau", "role-monitor");
        let (a, p) = (t.login("admin").await, t.login("pantau").await);
        install(&t, pkg("1.0.0", "", Some(OFFICIAL)), &a).await;
        patch(&t, "net.teliti.demo", json!({ "enabled": true }), &a).await;
        // Tanpa akses di halaman Akses: extension tidak terlihat dan berkas UI-nya 404. Setelah diberi Lihat: terlihat.
        assert!(t.get("/api/extensions/active", Some(&p)).await.body["extensions"].as_array().unwrap().is_empty());
        assert_eq!(t.get("/ext/net.teliti.demo/ui/index.js", Some(&p)).await.status, StatusCode::NOT_FOUND);
        let g = t.call(Method::PUT, "/api/extensions/net.teliti.demo/access/role-monitor", Some(json!({ "level": "read" })), Some(&a), &[], LAN).await;
        assert_eq!(g.status, StatusCode::OK, "{:?}", g.body);
        // Peran tanpa izin Admin: tidak bisa mengelola, tetapi UI tetap memuat extension aktif.
        assert_eq!(t.get("/api/extensions", Some(&p)).await.status, StatusCode::FORBIDDEN);
        assert_eq!(install(&t, pkg("1.0.0", "", Some(OFFICIAL)), &p).await.status, StatusCode::FORBIDDEN);
        assert_eq!(patch(&t, "net.teliti.demo", json!({ "enabled": false }), &p).await.status, StatusCode::FORBIDDEN);
        assert_eq!(t.call(Method::DELETE, "/api/extensions/net.teliti.demo", None, Some(&p), &[], LAN).await.status, StatusCode::FORBIDDEN);
        assert_eq!(t.get("/api/extensions/active", Some(&p)).await.body["extensions"].as_array().unwrap().len(), 1);
        assert_eq!(t.get("/ext/net.teliti.demo/ui/index.js", Some(&p)).await.status, StatusCode::OK);
        // Lewat akses Publik, admin pun tidak boleh mengelola.
        let pub_hdr = [("cf-connecting-ip", "203.0.113.9")];
        let r = send(&t, Method::POST, "/api/extensions", pkg("1.1.0", "", Some(OFFICIAL)), &a, &pub_hdr, LOOP).await;
        assert_eq!(r.status, StatusCode::FORBIDDEN, "{:?}", r.body);
        let r = t.call(Method::PATCH, "/api/extensions/net.teliti.demo", Some(json!({ "enabled": false })), Some(&a), &pub_hdr, LOOP).await;
        assert_eq!(r.status, StatusCode::FORBIDDEN);
        let _ = std::fs::remove_dir_all(&d);
    }
}
