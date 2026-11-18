//! Server akun untuk akses non-desktop (ADR 0005, 0006): login argon2id, 2FA TOTP + kode pemulihan,
//! sesi bertoken, peran dengan izin per menu, dan pengaman anti-kenaikan izin.
//!
//! Semua metode **sinkron** dan berat di CPU (argon2): panggil dari `spawn_blocking` di server HTTP.
//! `caller: Option<&Session>` = `None` berarti aplikasi desktop (tanpa login, boleh semuanya).

use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex};

use argon2::password_hash::phc::PasswordHash;
use argon2::password_hash::{PasswordHasher, PasswordVerifier};
use argon2::{Algorithm, Argon2, Params, Version};
use hmac::{Hmac, KeyInit, Mac};
use rusqlite::{OptionalExtension, params};
use sha1::Sha1;
use sha2::{Digest, Sha256};

use crate::monitor::SharedStore;
use crate::store::{AuditEntry, AuditOutcome, Store};
use crate::vault::generate_key;
use crate::write::Change;

const MIN_PASSWORD: usize = 8;
const CHALLENGE_TTL_MS: i64 = 5 * 60_000;
const RECOVERY_COUNT: usize = 8;

// ---------------- izin ----------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Resource {
    Ringkasan,
    User,
    Profile,
    Event,
    Router,
    Admin,
    Pengaturan,
    Ai,
}

impl Resource {
    pub const ALL: [Resource; 8] = [Resource::Ringkasan, Resource::User, Resource::Profile, Resource::Event, Resource::Router, Resource::Admin, Resource::Pengaturan, Resource::Ai];

    pub fn as_str(self) -> &'static str {
        match self {
            Resource::Ringkasan => "ringkasan",
            Resource::User => "user",
            Resource::Profile => "profile",
            Resource::Event => "event",
            Resource::Router => "router",
            Resource::Admin => "admin",
            Resource::Pengaturan => "pengaturan",
            Resource::Ai => "ai",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|r| r.as_str() == s)
    }

    /// Menu tanpa fitur ubah hanya boleh Tanpa akses atau Lihat.
    pub fn max_level(self) -> Level {
        match self {
            Resource::Ringkasan | Resource::Event | Resource::Ai => Level::Read,
            _ => Level::Update,
        }
    }

    fn idx(self) -> usize {
        Self::ALL.iter().position(|r| *r == self).expect("ada")
    }
}

/// Urutan menentukan peringkat: `Update` mencakup `Read`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum Level {
    #[default]
    None,
    Read,
    Update,
}

