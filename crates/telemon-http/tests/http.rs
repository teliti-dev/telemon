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
use telemon_http::{AppState, HttpConfig, app};
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
    let telemon = Arc::new(Telemon::new(store, secrets, conn, RuntimeConfig { poll_every: Duration::from_millis(20), sync_every: None }));
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