impl Level {
    pub fn as_str(self) -> &'static str {
        match self {
            Level::None => "none",
            Level::Read => "read",
            Level::Update => "update",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "none" => Level::None,
            "read" => Level::Read,
            "update" => Level::Update,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Permissions([Level; 8]);

impl Permissions {
    pub fn get(&self, r: Resource) -> Level {
        self.0[r.idx()]
    }
    pub fn set(&mut self, r: Resource, l: Level) {
        self.0[r.idx()] = l;
    }
    pub fn with(mut self, r: Resource, l: Level) -> Self {
        self.set(r, l);
        self
    }
    pub fn has_update(&self) -> bool {
        self.0.contains(&Level::Update)
    }
    /// `ringkasan=read,user=update,...`
    pub fn encode(&self) -> String {
        Resource::ALL.iter().map(|r| format!("{}={}", r.as_str(), self.get(*r).as_str())).collect::<Vec<_>>().join(",")
    }
    /// Kolom yang hilang atau tidak dikenal dianggap Tanpa akses (bawaan aman).
    pub fn parse(s: &str) -> Self {
        let mut p = Permissions::default();
        for kv in s.split(',') {
            if let Some((k, v)) = kv.split_once('=')
                && let (Some(r), Some(l)) = (Resource::parse(k.trim()), Level::parse(v.trim()))
            {
                p.set(r, l.min(r.max_level()));
            }
        }
        p
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Role {
    pub id: String,
    pub name: String,
    pub description: String,
    pub permissions: Permissions,
    pub builtin: bool,
    pub account_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoleInput {
    pub name: String,
    pub description: String,
    pub permissions: Permissions,
}

fn default_roles() -> Vec<(&'static str, &'static str, &'static str, Permissions)> {
    use Level::{None as N, Read as R, Update as U};
    let p = |v: [Level; 8]| Permissions(v);
    // urutan: ringkasan, user, profile, event, router, admin, pengaturan, ai
    vec![
        ("role-admin", "Administrator", "Semua menu, termasuk Admin dan Pengaturan; boleh mengubah semuanya.", p([R, U, U, R, U, U, U, R])),
        ("role-operator", "Operator", "Mengelola user dan profil PPP; router hanya dilihat.", p([R, U, U, R, R, N, N, R])),
        ("role-viewer", "Viewer", "Hanya melihat semua menu.", p([R, R, R, R, R, N, N, N])),
        ("role-monitor", "Monitor", "Layar pantau: Ringkasan, Event, dan Router saja.", p([R, N, N, R, R, N, N, N])),
    ]
}

// ---------------- sesi dan kebijakan ----------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Via {
    Lan,
    Publik,
}

impl Via {
    fn as_str(self) -> &'static str {
        match self {
            Via::Lan => "lan",
            Via::Publik => "publik",
        }
    }
    fn parse(s: &str) -> Self {
        if s == "publik" { Via::Publik } else { Via::Lan }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    pub id: String,
    pub account_id: String,
    pub username: String,
    pub display_name: String,
    pub role_id: String,
    pub role_name: String,
    pub permissions: Permissions,
    pub via: Via,
    pub two_factor: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct AuthPolicy {
    /// Wajib 2FA untuk semua akun (saklar pemilik).
    pub require_2fa: bool,
    /// Akses Publik sedang aktif: 2FA wajib untuk semua.
    pub public_on: bool,
    /// Peran berizin Ubah boleh menulis lewat akses Publik (bawaan mati).
    pub allow_public_write: bool,
    pub session_hours: u32,
    pub max_failures: u32,
    pub lock_minutes: u32,
}

impl Default for AuthPolicy {
    fn default() -> Self {
        Self { require_2fa: false, public_on: false, allow_public_write: false, session_hours: 12, max_failures: 5, lock_minutes: 5 }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct HashParams {
    pub memory_kib: u32,
    pub iterations: u32,
    pub parallelism: u32,
}

impl Default for HashParams {
    /// Rekomendasi OWASP untuk argon2id: 19 MiB, 2 iterasi, 1 jalur.
    fn default() -> Self {
        Self { memory_kib: 19 * 1024, iterations: 2, parallelism: 1 }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountSummary {
    pub id: String,
    pub username: String,
    pub display_name: String,
    pub role_id: String,
    pub role_name: String,
    pub two_factor: bool,
    pub disabled: bool,
    pub created_at: i64,
    pub last_login_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewAccount {
    pub username: String,
    pub display_name: String,
    pub password: String,
    pub role_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionDevice {
    pub id: String,
    pub device: String,
    pub ip: String,
    pub via: Via,
    pub created_at: i64,
    pub last_seen_at: i64,
    pub current: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoginOutcome {
    Ok { token: String, session: Session },
    TwoFactorRequired { challenge: String },
    TwoFactorSetupRequired { challenge: String, secret: String, otpauth_url: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Authenticated {
    pub token: String,
    pub session: Session,
}

// ---------------- galat ----------------

/// Galat dengan kode stabil (`code()`) agar UI menerjemahkannya (ADR 0008).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthError {
    Invalid(String),
    BadCredentials,
    Locked { minutes: u32 },
    Disabled,
    BadCode { recovery: bool },
    Unauthenticated,
    Forbidden(String),
    NotFound,
    /// Penyimpanan rahasia terkunci (kunci enkripsi belum dipasang).
    VaultLocked,
    Db(String),
}

impl AuthError {
    pub fn code(&self) -> &'static str {
        match self {
            AuthError::Invalid(_) => "invalid",
            AuthError::BadCredentials => "bad_credentials",
            AuthError::Locked { .. } => "locked",
            AuthError::Disabled => "disabled",
            AuthError::BadCode { .. } => "bad_code",
            AuthError::Unauthenticated => "unauthenticated",
            AuthError::Forbidden(_) => "forbidden",
            AuthError::NotFound => "not_found",
            AuthError::VaultLocked => "vault_locked",
            AuthError::Db(_) => "db",
        }
    }
}

impl fmt::Display for AuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AuthError::Invalid(m) | AuthError::Forbidden(m) | AuthError::Db(m) => f.write_str(m),
            AuthError::BadCredentials => f.write_str("Username atau password salah"),
            AuthError::Locked { minutes } => write!(f, "Terlalu banyak percobaan. Coba lagi dalam {minutes} menit"),
            AuthError::Disabled => f.write_str("Akun dinonaktifkan. Hubungi pemilik Telemon"),
            AuthError::BadCode { recovery: true } => f.write_str("Kode pemulihan salah atau sudah dipakai"),
            AuthError::BadCode { recovery: false } => f.write_str("Kode salah"),
            AuthError::Unauthenticated => f.write_str("Belum login"),
            AuthError::NotFound => f.write_str("Tidak ditemukan"),
            AuthError::VaultLocked => f.write_str("Penyimpanan rahasia terkunci"),
        }
    }
}

impl std::error::Error for AuthError {}

impl From<rusqlite::Error> for AuthError {
    fn from(e: rusqlite::Error) -> Self {
        AuthError::Db(e.to_string())
    }
}

type Res<T> = Result<T, AuthError>;

fn invalid<T>(m: &str) -> Res<T> {
    Err(AuthError::Invalid(m.into()))
}
fn forbidden<T>(m: &str) -> Res<T> {
    Err(AuthError::Forbidden(m.into()))
}

// ---------------- kripto kecil ----------------

fn sha256_hex(data: &[u8]) -> String {
    Sha256::digest(data).as_slice().iter().map(|b| format!("{b:02x}")).collect()
}

fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn random_bytes<const N: usize>() -> Res<[u8; N]> {
    let k = generate_key().map_err(|_| AuthError::Db("pembangkit acak gagal".into()))?;
    let mut out = [0u8; N];
    out.copy_from_slice(&k[..N]);
    Ok(out)
}

fn random_hex(n_bytes: usize) -> Res<String> {
    let k = generate_key().map_err(|_| AuthError::Db("pembangkit acak gagal".into()))?;
    Ok(k[..n_bytes.min(32)].iter().map(|b| format!("{b:02x}")).collect())
}

const B32: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

pub fn base32_encode(data: &[u8]) -> String {
    let (mut out, mut buf, mut bits) = (String::new(), 0u32, 0u32);
    for &b in data {
        buf = (buf << 8) | u32::from(b);
        bits += 8;
        while bits >= 5 {
            out.push(B32[((buf >> (bits - 5)) & 31) as usize] as char);
            bits -= 5;
        }
    }
    if bits > 0 {
        out.push(B32[((buf << (5 - bits)) & 31) as usize] as char);
    }
    out
}

pub fn base32_decode(s: &str) -> Option<Vec<u8>> {
    let (mut out, mut buf, mut bits) = (Vec::new(), 0u32, 0u32);
    for c in s.chars().filter(|c| !matches!(c, ' ' | '=' | '-')) {
        let v = B32.iter().position(|&b| b as char == c.to_ascii_uppercase())? as u32;
        buf = (buf << 5) | v;
        bits += 5;
        if bits >= 8 {
            out.push((buf >> (bits - 8)) as u8);
            bits -= 8;
        }
    }
    Some(out)
}

/// Kode TOTP 6 digit untuk langkah waktu `counter` (RFC 4226/6238, HMAC-SHA1).
pub fn totp_code(secret: &[u8], counter: u64) -> u32 {
    let mut mac = <Hmac<Sha1> as KeyInit>::new_from_slice(secret).expect("kunci hmac");
    mac.update(&counter.to_be_bytes());
    let h = mac.finalize().into_bytes();
    let o = (h[19] & 0x0f) as usize;
    let bin = (u32::from(h[o] & 0x7f) << 24) | (u32::from(h[o + 1]) << 16) | (u32::from(h[o + 2]) << 8) | u32::from(h[o + 3]);
    bin % 1_000_000
}

/// Langkah waktu yang cocok dengan `code` (toleransi ±1 langkah untuk selisih jam), atau None.
pub fn totp_match(secret: &[u8], code: &str, now_ms: i64) -> Option<u64> {
    let code = code.trim();
    if code.len() != 6 || !code.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let want: u32 = code.parse().ok()?;
    let step = (now_ms / 1000 / 30).max(0) as u64;
    (step.saturating_sub(1)..=step + 1).find(|s| ct_eq(&totp_code(secret, *s).to_be_bytes(), &want.to_be_bytes()))
}

fn new_recovery_codes() -> Res<Vec<String>> {
    const A: &[u8] = b"abcdefghjkmnpqrstuvwxyz23456789";
    (0..RECOVERY_COUNT)
        .map(|_| {
            let b: [u8; 10] = random_bytes()?;
            let s: String = b.iter().map(|x| A[*x as usize % A.len()] as char).collect();
            Ok(format!("{}-{}", &s[..5], &s[5..]))
        })
        .collect()
}

fn normalize_recovery(code: &str) -> String {
    code.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_lowercase()
}

// ---------------- Auth ----------------

struct Challenge {
    account_id: String,
    expires: i64,
    via: Via,
    device: String,
    ip: String,
}

struct Fail {
    n: u32,
    until: i64,
}

type Clock = Box<dyn Fn() -> i64 + Send + Sync>;

pub struct Auth {
    store: SharedStore,
    argon: Argon2<'static>,
    dummy_hash: String,
    policy: Mutex<AuthPolicy>,
    challenges: Mutex<HashMap<String, Challenge>>,
    failures: Mutex<HashMap<String, Fail>>,
    clock: Clock,
}

struct AccountRow {
    id: String,
    username: String,
    display_name: String,
    role_id: String,
    password_hash: String,
    totp_enc: Option<Vec<u8>>,
    pending_enc: Option<Vec<u8>>,
    totp_last_step: i64,
    recovery: Vec<String>,
    disabled: bool,
    created_at: i64,
    last_login_at: Option<i64>,
}

const ACCOUNT_COLS: &str = "id, username, display_name, role_id, password_hash, totp_enc, totp_pending_enc, totp_last_step, recovery, disabled, created_at, last_login_at";

fn account_from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<AccountRow> {
    let rec: String = r.get(8)?;
    Ok(AccountRow {
        id: r.get(0)?,
        username: r.get(1)?,
        display_name: r.get(2)?,
        role_id: r.get(3)?,
        password_hash: r.get(4)?,
        totp_enc: r.get(5)?,
        pending_enc: r.get(6)?,
        totp_last_step: r.get(7)?,
        recovery: rec.split(',').filter(|s| !s.is_empty()).map(String::from).collect(),
        disabled: r.get::<_, i64>(9)? != 0,
        created_at: r.get(10)?,
        last_login_at: r.get(11)?,
    })
}

fn load_role(st: &Store, id: &str) -> Res<Role> {
    st.conn
        .query_row(
            "SELECT id, name, description, permissions, builtin, (SELECT COUNT(*) FROM accounts a WHERE a.role_id = roles.id) FROM roles WHERE id = ?1",
            [id],
            |r| {
                Ok(Role { id: r.get(0)?, name: r.get(1)?, description: r.get(2)?, permissions: Permissions::parse(&r.get::<_, String>(3)?), builtin: r.get::<_, i64>(4)? != 0, account_count: r.get::<_, i64>(5)? as u32 })
            },
        )
        .optional()?
        .ok_or(AuthError::NotFound)
}

impl Auth {
    pub fn new(store: SharedStore, params: HashParams) -> Res<Self> {
        Self::with_clock(store, params, Box::new(|| std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)))
    }

    pub fn with_clock(store: SharedStore, params: HashParams, clock: Clock) -> Res<Self> {
        let p = Params::new(params.memory_kib, params.iterations, params.parallelism, None).map_err(|e| AuthError::Invalid(e.to_string()))?;
        let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, p);
        // Hash tiruan: verifikasi untuk username yang tidak ada memakan waktu yang sama.
        let dummy_hash = argon.hash_password(b"tidak-dipakai-siapa-pun").map_err(|e| AuthError::Db(e.to_string()))?.to_string();
        let a = Auth { store, argon, dummy_hash, policy: Mutex::new(AuthPolicy::default()), challenges: Mutex::new(HashMap::new()), failures: Mutex::new(HashMap::new()), clock };
        a.seed_roles()?;
        Ok(a)
    }

    pub fn set_policy(&self, p: AuthPolicy) {
        *self.policy.lock().expect("lock") = p;
    }

    pub fn policy(&self) -> AuthPolicy {
        *self.policy.lock().expect("lock")
    }

    fn now(&self) -> i64 {
        (self.clock)()
    }

    fn with<R>(&self, f: impl FnOnce(&mut Store) -> R) -> R {
        f(&mut self.store.lock().expect("store lock"))
    }

    fn seed_roles(&self) -> Res<()> {
        self.with(|st| {
            let n: i64 = st.conn.query_row("SELECT COUNT(*) FROM roles", [], |r| r.get(0))?;
            if n == 0 {
                for (id, name, desc, perms) in default_roles() {
                    st.conn.execute("INSERT INTO roles (id, name, description, permissions, builtin) VALUES (?1,?2,?3,?4,1)", params![id, name, desc, perms.encode()])?;
                }
            }
            Ok(())
        })
    }

    fn hash(&self, password: &str) -> Res<String> {
        Ok(self.argon.hash_password(password.as_bytes()).map_err(|e| AuthError::Db(e.to_string()))?.to_string())
    }

    fn verify(&self, hash: &str, password: &str) -> bool {
        PasswordHash::new(hash).is_ok_and(|h| self.argon.verify_password(password.as_bytes(), &h).is_ok())
    }

    fn audit(&self, actor: &str, action: &str, target: &str, title: &str, changes: Vec<Change>, ok: bool, error: Option<&str>) {
        let e = AuditEntry {
            at_ms: self.now(),
            actor: actor.into(),
            // Riwayat akun bukan milik router tertentu.
            router_id: "-".into(),
            action: action.into(),
            target: target.into(),
            title: title.into(),
            changes,
            outcome: if ok { AuditOutcome::Ok } else { AuditOutcome::Failed { steps_done: 0, error: error.unwrap_or_default().into() } },
        };
        let _ = self.with(|st| st.record_audit(&e));
    }

    // ---------------- pembatasan percobaan ----------------

    fn check_lock(&self, key: &str) -> Res<()> {
        let now = self.now();
        let f = self.failures.lock().expect("lock");
        if let Some(fail) = f.get(key).filter(|f| f.until > now) {
            return Err(AuthError::Locked { minutes: ((fail.until - now) as f64 / 60_000.0).ceil() as u32 });
        }
        Ok(())
    }

    fn fail(&self, key: &str) {
        let p = self.policy();
        let mut f = self.failures.lock().expect("lock");
        let e = f.entry(key.to_string()).or_insert(Fail { n: 0, until: 0 });
        e.n += 1;
        if e.n >= p.max_failures {
            e.until = self.now() + i64::from(p.lock_minutes) * 60_000;
            e.n = 0;
        }
    }

    fn clear_fail(&self, key: &str) {
        self.failures.lock().expect("lock").remove(key);
    }

    // ---------------- masuk ----------------

    fn role_perms(&self, role_id: &str) -> Res<Role> {
        self.with(|st| load_role(st, role_id))
    }

    /// 2FA wajib: saklar pemilik, akses publik aktif, atau peran dengan izin Ubah.
    fn must_2fa(&self, role: &Role) -> bool {
        let p = self.policy();
        p.require_2fa || p.public_on || role.permissions.has_update()
    }

    fn session_for(&self, a: &AccountRow, id: &str, via: Via) -> Res<Session> {
        let role = self.role_perms(&a.role_id)?;
        Ok(Session {
            id: id.into(),
            account_id: a.id.clone(),
            username: a.username.clone(),
            display_name: a.display_name.clone(),
            role_id: role.id,
            role_name: role.name,
            permissions: role.permissions,
            via,
            two_factor: a.totp_enc.is_some(),
        })
    }

    fn open_session(&self, a: &AccountRow, via: Via, device: &str, ip: &str) -> Res<(String, Session)> {
        let now = self.now();
        let token = random_hex(32)?;
        let id = format!("s-{}", random_hex(6)?);
        let exp = now + i64::from(self.policy().session_hours) * 3_600_000;
        self.with(|st| -> Res<()> {
            st.conn.execute(
                "INSERT INTO sessions (token_hash, id, account_id, via, device, ip, created_at, last_seen_at, expires_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?7,?8)",
                params![sha256_hex(token.as_bytes()), id, a.id, via.as_str(), device, ip, now, exp],
            )?;
            st.conn.execute("UPDATE accounts SET last_login_at = ?2 WHERE id = ?1", params![a.id, now])?;
            Ok(())
        })?;
        self.audit(&a.username, "auth.login", &a.username, "Masuk", vec![], true, None);
        Ok((token, self.session_for(a, &id, via)?))
    }

    fn account_by_username(&self, u: &str) -> Res<Option<AccountRow>> {
        self.with(|st| Ok(st.conn.query_row(&format!("SELECT {ACCOUNT_COLS} FROM accounts WHERE username = ?1"), [u], account_from_row).optional()?))
    }

    fn account_by_id(&self, id: &str) -> Res<AccountRow> {
        self.with(|st| st.conn.query_row(&format!("SELECT {ACCOUNT_COLS} FROM accounts WHERE id = ?1"), [id], account_from_row).optional())?.ok_or(AuthError::NotFound)
    }

    fn new_challenge(&self, a: &AccountRow, via: Via, device: &str, ip: &str) -> Res<String> {
        let id = random_hex(16)?;
        let now = self.now();
        let mut c = self.challenges.lock().expect("lock");
        c.retain(|_, v| v.expires > now);
        c.insert(id.clone(), Challenge { account_id: a.id.clone(), expires: now + CHALLENGE_TTL_MS, via, device: device.into(), ip: ip.into() });
        Ok(id)
    }

    fn peek_challenge(&self, id: &str) -> Res<(AccountRow, Via, String, String)> {
        let now = self.now();
        let (account_id, via, device, ip) = {
            let c = self.challenges.lock().expect("lock");
            let ch = c.get(id).filter(|c| c.expires > now).ok_or_else(|| AuthError::Invalid("Sesi masuk kedaluwarsa; masuk lagi".into()))?;
            (ch.account_id.clone(), ch.via, ch.device.clone(), ch.ip.clone())
        };
        Ok((self.account_by_id(&account_id)?, via, device, ip))
    }

    fn secret_of(&self, blob: &[u8], account_id: &str) -> Res<Vec<u8>> {
        self.with(|st| {
            let v = st.vault.as_ref().ok_or(AuthError::VaultLocked)?;
            v.open(format!("totp:{account_id}").as_bytes(), blob).map_err(|_| AuthError::Db("rahasia 2FA tidak bisa dibuka".into()))
        })
    }

    fn seal_secret(&self, secret: &[u8], account_id: &str) -> Res<Vec<u8>> {
        self.with(|st| {
            let v = st.vault.as_ref().ok_or(AuthError::VaultLocked)?;
            v.seal(format!("totp:{account_id}").as_bytes(), secret).map_err(|_| AuthError::Db("enkripsi 2FA gagal".into()))
        })
    }

    fn otpauth(&self, username: &str, secret: &[u8]) -> String {
        format!("otpauth://totp/Telemon:{username}?secret={}&issuer=Telemon", base32_encode(secret))
    }

    /// Mulai pendaftaran 2FA: rahasia baru disimpan terenkripsi sebagai "tertunda" sampai dikonfirmasi.
    fn begin_setup(&self, a: &AccountRow) -> Res<(String, String)> {
        let secret: [u8; 20] = random_bytes()?;
        let enc = self.seal_secret(&secret, &a.id)?;
        self.with(|st| st.conn.execute("UPDATE accounts SET totp_pending_enc = ?2 WHERE id = ?1", params![a.id, enc]))?;
        Ok((base32_encode(&secret), self.otpauth(&a.username, &secret)))
    }

    pub fn login(&self, username: &str, password: &str, via: Via, device: &str, ip: &str) -> Res<LoginOutcome> {
        let key = username.trim().to_lowercase();
        self.check_lock(&key)?;
        let acct = self.account_by_username(&key)?;
        // Verifikasi selalu dijalankan (hash tiruan bila akun tidak ada) agar waktu tidak membocorkan keberadaan akun.
        let ok = self.verify(acct.as_ref().map_or(&self.dummy_hash, |a| &a.password_hash), password);
        let Some(a) = acct.filter(|_| ok) else {
            self.fail(&key);
            self.audit(&key, "auth.login_failed", &key, "Masuk gagal", vec![], false, Some("kredensial salah"));
            return Err(AuthError::BadCredentials);
        };
        if a.disabled {
            return Err(AuthError::Disabled);
        }
        self.clear_fail(&key);
        if a.totp_enc.is_some() {
            return Ok(LoginOutcome::TwoFactorRequired { challenge: self.new_challenge(&a, via, device, ip)? });
        }
        let role = self.role_perms(&a.role_id)?;
        if self.must_2fa(&role) {
            let (secret, otpauth_url) = self.begin_setup(&a)?;
            return Ok(LoginOutcome::TwoFactorSetupRequired { challenge: self.new_challenge(&a, via, device, ip)?, secret, otpauth_url });
        }
        let (token, session) = self.open_session(&a, via, device, ip)?;
        Ok(LoginOutcome::Ok { token, session })
    }

    /// Langkah kedua login: kode authenticator atau kode pemulihan sekali pakai.
    pub fn verify_2fa(&self, challenge: &str, code: &str, recovery: bool) -> Res<Authenticated> {
        let (a, via, device, ip) = self.peek_challenge(challenge)?;
        self.check_lock(&a.username)?;
        let ok = if recovery {
            let h = sha256_hex(normalize_recovery(code).as_bytes());
            match a.recovery.iter().position(|r| ct_eq(r.as_bytes(), h.as_bytes())) {
                Some(i) => {
                    let mut left = a.recovery.clone();
                    left.remove(i);
                    self.with(|st| st.conn.execute("UPDATE accounts SET recovery = ?2 WHERE id = ?1", params![a.id, left.join(",")]))?;
                    true
                }
                None => false,
            }
        } else {
            let blob = a.totp_enc.as_ref().ok_or(AuthError::BadCode { recovery })?;
            let secret = self.secret_of(blob, &a.id)?;
            match totp_match(&secret, code, self.now()) {
                // Kode yang sama (atau lebih lama) tidak bisa dipakai dua kali.
                Some(step) if step as i64 > a.totp_last_step => {
                    self.with(|st| st.conn.execute("UPDATE accounts SET totp_last_step = ?2 WHERE id = ?1", params![a.id, step as i64]))?;
                    true
                }
                _ => false,
            }
        };
        if !ok {
            self.fail(&a.username);
            self.audit(&a.username, "auth.2fa_failed", &a.username, "Kode 2FA salah", vec![], false, None);
            return Err(AuthError::BadCode { recovery });
        }
        self.challenges.lock().expect("lock").remove(challenge);
        let (token, session) = self.open_session(&a, via, &device, &ip)?;
        Ok(Authenticated { token, session })
    }

    /// Menyelesaikan pendaftaran 2FA yang diwajibkan saat login. Mengembalikan kode pemulihan (sekali tampil).
    pub fn complete_2fa_setup(&self, challenge: &str, code: &str) -> Res<(Authenticated, Vec<String>)> {
        let (a, via, device, ip) = self.peek_challenge(challenge)?;
        self.check_lock(&a.username)?;
        let codes = self.confirm_setup_for(&a, code)?;
        self.challenges.lock().expect("lock").remove(challenge);
        let a = self.account_by_id(&a.id)?;
        let (token, session) = self.open_session(&a, via, &device, &ip)?;
        Ok((Authenticated { token, session }, codes))
    }

    fn confirm_setup_for(&self, a: &AccountRow, code: &str) -> Res<Vec<String>> {
        let blob = a.pending_enc.as_ref().ok_or_else(|| AuthError::Invalid("Mulai pendaftaran 2FA terlebih dulu".into()))?;
        let secret = self.secret_of(blob, &a.id)?;
        let Some(step) = totp_match(&secret, code, self.now()) else {
            self.fail(&a.username);
            return Err(AuthError::BadCode { recovery: false });
        };
        let codes = new_recovery_codes()?;
        let hashes: Vec<String> = codes.iter().map(|c| sha256_hex(normalize_recovery(c).as_bytes())).collect();
        self.with(|st| {
            st.conn.execute(
                "UPDATE accounts SET totp_enc = totp_pending_enc, totp_pending_enc = NULL, totp_last_step = ?2, recovery = ?3 WHERE id = ?1",
                params![a.id, step as i64, hashes.join(",")],
            )
        })?;
        self.audit(&a.username, "auth.2fa_enabled", &a.username, "2FA diaktifkan", vec![], true, None);
        Ok(codes)
    }

    // ---------------- sesi ----------------

    /// Periksa token sesi. Mengembalikan peran dan izin **terkini** (perubahan peran berlaku langsung).
    pub fn authenticate(&self, token: &str) -> Res<Session> {
        let now = self.now();
        let h = sha256_hex(token.as_bytes());
        let row: Option<(String, String, String, i64)> = self.with(|st| {
            st.conn.query_row("SELECT id, account_id, via, expires_at FROM sessions WHERE token_hash = ?1", [&h], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))).optional()
        })?;
        let Some((sid, account_id, via, expires)) = row else { return Err(AuthError::Unauthenticated) };
        if expires <= now {
            self.with(|st| st.conn.execute("DELETE FROM sessions WHERE token_hash = ?1", [&h]))?;
            return Err(AuthError::Unauthenticated);
        }
        let a = self.account_by_id(&account_id).map_err(|_| AuthError::Unauthenticated)?;
        if a.disabled {
            return Err(AuthError::Unauthenticated);
        }
        self.with(|st| st.conn.execute("UPDATE sessions SET last_seen_at = ?2 WHERE token_hash = ?1", params![h, now]))?;
        self.session_for(&a, &sid, Via::parse(&via))
    }

    pub fn logout(&self, token: &str) -> Res<()> {
        let h = sha256_hex(token.as_bytes());
        self.with(|st| st.conn.execute("DELETE FROM sessions WHERE token_hash = ?1", [h]))?;
        Ok(())
    }

    pub fn purge_expired_sessions(&self) -> Res<usize> {
        let now = self.now();
        Ok(self.with(|st| st.conn.execute("DELETE FROM sessions WHERE expires_at <= ?1", [now]))?)
    }

    pub fn list_my_sessions(&self, s: &Session) -> Res<Vec<SessionDevice>> {
        self.with(|st| {
            let mut stmt = st.conn.prepare("SELECT id, device, ip, via, created_at, last_seen_at FROM sessions WHERE account_id = ?1 ORDER BY last_seen_at DESC")?;
            let rows = stmt.query_map([&s.account_id], |r| {
                Ok(SessionDevice { id: r.get(0)?, device: r.get(1)?, ip: r.get(2)?, via: Via::parse(&r.get::<_, String>(3)?), created_at: r.get(4)?, last_seen_at: r.get(5)?, current: false })
            })?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?.into_iter().map(|d| SessionDevice { current: d.id == s.id, ..d }).collect())
        })
    }

    /// Keluarkan sesi perangkat lain milik akun sendiri.
    pub fn revoke_my_session(&self, s: &Session, id: &str) -> Res<()> {
        if id == s.id {
            return invalid("Gunakan Keluar untuk sesi ini");
        }
        let n = self.with(|st| st.conn.execute("DELETE FROM sessions WHERE id = ?1 AND account_id = ?2", params![id, s.account_id]))?;
        if n == 0 { Err(AuthError::NotFound) } else { Ok(()) }
    }

    // ---------------- akun sendiri ----------------

    fn check_password(&self, s: &Session, password: &str) -> Res<AccountRow> {
        let a = self.account_by_id(&s.account_id)?;
        self.check_lock(&a.username)?;
        if !self.verify(&a.password_hash, password) {
            self.fail(&a.username);
            return Err(AuthError::BadCredentials);
        }
        Ok(a)
    }

    pub fn update_my_profile(&self, s: &Session, display_name: &str) -> Res<Session> {
        let name = display_name.trim();
        if name.is_empty() || name.chars().count() > 60 {
            return invalid("Nama 1-60 karakter");
        }
        self.with(|st| st.conn.execute("UPDATE accounts SET display_name = ?2 WHERE id = ?1", params![s.account_id, name]))?;
        Ok(Session { display_name: name.into(), ..s.clone() })
    }

    /// Mengganti password mengeluarkan semua sesi lain akun ini.
    pub fn change_my_password(&self, s: &Session, current: &str, next: &str) -> Res<()> {
        let a = self.check_password(s, current)?;
        if next.chars().count() < MIN_PASSWORD {
            return invalid("Password baru minimal 8 karakter");
        }
        if next == current {
            return invalid("Password baru harus berbeda");
        }
        let h = self.hash(next)?;
        self.with(|st| -> Res<()> {
            st.conn.execute("UPDATE accounts SET password_hash = ?2 WHERE id = ?1", params![a.id, h])?;
            st.conn.execute("DELETE FROM sessions WHERE account_id = ?1 AND id != ?2", params![a.id, s.id])?;
            Ok(())
        })?;
        self.audit(&a.username, "account.password_change", &a.username, "Ganti password sendiri", vec![], true, None);
        Ok(())
    }

    pub fn start_2fa_setup(&self, s: &Session) -> Res<(String, String)> {
        let a = self.account_by_id(&s.account_id)?;
        if a.totp_enc.is_some() {
            return invalid("2FA sudah aktif");
        }
        self.begin_setup(&a)
    }

    pub fn confirm_2fa_setup(&self, s: &Session, code: &str) -> Res<Vec<String>> {
        let a = self.account_by_id(&s.account_id)?;
        self.check_lock(&a.username)?;
        self.confirm_setup_for(&a, code)
    }

    pub fn disable_2fa(&self, s: &Session, password: &str) -> Res<()> {
        let a = self.check_password(s, password)?;
        if self.must_2fa(&self.role_perms(&a.role_id)?) {
            return forbidden("2FA diwajibkan untuk akun ini");
        }
        self.with(|st| st.conn.execute("UPDATE accounts SET totp_enc = NULL, totp_pending_enc = NULL, recovery = '' WHERE id = ?1", [&a.id]))?;
        self.audit(&a.username, "auth.2fa_disabled", &a.username, "2FA dinonaktifkan", vec![], true, None);
        Ok(())
    }

    pub fn regenerate_recovery_codes(&self, s: &Session, password: &str) -> Res<Vec<String>> {
        let a = self.check_password(s, password)?;
        if a.totp_enc.is_none() {
            return invalid("Aktifkan 2FA terlebih dulu");
        }
        let codes = new_recovery_codes()?;
        let hashes: Vec<String> = codes.iter().map(|c| sha256_hex(normalize_recovery(c).as_bytes())).collect();
        self.with(|st| st.conn.execute("UPDATE accounts SET recovery = ?2 WHERE id = ?1", params![a.id, hashes.join(",")]))?;
        Ok(codes)
    }

    /// Jumlah kode pemulihan yang tersisa (untuk peringatan di halaman Akun saya).
    pub fn recovery_codes_left(&self, s: &Session) -> Res<usize> {
        Ok(self.account_by_id(&s.account_id)?.recovery.len())
    }

    // ---------------- otorisasi ----------------

    /// Periksa izin menu. `None` = desktop (boleh). Tulis lewat akses publik ditolak bila dimatikan pemilik.
    pub fn authorize(&self, caller: Option<&Session>, r: Resource, need: Level) -> Res<()> {
        let Some(s) = caller else { return Ok(()) };
        let have = s.permissions.get(r);
        if have == Level::None || (need == Level::Update && have != Level::Update) {
            return forbidden(if need == Level::Update { "Peran Anda tidak boleh mengubah data ini" } else { "Peran Anda tidak punya akses ke data ini" });
        }
        if need == Level::Update && s.via == Via::Publik && !self.policy().allow_public_write {
            return forbidden("Tulis lewat akses publik dimatikan oleh pemilik Telemon");
        }
        Ok(())
    }

    /// Izin tidak boleh melebihi milik penelepon.
    fn assert_grantable(&self, caller: Option<&Session>, perms: &Permissions) -> Res<()> {
        let Some(me) = caller else { return Ok(()) };
        for r in Resource::ALL {
            if perms.get(r) > me.permissions.get(r) {
                return forbidden("Anda tidak bisa memberi izin melebihi peran Anda");
            }
        }
        Ok(())
    }

    fn assert_manageable(&self, caller: Option<&Session>, target: &AccountRow) -> Res<()> {
        let Some(me) = caller else { return Ok(()) };
        if me.account_id == target.id {
            return forbidden("Tidak bisa mengubah akun Anda sendiri dari sini; pakai halaman Akun saya");
        }
        self.assert_grantable(caller, &self.role_perms(&target.role_id)?.permissions)
    }

    fn actor(caller: Option<&Session>) -> String {
        caller.map_or("desktop".into(), |s| s.username.clone())
    }

    // ---------------- admin: akun ----------------

    fn summary(&self, a: &AccountRow) -> Res<AccountSummary> {
        let role = self.role_perms(&a.role_id)?;
        Ok(AccountSummary { id: a.id.clone(), username: a.username.clone(), display_name: a.display_name.clone(), role_id: role.id, role_name: role.name, two_factor: a.totp_enc.is_some(), disabled: a.disabled, created_at: a.created_at, last_login_at: a.last_login_at })
    }

    pub fn list_accounts(&self, caller: Option<&Session>) -> Res<Vec<AccountSummary>> {
        self.authorize(caller, Resource::Admin, Level::Read)?;
        let rows: Vec<AccountRow> = self.with(|st| -> Res<Vec<AccountRow>> {
            let mut stmt = st.conn.prepare(&format!("SELECT {ACCOUNT_COLS} FROM accounts ORDER BY username"))?;
            Ok(stmt.query_map([], account_from_row)?.collect::<Result<_, _>>()?)
        })?;
        rows.iter().map(|a| self.summary(a)).collect()
    }

    pub fn create_account(&self, caller: Option<&Session>, n: &NewAccount) -> Res<AccountSummary> {
        self.authorize(caller, Resource::Admin, Level::Update)?;
        let role = self.role_perms(&n.role_id).map_err(|_| AuthError::Invalid("Pilih peran untuk akun ini".into()))?;
        self.assert_grantable(caller, &role.permissions)?;
        let username = n.username.trim().to_lowercase();
        if !(3..=30).contains(&username.len()) || !username.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-')) {
            return invalid("Username 3-30 karakter: huruf kecil, angka, titik, garis bawah, atau strip");
        }
        if n.display_name.trim().is_empty() || n.display_name.trim().chars().count() > 60 {
            return invalid("Nama 1-60 karakter");
        }
        if n.password.chars().count() < MIN_PASSWORD {
            return invalid("Password minimal 8 karakter");
        }
        if self.account_by_username(&username)?.is_some() {
            return invalid("Username sudah dipakai");
        }
        let id = format!("a-{}", random_hex(6)?);
        let h = self.hash(&n.password)?;
        self.with(|st| st.conn.execute(
            "INSERT INTO accounts (id, username, display_name, role_id, password_hash, created_at) VALUES (?1,?2,?3,?4,?5,?6)",
            params![id, username, n.display_name.trim(), role.id, h, self.now()],
        ))?;
        self.audit(&Self::actor(caller), "account.create", &username, &format!("Tambah akun {username}"), vec![Change { field: "peran".into(), from: None, to: Some(role.name) }], true, None);
        self.summary(&self.account_by_id(&id)?)
    }

    pub fn reset_account_password(&self, caller: Option<&Session>, id: &str, password: &str) -> Res<()> {
        self.authorize(caller, Resource::Admin, Level::Update)?;
        if password.chars().count() < MIN_PASSWORD {
            return invalid("Password minimal 8 karakter");
        }
        let a = self.account_by_id(id)?;
        self.assert_manageable(caller, &a)?;
        let h = self.hash(password)?;
        self.with(|st| -> Res<()> {
            st.conn.execute("UPDATE accounts SET password_hash = ?2 WHERE id = ?1", params![id, h])?;
            st.conn.execute("DELETE FROM sessions WHERE account_id = ?1", [id])?;
            Ok(())
        })?;
        self.audit(&Self::actor(caller), "account.reset_password", &a.username, &format!("Reset password {}", a.username), vec![], true, None);
        Ok(())
    }

    pub fn set_account_disabled(&self, caller: Option<&Session>, id: &str, disabled: bool) -> Res<AccountSummary> {
        self.authorize(caller, Resource::Admin, Level::Update)?;
        let a = self.account_by_id(id)?;
        self.assert_manageable(caller, &a)?;
        self.with(|st| -> Res<()> {
            st.conn.execute("UPDATE accounts SET disabled = ?2 WHERE id = ?1", params![id, disabled])?;
            if disabled {
                st.conn.execute("DELETE FROM sessions WHERE account_id = ?1", [id])?;
            }
            Ok(())
        })?;
        let act = if disabled { "account.disable" } else { "account.enable" };
        self.audit(&Self::actor(caller), act, &a.username, &format!("{} akun {}", if disabled { "Nonaktifkan" } else { "Aktifkan" }, a.username), vec![], true, None);
        self.summary(&self.account_by_id(id)?)
    }

    pub fn reset_account_2fa(&self, caller: Option<&Session>, id: &str) -> Res<AccountSummary> {
        self.authorize(caller, Resource::Admin, Level::Update)?;
        let a = self.account_by_id(id)?;
        self.assert_manageable(caller, &a)?;
        self.with(|st| -> Res<()> {
            st.conn.execute("UPDATE accounts SET totp_enc = NULL, totp_pending_enc = NULL, totp_last_step = 0, recovery = '' WHERE id = ?1", [id])?;
            st.conn.execute("DELETE FROM sessions WHERE account_id = ?1", [id])?;
            Ok(())
        })?;
        self.audit(&Self::actor(caller), "account.reset_2fa", &a.username, &format!("Reset 2FA {}", a.username), vec![], true, None);
        self.summary(&self.account_by_id(id)?)
    }

    pub fn change_account_role(&self, caller: Option<&Session>, id: &str, role_id: &str) -> Res<AccountSummary> {
        self.authorize(caller, Resource::Admin, Level::Update)?;
        let a = self.account_by_id(id)?;
        self.assert_manageable(caller, &a)?;
        let new = self.role_perms(role_id).map_err(|_| AuthError::Invalid("Pilih peran untuk akun ini".into()))?;
        self.assert_grantable(caller, &new.permissions)?;
        let old = self.role_perms(&a.role_id)?;
        self.with(|st| st.conn.execute("UPDATE accounts SET role_id = ?2 WHERE id = ?1", params![id, role_id]))?;
        self.audit(&Self::actor(caller), "account.change_role", &a.username, &format!("Ubah peran {}", a.username), vec![Change { field: "peran".into(), from: Some(old.name), to: Some(new.name) }], true, None);
        self.summary(&self.account_by_id(id)?)
    }

    pub fn delete_account(&self, caller: Option<&Session>, id: &str) -> Res<()> {
        self.authorize(caller, Resource::Admin, Level::Update)?;
        let a = self.account_by_id(id)?;
        self.assert_manageable(caller, &a)?;
        let others: i64 = self.with(|st| st.conn.query_row("SELECT COUNT(*) FROM accounts WHERE id != ?1 AND disabled = 0", [id], |r| r.get(0)))?;
        if others == 0 && self.policy().public_on {
            return forbidden("Akses publik sedang aktif; matikan dulu sebelum menghapus akun terakhir");
        }
        self.with(|st| -> Res<()> {
            st.conn.execute("DELETE FROM sessions WHERE account_id = ?1", [id])?;
            st.conn.execute("DELETE FROM accounts WHERE id = ?1", [id])?;
            Ok(())
        })?;
        self.audit(&Self::actor(caller), "account.delete", &a.username, &format!("Hapus akun {}", a.username), vec![], true, None);
        Ok(())
    }

    // ---------------- admin: peran ----------------

    pub fn list_roles(&self, caller: Option<&Session>) -> Res<Vec<Role>> {
        self.authorize(caller, Resource::Admin, Level::Read)?;
        self.with(|st| -> Res<Vec<Role>> {
            let ids: Vec<String> = {
                let mut stmt = st.conn.prepare("SELECT id FROM roles ORDER BY builtin DESC, name COLLATE NOCASE")?;
                stmt.query_map([], |r| r.get(0))?.collect::<Result<_, _>>()?
            };
            ids.iter().map(|id| load_role(st, id)).collect()
        })
    }

    fn check_role_input(&self, input: &RoleInput) -> Res<()> {
        let n = input.name.trim();
        if n.is_empty() || n.chars().count() > 40 {
            return invalid("Nama peran 1-40 karakter");
        }
        for r in Resource::ALL {
            if input.permissions.get(r) > r.max_level() {
                return invalid("Menu ini hanya punya Tanpa akses atau Lihat");
            }
        }
        Ok(())
    }

    pub fn create_role(&self, caller: Option<&Session>, input: &RoleInput) -> Res<Role> {
        self.authorize(caller, Resource::Admin, Level::Update)?;
        self.check_role_input(input)?;
        self.assert_grantable(caller, &input.permissions)?;
        let id = format!("role-{}", random_hex(5)?);
        match self.with(|st| st.conn.execute("INSERT INTO roles (id, name, description, permissions, builtin) VALUES (?1,?2,?3,?4,0)", params![id, input.name.trim(), input.description.trim(), input.permissions.encode()])) {
            Ok(_) => {}
            Err(rusqlite::Error::SqliteFailure(e, _)) if e.code == rusqlite::ErrorCode::ConstraintViolation => return invalid("Nama peran sudah dipakai"),
            Err(e) => return Err(e.into()),
        }
        self.audit(&Self::actor(caller), "role.create", input.name.trim(), &format!("Tambah peran {}", input.name.trim()), vec![], true, None);
        self.role_perms(&id)
    }

    pub fn update_role(&self, caller: Option<&Session>, id: &str, input: &RoleInput) -> Res<Role> {
        self.authorize(caller, Resource::Admin, Level::Update)?;
        let old = self.role_perms(id)?;
        if let Some(me) = caller
            && me.role_id == id
        {
            return forbidden("Tidak bisa mengubah peran yang sedang Anda pakai");
        }
        self.check_role_input(input)?;
        self.assert_grantable(caller, &input.permissions)?;
        // Peran bawaan: namanya tetap, izin dan keterangan boleh diubah.
        let name = if old.builtin { old.name.clone() } else { input.name.trim().to_string() };
        match self.with(|st| st.conn.execute("UPDATE roles SET name = ?2, description = ?3, permissions = ?4 WHERE id = ?1", params![id, name, input.description.trim(), input.permissions.encode()])) {
            Ok(_) => {}
            Err(rusqlite::Error::SqliteFailure(e, _)) if e.code == rusqlite::ErrorCode::ConstraintViolation => return invalid("Nama peran sudah dipakai"),
            Err(e) => return Err(e.into()),
        }
        // Izin berubah: peran dengan izin Ubah mewajibkan 2FA, dan sesi aktif memakai izin terbaru (authenticate membaca ulang).
        let changes: Vec<Change> = Resource::ALL
            .iter()
            .filter(|r| old.permissions.get(**r) != input.permissions.get(**r))
            .map(|r| Change { field: r.as_str().into(), from: Some(old.permissions.get(*r).as_str().into()), to: Some(input.permissions.get(*r).as_str().into()) })
            .collect();
        self.audit(&Self::actor(caller), "role.update", &name, &format!("Ubah peran {name}"), changes, true, None);
        self.role_perms(id)
    }

    pub fn delete_role(&self, caller: Option<&Session>, id: &str) -> Res<()> {
        self.authorize(caller, Resource::Admin, Level::Update)?;
        let r = self.role_perms(id)?;
        if r.builtin {
            return forbidden("Peran bawaan tidak bisa dihapus");
        }
        if r.account_count > 0 {
            return forbidden("Peran masih dipakai akun");
        }
        self.with(|st| st.conn.execute("DELETE FROM roles WHERE id = ?1", [id]))?;
        self.audit(&Self::actor(caller), "role.delete", &r.name, &format!("Hapus peran {}", r.name), vec![], true, None);
        Ok(())
    }

    /// Kembalikan izin peran bawaan ke nilai awal.
    pub fn reset_builtin_role(&self, caller: Option<&Session>, id: &str) -> Res<Role> {
        let (_, name, desc, perms) = default_roles().into_iter().find(|(i, ..)| *i == id).ok_or(AuthError::NotFound)?;
        self.update_role(caller, id, &RoleInput { name: name.into(), description: desc.into(), permissions: perms })
    }
}

// Dipakai bersama oleh lapisan HTTP.
pub type SharedAuth = Arc<Auth>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::AuditQuery;
    use std::sync::atomic::{AtomicI64, Ordering};

    const T0: i64 = 1_000_000_000_000;

    struct H {
        auth: Auth,
        store: SharedStore,
        clock: Arc<AtomicI64>,
    }

    fn setup() -> H {
        let mut st = Store::open_in_memory().unwrap();
        st.unlock(&crate::vault::generate_key().unwrap()).unwrap();
        let store: SharedStore = Arc::new(Mutex::new(st));
        let clock = Arc::new(AtomicI64::new(T0));
        let c2 = clock.clone();
        let auth = Auth::with_clock(store.clone(), HashParams { memory_kib: 8, iterations: 1, parallelism: 1 }, Box::new(move || c2.load(Ordering::SeqCst))).unwrap();
        H { auth, store, clock }
    }

    impl H {
        fn advance(&self, ms: i64) {
            self.clock.fetch_add(ms, Ordering::SeqCst);
        }
        fn now(&self) -> i64 {
            self.clock.load(Ordering::SeqCst)
        }
        fn account(&self, user: &str, role: &str) -> AccountSummary {
            self.auth.create_account(None, &NewAccount { username: user.into(), display_name: user.to_uppercase(), password: "password-uji".into(), role_id: role.into() }).unwrap()
        }
        fn code(&self, secret_b32: &str) -> String {
            format!("{:06}", totp_code(&base32_decode(secret_b32).unwrap(), (self.now() / 30_000) as u64))
        }
        /// Masuk penuh (termasuk pendaftaran 2FA bila diwajibkan); mengembalikan token, sesi, dan rahasia 2FA bila ada.
        fn login(&self, user: &str) -> (String, Session, Option<String>) {
            match self.auth.login(user, "password-uji", Via::Lan, "uji", "10.0.0.2").unwrap() {
                LoginOutcome::Ok { token, session } => (token, session, None),
                LoginOutcome::TwoFactorSetupRequired { challenge, secret, .. } => {
                    let code = self.code(&secret);
                    let (a, _) = self.auth.complete_2fa_setup(&challenge, &code).unwrap();
                    (a.token, a.session, Some(secret))
                }
                LoginOutcome::TwoFactorRequired { .. } => panic!("butuh kode; pakai login_2fa"),
            }
        }
    }

    fn err<T: std::fmt::Debug>(r: Res<T>) -> AuthError {
        r.expect_err("harusnya galat")
    }

    #[test]
    fn totp_vektor_rfc6238_dan_base32() {
        let key = b"12345678901234567890";
        // RFC 6238 Lampiran B (6 digit terakhir dari kode 8 digit).
        for (t, want) in [(59u64, 287082u32), (1_111_111_109, 81804), (1_111_111_111, 50471), (1_234_567_890, 5924), (2_000_000_000, 279037)] {
            assert_eq!(totp_code(key, t / 30), want, "t={t}");
        }
        assert_eq!(base32_encode(b"foobar"), "MZXW6YTBOI");
        assert_eq!(base32_decode("mzxw 6ytb oi").unwrap(), b"foobar");
        assert_eq!(base32_decode("1"), None);
        let s = [7u8; 20];
        assert_eq!(base32_decode(&base32_encode(&s)).unwrap(), s);
        // Toleransi ±1 langkah, di luar itu ditolak.
        let now = 59_000;
        assert_eq!(totp_match(key, "287082", now), Some(1));
        assert!(totp_match(key, "287082", now + 30_000).is_some());
        assert!(totp_match(key, "287082", now + 90_000).is_none());
        assert!(totp_match(key, "28708", now).is_none() && totp_match(key, "abcdef", now).is_none());
    }

    #[test]
    fn izin_encode_parse_dan_batas() {
        let p = Permissions::default().with(Resource::User, Level::Update).with(Resource::Event, Level::Read);
        assert_eq!(Permissions::parse(&p.encode()), p);
        // Menu tanpa fitur ubah dibatasi Lihat; kolom asing dan rusak diabaikan.
        let q = Permissions::parse("ringkasan=update,user=read,xx=read,event,router=salah");
        assert_eq!((q.get(Resource::Ringkasan), q.get(Resource::User), q.get(Resource::Router)), (Level::Read, Level::Read, Level::None));
        assert!(Level::Update > Level::Read && Level::Read > Level::None);
    }

    #[test]
    fn peran_bawaan_dan_validasi_akun() {
        let h = setup();
        let roles = h.auth.list_roles(None).unwrap();
        assert_eq!(roles.iter().map(|r| r.name.as_str()).collect::<Vec<_>>().len(), 4);
        let admin = roles.iter().find(|r| r.id == "role-admin").unwrap();
        assert!(admin.builtin && admin.permissions.get(Resource::Admin) == Level::Update);
        let monitor = roles.iter().find(|r| r.id == "role-monitor").unwrap();
        assert_eq!((monitor.permissions.get(Resource::User), monitor.permissions.get(Resource::Router)), (Level::None, Level::Read));

        let na = |u: &str, d: &str, p: &str, r: &str| NewAccount { username: u.into(), display_name: d.into(), password: p.into(), role_id: r.into() };
        for (bad, why) in [
            (na("ab", "A", "password-uji", "role-viewer"), "username"),
            (na("Ada Lovelace", "A", "password-uji", "role-viewer"), "username"),
            (na("budi", " ", "password-uji", "role-viewer"), "Nama"),
            (na("budi", "B", "pendek", "role-viewer"), "Password"),
            (na("budi", "B", "password-uji", "tidak-ada"), "peran"),
        ] {
            let e = err(h.auth.create_account(None, &bad));
            assert!(e.to_string().to_lowercase().contains(&why.to_lowercase()), "{why}: {e}");
        }
        h.account("budi", "role-viewer");
        assert!(err(h.auth.create_account(None, &na("BUDI", "B", "password-uji", "role-viewer"))).to_string().contains("sudah dipakai"));
    }

    #[test]
    fn masuk_tanpa_2fa_untuk_peran_baca_saja_dan_hash_argon2id() {
        let h = setup();
        h.account("pantau", "role-monitor");
        let hash: String = h.store.lock().unwrap().conn.query_row("SELECT password_hash FROM accounts WHERE username='pantau'", [], |r| r.get(0)).unwrap();
        assert!(hash.starts_with("$argon2id$"), "{hash}");
        assert!(!hash.contains("password-uji"));
        let (token, s, secret) = h.login("pantau");
        assert!(secret.is_none() && !s.two_factor);
        assert_eq!(s.role_name, "Monitor");
        assert_eq!(h.auth.authenticate(&token).unwrap().account_id, s.account_id);
        // Token tidak tersimpan polos.
        let stored: String = h.store.lock().unwrap().conn.query_row("SELECT token_hash FROM sessions", [], |r| r.get(0)).unwrap();
        assert_ne!(stored, token);
        h.auth.logout(&token).unwrap();
        assert_eq!(err(h.auth.authenticate(&token)), AuthError::Unauthenticated);
    }

    #[test]
    fn peran_ubah_wajib_2fa_dan_alur_penuh() {
        let h = setup();
        h.account("operator", "role-operator");
        // Pertama kali: wajib mendaftar 2FA.
        let LoginOutcome::TwoFactorSetupRequired { challenge, secret, otpauth_url } = h.auth.login("operator", "password-uji", Via::Lan, "uji", "ip").unwrap() else { panic!("harusnya setup") };
        assert!(otpauth_url.starts_with("otpauth://totp/Telemon:operator?secret=") && otpauth_url.contains(&secret));
        // Rahasia terenkripsi di basis data.
        let blob: Vec<u8> = h.store.lock().unwrap().conn.query_row("SELECT totp_pending_enc FROM accounts WHERE username='operator'", [], |r| r.get(0)).unwrap();
        assert!(!blob.windows(8).any(|w| w == secret.as_bytes()));
        assert_eq!(err(h.auth.complete_2fa_setup(&challenge, "000000")), AuthError::BadCode { recovery: false });
        let (a, codes) = h.auth.complete_2fa_setup(&challenge, &h.code(&secret)).unwrap();
        assert!(a.session.two_factor);
        assert_eq!(codes.len(), 8);
        assert!(codes.iter().all(|c| c.len() == 11 && c.as_bytes()[5] == b'-'));
        assert_eq!(h.auth.recovery_codes_left(&a.session).unwrap(), 8);
        // Kode pemulihan tersimpan sebagai hash.
        let rec: String = h.store.lock().unwrap().conn.query_row("SELECT recovery FROM accounts WHERE username='operator'", [], |r| r.get(0)).unwrap();
        assert!(!codes.iter().any(|c| rec.contains(c.as_str())));

        // Login berikutnya: minta kode. Kode di langkah waktu yang sama sudah terpakai (anti-ulang).
        let LoginOutcome::TwoFactorRequired { challenge } = h.auth.login("operator", "password-uji", Via::Lan, "uji", "ip").unwrap() else { panic!("harusnya 2fa") };
        assert_eq!(err(h.auth.verify_2fa(&challenge, &h.code(&secret), false)), AuthError::BadCode { recovery: false }, "kode yang baru dipakai tidak boleh dipakai ulang");
        h.advance(30_000);
        let ok = h.auth.verify_2fa(&challenge, &h.code(&secret), false).unwrap();
        assert_eq!(h.auth.authenticate(&ok.token).unwrap().username, "operator");
        // Tantangan sekali pakai.
        assert!(matches!(err(h.auth.verify_2fa(&challenge, &h.code(&secret), false)), AuthError::Invalid(_)));

        // Kode pemulihan: sekali pakai, huruf besar dan spasi ditoleransi.
        let LoginOutcome::TwoFactorRequired { challenge } = h.auth.login("operator", "password-uji", Via::Lan, "uji", "ip").unwrap() else { panic!() };
        let rc = codes[0].to_uppercase();
        assert!(h.auth.verify_2fa(&challenge, &format!(" {rc} "), true).is_ok());
        assert_eq!(h.auth.recovery_codes_left(&ok.session).unwrap(), 7);
        let LoginOutcome::TwoFactorRequired { challenge } = h.auth.login("operator", "password-uji", Via::Lan, "uji", "ip").unwrap() else { panic!() };
        assert_eq!(err(h.auth.verify_2fa(&challenge, &codes[0], true)), AuthError::BadCode { recovery: true });
        // Kedaluwarsa tantangan.
        h.advance(CHALLENGE_TTL_MS + 1);
        assert!(matches!(err(h.auth.verify_2fa(&challenge, &codes[1], true)), AuthError::Invalid(_)));
    }

    #[test]
    fn saklar_2fa_dan_publik_mewajibkan_semua() {
        let h = setup();
        h.account("pantau", "role-monitor");
        h.auth.set_policy(AuthPolicy { require_2fa: true, ..Default::default() });
        assert!(matches!(h.auth.login("pantau", "password-uji", Via::Lan, "", "").unwrap(), LoginOutcome::TwoFactorSetupRequired { .. }));
        h.auth.set_policy(AuthPolicy { public_on: true, ..Default::default() });
        assert!(matches!(h.auth.login("pantau", "password-uji", Via::Publik, "", "").unwrap(), LoginOutcome::TwoFactorSetupRequired { .. }));
        h.auth.set_policy(AuthPolicy::default());
        assert!(matches!(h.auth.login("pantau", "password-uji", Via::Lan, "", "").unwrap(), LoginOutcome::Ok { .. }));
    }

    #[test]
    fn penguncian_percobaan_salah() {
        let h = setup();
        h.account("budi", "role-viewer");
        for _ in 0..4 {
            assert_eq!(err(h.auth.login("budi", "salah-salah", Via::Lan, "", "")), AuthError::BadCredentials);
        }
        assert_eq!(err(h.auth.login("budi", "salah-salah", Via::Lan, "", "")), AuthError::BadCredentials);
        // Terkunci, bahkan dengan password benar.
        assert_eq!(err(h.auth.login("budi", "password-uji", Via::Lan, "", "")), AuthError::Locked { minutes: 5 });
        h.advance(4 * 60_000);
        assert_eq!(err(h.auth.login("budi", "password-uji", Via::Lan, "", "")), AuthError::Locked { minutes: 1 });
        h.advance(60_001);
        assert!(h.auth.login("budi", "password-uji", Via::Lan, "", "").is_ok());
        // Username tidak ada: tampak sama (tidak membocorkan keberadaan akun) dan ikut terkunci.
        for _ in 0..5 {
            assert_eq!(err(h.auth.login("hantu", "x", Via::Lan, "", "")), AuthError::BadCredentials);
        }
        assert!(matches!(err(h.auth.login("hantu", "x", Via::Lan, "", "")), AuthError::Locked { .. }));
        // Berhasil membersihkan hitungan.
        for _ in 0..3 {
            let _ = h.auth.login("budi", "salah-salah", Via::Lan, "", "");
        }
        assert!(h.auth.login("budi", "password-uji", Via::Lan, "", "").is_ok());
        for _ in 0..3 {
            let _ = h.auth.login("budi", "salah-salah", Via::Lan, "", "");
        }
        assert_eq!(err(h.auth.login("budi", "salah-salah", Via::Lan, "", "")), AuthError::BadCredentials, "hitungan dimulai ulang setelah berhasil");
    }

    #[test]
    fn akun_nonaktif_dan_sesi_kedaluwarsa() {
        let h = setup();
        let a = h.account("budi", "role-viewer");
        let (token, ..) = h.login("budi");
        h.auth.set_account_disabled(None, &a.id, true).unwrap();
        assert_eq!(err(h.auth.authenticate(&token)), AuthError::Unauthenticated);
        assert_eq!(err(h.auth.login("budi", "password-uji", Via::Lan, "", "")), AuthError::Disabled);
        h.auth.set_account_disabled(None, &a.id, false).unwrap();
        let (token, ..) = h.login("budi");
        h.advance(12 * 3_600_000 - 1);
        assert!(h.auth.authenticate(&token).is_ok());
        h.advance(2);
        assert_eq!(err(h.auth.authenticate(&token)), AuthError::Unauthenticated);
        assert_eq!(err(h.auth.authenticate("token-ngawur")), AuthError::Unauthenticated);
        h.login("budi");
        h.advance(13 * 3_600_000);
        assert_eq!(h.auth.purge_expired_sessions().unwrap(), 1);
    }

    #[test]
    fn perubahan_peran_berlaku_langsung_pada_sesi_aktif() {
        let h = setup();
        let a = h.account("budi", "role-viewer");
        let (token, s, _) = h.login("budi");
        assert_eq!(s.permissions.get(Resource::User), Level::Read);
        h.auth.change_account_role(None, &a.id, "role-monitor").unwrap();
        assert_eq!(h.auth.authenticate(&token).unwrap().permissions.get(Resource::User), Level::None);
    }

    #[test]
    fn akun_sendiri_ganti_password_profil_dan_2fa() {
        let h = setup();
        h.account("budi", "role-viewer");
        let (t1, s1, _) = h.login("budi");
        let (t2, ..) = h.login("budi");
        assert_eq!(h.auth.list_my_sessions(&s1).unwrap().len(), 2);
        assert!(h.auth.list_my_sessions(&s1).unwrap().iter().any(|d| d.current && d.id == s1.id));
        assert_eq!(err(h.auth.change_my_password(&s1, "salah", "password-baru")), AuthError::BadCredentials);
        assert!(err(h.auth.change_my_password(&s1, "password-uji", "pendek")).to_string().contains("minimal 8"));
        assert!(err(h.auth.change_my_password(&s1, "password-uji", "password-uji")).to_string().contains("berbeda"));
        h.auth.change_my_password(&s1, "password-uji", "password-baru").unwrap();
        // Sesi lain keluar, sesi ini tetap.
        assert!(h.auth.authenticate(&t1).is_ok());
        assert_eq!(err(h.auth.authenticate(&t2)), AuthError::Unauthenticated);
        assert!(matches!(h.auth.login("budi", "password-uji", Via::Lan, "", "").unwrap_err(), AuthError::BadCredentials));
        assert!(h.auth.login("budi", "password-baru", Via::Lan, "", "").is_ok());

        assert_eq!(h.auth.update_my_profile(&s1, "  Budi S ").unwrap().display_name, "Budi S");
        assert!(h.auth.update_my_profile(&s1, " ").is_err());

        // Aktifkan 2FA opsional, lalu matikan (boleh karena tidak diwajibkan).
        let (secret, url) = h.auth.start_2fa_setup(&s1).unwrap();
        assert!(url.contains(&secret));
        assert!(err(h.auth.confirm_2fa_setup(&s1, "123456")).to_string().contains("Kode salah"));
        let codes = h.auth.confirm_2fa_setup(&s1, &h.code(&secret)).unwrap();
        assert_eq!(codes.len(), 8);
        assert!(err(h.auth.start_2fa_setup(&s1)).to_string().contains("sudah aktif"));
        assert_eq!(h.auth.regenerate_recovery_codes(&s1, "password-baru").unwrap().len(), 8);
        assert_eq!(err(h.auth.regenerate_recovery_codes(&s1, "salah")), AuthError::BadCredentials);
        h.auth.disable_2fa(&s1, "password-baru").unwrap();
        assert!(err(h.auth.regenerate_recovery_codes(&s1, "password-baru")).to_string().contains("Aktifkan 2FA"));
        // Saat diwajibkan, tidak bisa dimatikan.
        h.auth.set_policy(AuthPolicy { require_2fa: true, ..Default::default() });
        assert!(matches!(err(h.auth.disable_2fa(&s1, "password-baru")), AuthError::Forbidden(_)));
        // Keluarkan sesi lain milik sendiri.
        h.auth.set_policy(AuthPolicy::default());
        let LoginOutcome::Ok { session: s3, .. } = h.auth.login("budi", "password-baru", Via::Lan, "uji", "ip").unwrap() else { panic!() };
        assert!(h.auth.revoke_my_session(&s1, &s3.id).is_ok());
        assert!(matches!(err(h.auth.revoke_my_session(&s1, &s1.id)), AuthError::Invalid(_)));
        assert_eq!(err(h.auth.revoke_my_session(&s1, "tidak-ada")), AuthError::NotFound);
    }

    #[test]
    fn otorisasi_menu_dan_tulis_lewat_publik() {
        let h = setup();
        h.account("operator", "role-operator");
        h.account("viewer", "role-viewer");
        let (_, op, _) = h.login("operator");
        let (_, vw, _) = h.login("viewer");
        assert!(h.auth.authorize(None, Resource::Admin, Level::Update).is_ok(), "desktop boleh semuanya");
        assert!(h.auth.authorize(Some(&op), Resource::User, Level::Update).is_ok());
        assert!(h.auth.authorize(Some(&op), Resource::Router, Level::Read).is_ok());
        assert!(matches!(h.auth.authorize(Some(&op), Resource::Router, Level::Update), Err(AuthError::Forbidden(_))));
        assert!(matches!(h.auth.authorize(Some(&op), Resource::Admin, Level::Read), Err(AuthError::Forbidden(_))));
        assert!(matches!(h.auth.authorize(Some(&vw), Resource::User, Level::Update), Err(AuthError::Forbidden(_))));
        assert!(h.auth.authorize(Some(&vw), Resource::User, Level::Read).is_ok());
        // Lewat akses publik: tulis ditolak kecuali dibolehkan pemilik; baca tetap boleh.
        let via_pub = Session { via: Via::Publik, ..op.clone() };
        assert!(h.auth.authorize(Some(&via_pub), Resource::User, Level::Read).is_ok());
        assert!(err(h.auth.authorize(Some(&via_pub), Resource::User, Level::Update)).to_string().contains("publik"));
        h.auth.set_policy(AuthPolicy { allow_public_write: true, ..Default::default() });
        assert!(h.auth.authorize(Some(&via_pub), Resource::User, Level::Update).is_ok());
    }

    /// Admin kustom yang hanya punya sebagian izin, untuk menguji pengaman anti-kenaikan izin.
    fn custom_admin(h: &H, name: &str, perms: Permissions) -> (Session, String) {
        let role = h.auth.create_role(None, &RoleInput { name: name.into(), description: String::new(), permissions: perms.with(Resource::Admin, Level::Update) }).unwrap();
        h.account(name, &role.id);
        let (_, s, _) = h.login(name);
        (s, role.id)
    }

    #[test]
    fn pengaman_anti_kenaikan_izin() {
        let h = setup();
        // Admin terbatas: boleh Admin + Users(Ubah) + Ringkasan(Lihat), tidak punya Router/Pengaturan.
        let limited = Permissions::default().with(Resource::User, Level::Update).with(Resource::Ringkasan, Level::Read);
        let (me, my_role) = custom_admin(&h, "kecil", limited);
        let na = |u: &str, role: &str| NewAccount { username: u.into(), display_name: u.into(), password: "password-uji".into(), role_id: role.into() };

        // Tidak bisa memberi peran yang melebihi miliknya.
        assert!(matches!(err(h.auth.create_account(Some(&me), &na("xx1", "role-admin"))), AuthError::Forbidden(_)));
        assert!(matches!(err(h.auth.create_account(Some(&me), &na("xx2", "role-operator"))), AuthError::Forbidden(_)), "operator punya Profil dan AI yang tidak dimiliki penelepon");
        let ok_role = h.auth.create_role(Some(&me), &RoleInput { name: "pelihat user".into(), description: String::new(), permissions: Permissions::default().with(Resource::User, Level::Read) }).unwrap();
        h.auth.create_account(Some(&me), &na("xx3", &ok_role.id)).unwrap();
        assert!(matches!(err(h.auth.create_role(Some(&me), &RoleInput { name: "naik".into(), description: String::new(), permissions: Permissions::default().with(Resource::Router, Level::Read) })), AuthError::Forbidden(_)));

        // Tidak bisa mengelola akun berizin lebih tinggi, maupun akun sendiri.
        let admin = h.account("owner", "role-admin");
        assert!(matches!(err(h.auth.reset_account_password(Some(&me), &admin.id, "password-baru")), AuthError::Forbidden(_)));
        assert!(matches!(err(h.auth.set_account_disabled(Some(&me), &admin.id, true)), AuthError::Forbidden(_)));
        assert!(matches!(err(h.auth.delete_account(Some(&me), &admin.id)), AuthError::Forbidden(_)));
        assert!(matches!(err(h.auth.reset_account_2fa(Some(&me), &admin.id)), AuthError::Forbidden(_)));
        assert!(err(h.auth.change_account_role(Some(&me), &me.account_id, &ok_role.id)).to_string().contains("sendiri"));
        // Akun yang sepadan atau lebih rendah boleh dikelola.
        let low = h.auth.create_account(Some(&me), &na("xx4", &ok_role.id)).unwrap();
        assert!(h.auth.reset_account_password(Some(&me), &low.id, "password-baru").is_ok());
        assert!(h.auth.change_account_role(Some(&me), &low.id, &ok_role.id).is_ok());
        assert!(matches!(err(h.auth.change_account_role(Some(&me), &low.id, "role-admin")), AuthError::Forbidden(_)));

        // Peran: tidak bisa mengubah peran yang sedang dipakai; tidak bisa menaikkan izinnya.
        assert!(err(h.auth.update_role(Some(&me), &my_role, &RoleInput { name: "kecil".into(), description: String::new(), permissions: limited.with(Resource::Admin, Level::Update) })).to_string().contains("sedang Anda pakai"));
        assert!(matches!(err(h.auth.update_role(Some(&me), &ok_role.id, &RoleInput { name: "pelihat user".into(), description: String::new(), permissions: Permissions::default().with(Resource::Pengaturan, Level::Read) })), AuthError::Forbidden(_)));

        // Tanpa izin Admin sama sekali: semuanya ditolak.
        let (nobody, _) = {
            let r = h.auth.create_role(None, &RoleInput { name: "biasa".into(), description: String::new(), permissions: Permissions::default().with(Resource::User, Level::Read) }).unwrap();
            h.account("biasa", &r.id);
            let (_, s, _) = h.login("biasa");
            (s, r.id)
        };
        assert!(matches!(err(h.auth.list_accounts(Some(&nobody))), AuthError::Forbidden(_)));
        assert!(matches!(err(h.auth.list_roles(Some(&nobody))), AuthError::Forbidden(_)));
        assert!(matches!(err(h.auth.create_account(Some(&nobody), &na("zzz", &ok_role.id))), AuthError::Forbidden(_)));
        // Lewat akses publik tanpa izin tulis publik: Admin(Ubah) ditolak.
        let via_pub = Session { via: Via::Publik, ..me.clone() };
        assert!(matches!(err(h.auth.create_account(Some(&via_pub), &na("zyy", &ok_role.id))), AuthError::Forbidden(_)));
    }

    #[test]
    fn peran_crud_dan_aturan_bawaan() {
        let h = setup();
        let ri = |n: &str, p: Permissions| RoleInput { name: n.into(), description: "d".into(), permissions: p };
        // Menu tanpa fitur ubah tidak bisa diberi Ubah.
        assert!(err(h.auth.create_role(None, &ri("x", Permissions::default().with(Resource::Event, Level::Update)))).to_string().contains("hanya punya"));
        assert!(err(h.auth.create_role(None, &ri(" ", Permissions::default()))).to_string().contains("Nama peran"));
        let r = h.auth.create_role(None, &ri("Tamu", Permissions::default().with(Resource::Ringkasan, Level::Read))).unwrap();
        assert!(err(h.auth.create_role(None, &ri("tamu", Permissions::default()))).to_string().contains("sudah dipakai"));
        let u = h.auth.update_role(None, &r.id, &ri("Tamu 2", Permissions::default().with(Resource::Event, Level::Read))).unwrap();
        assert_eq!((u.name.as_str(), u.permissions.get(Resource::Event), u.permissions.get(Resource::Ringkasan)), ("Tamu 2", Level::Read, Level::None));
        // Dipakai akun: tidak bisa dihapus. Bawaan: tidak bisa dihapus.
        let acc = h.account("budi", &r.id);
        assert!(matches!(err(h.auth.delete_role(None, &r.id)), AuthError::Forbidden(_)));
        assert!(matches!(err(h.auth.delete_role(None, "role-admin")), AuthError::Forbidden(_)));
        h.auth.delete_account(None, &acc.id).unwrap();
        h.auth.delete_role(None, &r.id).unwrap();
        assert_eq!(err(h.auth.delete_role(None, &r.id)), AuthError::NotFound);
        // Peran bawaan: nama tetap, izin bisa diubah lalu dikembalikan ke bawaan.
        let changed = h.auth.update_role(None, "role-viewer", &ri("Nama Lain", Permissions::default().with(Resource::User, Level::Update))).unwrap();
        assert_eq!((changed.name.as_str(), changed.permissions.get(Resource::User)), ("Viewer", Level::Update));
        let back = h.auth.reset_builtin_role(None, "role-viewer").unwrap();
        assert_eq!((back.permissions.get(Resource::User), back.permissions.get(Resource::Router)), (Level::Read, Level::Read));
        assert_eq!(err(h.auth.reset_builtin_role(None, "role-kustom")), AuthError::NotFound);
    }

    #[test]
    fn hapus_akun_terakhir_saat_publik_aktif_ditolak() {
        let h = setup();
        let a = h.account("satu", "role-viewer");
        h.auth.set_policy(AuthPolicy { public_on: true, ..Default::default() });
        assert!(err(h.auth.delete_account(None, &a.id)).to_string().contains("Akses publik"));
        h.auth.set_policy(AuthPolicy::default());
        assert!(h.auth.delete_account(None, &a.id).is_ok());
    }

    #[test]
    fn reset_password_dan_2fa_oleh_admin_mengeluarkan_sesi() {
        let h = setup();
        let a = h.account("operator", "role-operator");
        let (token, _, secret) = h.login("operator");
        assert!(secret.is_some());
        h.auth.reset_account_password(None, &a.id, "password-reset").unwrap();
        assert_eq!(err(h.auth.authenticate(&token)), AuthError::Unauthenticated);
        assert!(matches!(h.auth.login("operator", "password-uji", Via::Lan, "", "").unwrap_err(), AuthError::BadCredentials));
        let LoginOutcome::TwoFactorRequired { .. } = h.auth.login("operator", "password-reset", Via::Lan, "", "").unwrap() else { panic!("2FA masih aktif") };
        let s = h.auth.reset_account_2fa(None, &a.id).unwrap();
        assert!(!s.two_factor);
        assert!(matches!(h.auth.login("operator", "password-reset", Via::Lan, "", "").unwrap(), LoginOutcome::TwoFactorSetupRequired { .. }), "peran Ubah wajib mendaftar ulang");
    }

    #[test]
    fn rahasia_2fa_butuh_penyimpanan_terbuka() {
        let store: SharedStore = Arc::new(Mutex::new(Store::open_in_memory().unwrap())); // tidak di-unlock
        let auth = Auth::with_clock(store, HashParams { memory_kib: 8, iterations: 1, parallelism: 1 }, Box::new(|| T0)).unwrap();
        auth.create_account(None, &NewAccount { username: "operator".into(), display_name: "O".into(), password: "password-uji".into(), role_id: "role-operator".into() }).unwrap();
        assert_eq!(err(auth.login("operator", "password-uji", Via::Lan, "", "")), AuthError::VaultLocked);
        // Akun tanpa kewajiban 2FA tetap bisa masuk.
        auth.create_account(None, &NewAccount { username: "pantau".into(), display_name: "P".into(), password: "password-uji".into(), role_id: "role-monitor".into() }).unwrap();
        assert!(matches!(auth.login("pantau", "password-uji", Via::Lan, "", "").unwrap(), LoginOutcome::Ok { .. }));
    }

    #[test]
    fn riwayat_akun_tercatat_tanpa_rahasia() {
        let h = setup();
        let a = h.account("operator", "role-operator");
        let (_, _, secret) = h.login("operator");
        let _ = h.auth.login("operator", "salah-total", Via::Lan, "", "");
        h.auth.reset_account_password(None, &a.id, "rahasia-baru-123").unwrap();
        h.auth.update_role(None, "role-viewer", &RoleInput { name: "x".into(), description: "".into(), permissions: Permissions::default().with(Resource::User, Level::Read) }).unwrap();
        let log = h.store.lock().unwrap().list_audit(&AuditQuery { limit: 100, ..Default::default() }).unwrap();
        let actions: Vec<&str> = log.iter().map(|l| l.entry.action.as_str()).collect();
        for want in ["account.create", "auth.2fa_enabled", "auth.login", "auth.login_failed", "account.reset_password", "role.update"] {
            assert!(actions.contains(&want), "{want} tidak tercatat: {actions:?}");
        }
        let role = log.iter().find(|l| l.entry.action == "role.update").unwrap();
        assert!(role.entry.changes.iter().any(|c| c.field == "router" && c.from.as_deref() == Some("read") && c.to.as_deref() == Some("none")));
        assert!(log.iter().all(|l| l.entry.router_id == "-"));
        let dump = format!("{log:?}");
        for secret_text in ["password-uji", "salah-total", "rahasia-baru-123", &secret.unwrap()] {
            assert!(!dump.contains(secret_text), "rahasia bocor di riwayat: {secret_text}");
        }
    }
}
