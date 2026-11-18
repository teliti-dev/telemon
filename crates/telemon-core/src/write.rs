//! Penulisan ke router (ADR 0004): hanya `/ppp/secret` dan `/ppp/profile`, lewat rencana yang
//! bisa ditinjau dulu. Tidak pernah `remove` pada secret/profile: hapus = nonaktifkan secret dan
//! putus sesinya; profile hanya ditandai di Telemon (lihat [`check_profile_deletable`]).
//!
//! Alur: `plan_*` (validasi + baca kondisi router + daftar perubahan) -> UI menampilkan
//! [`Plan::changes`] -> [`apply`] menjalankan langkah -> pemanggil menyinkronkan ulang (`sync_router`).

use std::fmt;
use std::net::Ipv4Addr;

use crate::router::{Error, Row, RouterApi};

/// Satu-satunya perintah yang boleh dikirim [`apply`]. `/ppp/active/remove` hanya memutus sesi.
pub const ALLOWED: &[&str] = &[
    "/ppp/secret/add",
    "/ppp/secret/set",
    "/ppp/profile/add",
    "/ppp/profile/set",
    "/ppp/active/remove",
];

#[derive(Clone, PartialEq, Eq)]
pub struct Step {
    pub cmd: String,
    pub attrs: Vec<(String, String)>,
    /// Kegagalan langkah ini tidak menggagalkan rencana (mis. sesi sudah putus sendiri).
    pub optional: bool,
}

impl fmt::Debug for Step {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let attrs: Vec<(&str, &str)> = self.attrs.iter().map(|(k, v)| (k.as_str(), if k == "password" { "<disembunyikan>" } else { v.as_str() })).collect();
        f.debug_struct("Step").field("cmd", &self.cmd).field("attrs", &attrs).field("optional", &self.optional).finish()
    }
}

/// Satu baris tinjauan. Nilai password tidak pernah dimasukkan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub field: String,
    pub from: Option<String>,
    pub to: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    pub title: String,
    pub changes: Vec<Change>,
    pub steps: Vec<Step>,
}

#[derive(Debug, Clone, Default)]
pub struct NewUser {
    pub name: String,
    pub password: String,
    pub profile: String,
    pub service: Option<String>,
    pub remote_address: Option<String>,
    pub comment: Option<String>,
    pub disabled: bool,
}

/// `Some(None)` pada kolom opsional = kosongkan; `None` = tidak diubah.
#[derive(Debug, Clone, Default)]
pub struct UserPatch {
    pub password: Option<String>,
    pub profile: Option<String>,
    pub remote_address: Option<Option<String>>,
    pub comment: Option<Option<String>>,
    pub disabled: Option<bool>,
}

#[derive(Debug, Clone, Default)]
pub struct NewProfile {
    pub name: String,
    pub rate_limit: Option<String>,
    pub local_address: Option<String>,
    pub remote_address: Option<String>,
    /// `default`, `yes`, atau `no`.
    pub only_one: Option<String>,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ProfilePatch {
    pub rate_limit: Option<Option<String>>,
    pub local_address: Option<Option<String>>,
    pub remote_address: Option<Option<String>>,
    pub only_one: Option<String>,
    pub comment: Option<Option<String>>,
}

fn rej<T>(m: impl Into<String>) -> Result<T, Error> {
    Err(Error::Rejected(m.into()))
}

fn attr(k: &str, v: impl Into<String>) -> (String, String) {
    (k.to_string(), v.into())
}

fn blank(s: &Option<String>) -> Option<String> {
    s.as_deref().map(str::trim).filter(|v| !v.is_empty()).map(String::from)
}

pub fn valid_username(s: &str) -> bool {
    (2..=40).contains(&s.chars().count()) && s.chars().all(|c| c.is_alphanumeric() || matches!(c, '_' | '.' | '@' | '-'))
}

/// `10M`, `10M/20M`, `512k/1M`.
pub fn valid_rate(s: &str) -> bool {
    let part = |p: &str| {
        let d = p.trim_end_matches(['k', 'K', 'm', 'M', 'g', 'G']);
        !d.is_empty() && d.chars().all(|c| c.is_ascii_digit()) && p.len() - d.len() <= 1
    };
    let mut it = s.split('/');
    matches!((it.next(), it.next(), it.next()), (Some(a), None, None) if part(a)) || matches!((s.split_once('/'), s.matches('/').count()), (Some((a, b)), 1) if part(a) && part(b))
}

fn valid_ip(s: &str) -> bool {
    s.parse::<Ipv4Addr>().is_ok()
}

fn check_ip(label: &str, v: &Option<String>) -> Result<(), Error> {
    match blank(v) {
        Some(ip) if !valid_ip(&ip) => rej(format!("{label} bukan alamat IPv4 yang valid")),
        _ => Ok(()),
    }
}

async fn profile_exists(api: &impl RouterApi, name: &str) -> Result<(), Error> {
    if api.find("/ppp/profile", "name", name).await?.is_empty() { rej(format!("profile {name} tidak ada di router")) } else { Ok(()) }
}

fn row_id(r: &Row) -> Result<String, Error> {
    r.get(".id").cloned().ok_or_else(|| Error::Fatal("router tidak mengirim .id".into()))
}

fn is_true(r: &Row, k: &str) -> bool {
    r.get(k).is_some_and(|v| crate::model::is_true(v))
}

fn show(v: &str) -> Option<String> {
    (!v.is_empty()).then(|| v.to_string())
}

// ---------------- user ----------------

pub async fn plan_add_user(api: &impl RouterApi, u: &NewUser) -> Result<Plan, Error> {
    let name = u.name.trim();
    if !valid_username(name) {
        return rej("username 2-40 karakter: huruf, angka, _ . @ -");
    }
    if u.password.chars().count() < 6 {
        return rej("password minimal 6 karakter");
    }
    check_ip("IP statis", &u.remote_address)?;
    profile_exists(api, &u.profile).await?;
    if !api.find("/ppp/secret", "name", name).await?.is_empty() {
        return rej(format!("user {name} sudah ada di router"));
    }
    let service = blank(&u.service).unwrap_or_else(|| "pppoe".into());
    let mut attrs = vec![attr("name", name), attr("password", u.password.clone()), attr("profile", u.profile.clone()), attr("service", service.clone())];
    let mut changes = vec![
        Change { field: "username".into(), from: None, to: Some(name.into()) },
        Change { field: "password".into(), from: None, to: Some("diatur".into()) },
        Change { field: "profile".into(), from: None, to: Some(u.profile.clone()) },
        Change { field: "service".into(), from: None, to: Some(service) },
    ];
    if let Some(ip) = blank(&u.remote_address) {
        attrs.push(attr("remote-address", ip.clone()));
        changes.push(Change { field: "remote-address".into(), from: None, to: Some(ip) });
    }
    if let Some(c) = blank(&u.comment) {
        attrs.push(attr("comment", c.clone()));
        changes.push(Change { field: "comment".into(), from: None, to: Some(c) });
    }
    if u.disabled {
        attrs.push(attr("disabled", "yes"));
        changes.push(Change { field: "disabled".into(), from: None, to: Some("ya".into()) });
    }
    Ok(Plan { title: format!("Tambah user {name}"), changes, steps: vec![Step { cmd: "/ppp/secret/add".into(), attrs, optional: false }] })
}

pub async fn plan_edit_user(api: &impl RouterApi, name: &str, p: &UserPatch) -> Result<Plan, Error> {
    let cur = api.find("/ppp/secret", "name", name).await?.into_iter().next();
    let Some(cur) = cur else { return rej(format!("user {name} tidak ada di router")) };
    let mut attrs = vec![attr(".id", row_id(&cur)?)];
    let mut changes = Vec::new();

    if let Some(pw) = &p.password {
        if pw.chars().count() < 6 {
            return rej("password minimal 6 karakter");
        }
        attrs.push(attr("password", pw.clone()));
        changes.push(Change { field: "password".into(), from: None, to: Some("diganti".into()) });
    }
    if let Some(pr) = &p.profile {
        let old = cur.get("profile").cloned().unwrap_or_default();
        if *pr != old {
            profile_exists(api, pr).await?;
            attrs.push(attr("profile", pr.clone()));
            changes.push(Change { field: "profile".into(), from: show(&old), to: Some(pr.clone()) });
        }
    }
    for (field, new) in [("remote-address", &p.remote_address), ("comment", &p.comment)] {
        if let Some(new) = new {
            if field == "remote-address" {
                check_ip("IP statis", new)?;
            }
            let new = blank(new);
            let old = cur.get(field).and_then(|v| show(v));
            if new != old {
                attrs.push(attr(field, new.clone().unwrap_or_default()));
                changes.push(Change { field: field.into(), from: old, to: new });
            }
        }
    }
    let mut kick = None;
    if let Some(dis) = p.disabled {
        if dis != is_true(&cur, "disabled") {
            attrs.push(attr("disabled", if dis { "yes" } else { "no" }));
            changes.push(Change { field: "disabled".into(), from: Some(if dis { "tidak" } else { "ya" }.into()), to: Some(if dis { "ya" } else { "tidak" }.into()) });
            if dis {
                kick = api.find("/ppp/active", "name", name).await?.into_iter().next();
            }
        }
    }
    if changes.is_empty() {
        return rej("tidak ada perubahan");
    }
    let mut steps = vec![Step { cmd: "/ppp/secret/set".into(), attrs, optional: false }];
    if let Some(a) = kick {
        steps.push(Step { cmd: "/ppp/active/remove".into(), attrs: vec![attr(".id", row_id(&a)?)], optional: true });
        changes.push(Change { field: "sesi aktif".into(), from: Some("terhubung".into()), to: Some("diputus".into()) });
    }
    Ok(Plan { title: format!("Ubah user {name}"), changes, steps })
}

/// Hapus (soft): secret dinonaktifkan dan sesinya diputus. Secret tidak dihapus dari router.
pub async fn plan_soft_delete_user(api: &impl RouterApi, name: &str) -> Result<Plan, Error> {
    let mut plan = plan_edit_user(api, name, &UserPatch { disabled: Some(true), ..Default::default() }).await.map_err(|e| match e {
        Error::Rejected(m) if m == "tidak ada perubahan" => Error::Rejected(format!("user {name} sudah dinonaktifkan")),
        other => other,
    })?;
    plan.title = format!("Hapus (soft) user {name}");
    Ok(plan)
}

pub async fn plan_restore_user(api: &impl RouterApi, name: &str) -> Result<Plan, Error> {
    let mut plan = plan_edit_user(api, name, &UserPatch { disabled: Some(false), ..Default::default() }).await.map_err(|e| match e {
        Error::Rejected(m) if m == "tidak ada perubahan" => Error::Rejected(format!("user {name} tidak dinonaktifkan")),
        other => other,
    })?;
    plan.title = format!("Pulihkan user {name}");
    Ok(plan)
}

// ---------------- profile ----------------

fn check_profile_fields(rate: &Option<String>, local: &Option<String>, remote: &Option<String>, only_one: &Option<String>) -> Result<(), Error> {
    if let Some(r) = blank(rate).filter(|r| !valid_rate(r)) {
        return rej(format!("rate limit {r} tidak valid (contoh 10M/10M)"));
    }
    check_ip("Local address", local)?;
    check_ip("Remote address", remote)?;
    if let Some(o) = only_one && !matches!(o.as_str(), "default" | "yes" | "no") {
        return rej("only-one harus default, yes, atau no");
    }
    Ok(())
}

pub async fn plan_add_profile(api: &impl RouterApi, p: &NewProfile) -> Result<Plan, Error> {
    let name = p.name.trim();
    if name.is_empty() || name.len() > 40 {
        return rej("nama profile 1-40 karakter");
    }
    check_profile_fields(&p.rate_limit, &p.local_address, &p.remote_address, &p.only_one)?;
    if !api.find("/ppp/profile", "name", name).await?.is_empty() {
        return rej(format!("profile {name} sudah ada di router"));
    }
    let mut attrs = vec![attr("name", name)];
    let mut changes = vec![Change { field: "nama".into(), from: None, to: Some(name.into()) }];
    for (field, v) in [("rate-limit", &p.rate_limit), ("local-address", &p.local_address), ("remote-address", &p.remote_address), ("comment", &p.comment)] {
        if let Some(v) = blank(v) {
            attrs.push(attr(field, v.clone()));
            changes.push(Change { field: field.into(), from: None, to: Some(v) });
        }
    }
    if let Some(o) = &p.only_one {
        attrs.push(attr("only-one", o.clone()));
        changes.push(Change { field: "only-one".into(), from: None, to: Some(o.clone()) });
    }
    Ok(Plan { title: format!("Tambah profile {name}"), changes, steps: vec![Step { cmd: "/ppp/profile/add".into(), attrs, optional: false }] })
}

pub async fn plan_edit_profile(api: &impl RouterApi, name: &str, p: &ProfilePatch) -> Result<Plan, Error> {
    let Some(cur) = api.find("/ppp/profile", "name", name).await?.into_iter().next() else {
        return rej(format!("profile {name} tidak ada di router"));
    };
    check_profile_fields(&p.rate_limit.clone().flatten(), &p.local_address.clone().flatten(), &p.remote_address.clone().flatten(), &p.only_one)?;
    let mut attrs = vec![attr(".id", row_id(&cur)?)];
    let mut changes = Vec::new();
    for (field, new) in [("rate-limit", &p.rate_limit), ("local-address", &p.local_address), ("remote-address", &p.remote_address), ("comment", &p.comment)] {
        if let Some(new) = new {
            let new = blank(new);
            let old = cur.get(field).and_then(|v| show(v));
            if new != old {
                attrs.push(attr(field, new.clone().unwrap_or_default()));
                changes.push(Change { field: field.into(), from: old, to: new });
            }
        }
    }
    if let Some(o) = &p.only_one {
        let old = cur.get("only-one").cloned().unwrap_or_else(|| "default".into());
        if *o != old {
            attrs.push(attr("only-one", o.clone()));
            changes.push(Change { field: "only-one".into(), from: Some(old), to: Some(o.clone()) });
        }
    }
    if changes.is_empty() {
        return rej("tidak ada perubahan");
    }
    Ok(Plan { title: format!("Ubah profile {name}"), changes, steps: vec![Step { cmd: "/ppp/profile/set".into(), attrs, optional: false }] })
}

/// Profile hanya ditandai terhapus di Telemon (RouterOS tidak punya flag disabled untuk profile);
/// ditolak bila masih dipakai secret di router.
pub async fn check_profile_deletable(api: &impl RouterApi, name: &str) -> Result<(), Error> {
    let used = api.find("/ppp/secret", "profile", name).await?.len();
    if used > 0 { rej(format!("profile {name} masih dipakai {used} user")) } else { Ok(()) }
}

// ---------------- eksekusi ----------------

/// Jalankan rencana. `Ok(n)` = n langkah selesai. `Err((n, e))` = gagal setelah n langkah selesai
/// (langkah opsional yang gagal tidak dihitung sebagai kegagalan).
pub async fn apply(api: &impl RouterApi, plan: &Plan) -> Result<usize, (usize, Error)> {
    // Periksa semua langkah dulu: satu perintah terlarang membatalkan seluruh rencana sebelum ada yang terkirim.
    if let Some(bad) = plan.steps.iter().find(|s| !ALLOWED.contains(&s.cmd.as_str())) {
        return Err((0, Error::Rejected(format!("perintah {} tidak diizinkan", bad.cmd))));
    }
    let mut done = 0;
    for step in &plan.steps {
        match api.run(&step.cmd, &step.attrs).await {
            Ok(_) => done += 1,
            Err(_) if step.optional => {}
            Err(e) => return Err((done, e)),
        }
    }
    Ok(done)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    fn row(kv: &[(&str, &str)]) -> Row {
        kv.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    /// Router tiruan dengan keadaan: menerima hanya perintah tulis yang wajar dan menolak sisanya.
    struct Sim {
        secrets: Mutex<Vec<Row>>,
        profiles: Mutex<Vec<Row>>,
        active: Mutex<Vec<Row>>,
        calls: Mutex<Vec<String>>,
        next: Mutex<u32>,
    }

    impl Sim {
        fn new() -> Self {
            Self {
                secrets: Mutex::new(vec![row(&[(".id", "*1"), ("name", "budi"), ("password", "lama123"), ("profile", "10M"), ("disabled", "false"), ("service", "pppoe")])]),
                profiles: Mutex::new(vec![row(&[(".id", "*p1"), ("name", "10M"), ("rate-limit", "10M/10M")]), row(&[(".id", "*p2"), ("name", "20M"), ("rate-limit", "20M/20M")])]),
                active: Mutex::new(vec![row(&[(".id", "*a1"), ("name", "budi"), ("service", "pppoe"), ("uptime", "1m")])]),
                calls: Mutex::new(vec![]),
                next: Mutex::new(100),
            }
        }
    }

    fn table<'a>(s: &'a Sim, path: &str) -> Option<&'a Mutex<Vec<Row>>> {
        match path {
            "/ppp/secret" => Some(&s.secrets),
            "/ppp/profile" => Some(&s.profiles),
            "/ppp/active" => Some(&s.active),
            _ => None,
        }
    }

    impl RouterApi for Sim {
        async fn print(&self, path: &str) -> Result<Vec<Row>, Error> {
            table(self, path).map(|t| t.lock().unwrap().clone()).ok_or_else(|| Error::Trap("no such command".into()))
        }

        async fn run(&self, cmd: &str, attrs: &[(String, String)]) -> Result<Vec<Row>, Error> {
            self.calls.lock().unwrap().push(cmd.to_string());
            let (path, verb) = cmd.rsplit_once('/').unwrap();
            let Some(t) = table(self, path) else { return Err(Error::Trap("no such command".into())) };
            let mut rows = t.lock().unwrap();
            let get = |k: &str| attrs.iter().find(|(a, _)| a == k).map(|(_, v)| v.clone());
            match (path, verb) {
                ("/ppp/secret" | "/ppp/profile", "add") => {
                    let name = get("name").ok_or_else(|| Error::Trap("missing name".into()))?;
                    if rows.iter().any(|r| r["name"] == name) {
                        return Err(Error::Trap("failure: already have such entry".into()));
                    }
                    let mut n = self.next.lock().unwrap();
                    *n += 1;
                    let mut r: Row = attrs.iter().cloned().collect();
                    r.insert(".id".into(), format!("*n{n}"));
                    r.entry("disabled".into()).or_insert_with(|| "false".into());
                    rows.push(r);
                    Ok(vec![])
                }
                ("/ppp/secret" | "/ppp/profile", "set") => {
                    let id = get(".id").unwrap();
                    let r = rows.iter_mut().find(|r| r[".id"] == id).ok_or_else(|| Error::Trap("no such item".into()))?;
                    for (k, v) in attrs.iter().filter(|(k, _)| k != ".id") {
                        if v.is_empty() { r.remove(k); } else { r.insert(k.clone(), if k == "disabled" { if v == "yes" { "true".into() } else { "false".into() } } else { v.clone() }); }
                    }
                    Ok(vec![])
                }
                ("/ppp/active", "remove") => {
                    let id = get(".id").unwrap();
                    let before = rows.len();
                    rows.retain(|r| r[".id"] != id);
                    if rows.len() == before { Err(Error::Trap("no such item".into())) } else { Ok(vec![]) }
                }
                _ => Err(Error::Trap("unknown command".into())),
            }
        }
    }

    fn rejected<T: fmt::Debug>(r: Result<T, Error>) -> String {
        match r {
            Err(Error::Rejected(m)) => m,
            other => panic!("harusnya Rejected, dapat {other:?}"),
        }
    }

    #[tokio::test]
    async fn tambah_user_validasi_dan_terapkan() {
        let sim = Sim::new();
        let nu = |name: &str, pw: &str, prof: &str| NewUser { name: name.into(), password: pw.into(), profile: prof.into(), ..Default::default() };
        assert!(rejected(plan_add_user(&sim, &nu("a", "rahasia1", "10M")).await).contains("username"));
        assert!(rejected(plan_add_user(&sim, &nu("sari baru", "rahasia1", "10M")).await).contains("username"));
        assert!(rejected(plan_add_user(&sim, &nu("sari", "123", "10M")).await).contains("password"));
        assert!(rejected(plan_add_user(&sim, &nu("sari", "rahasia1", "99M")).await).contains("tidak ada"));
        assert!(rejected(plan_add_user(&sim, &nu("budi", "rahasia1", "10M")).await).contains("sudah ada"));
        let bad_ip = NewUser { remote_address: Some("300.1.1.1".into()), ..nu("sari", "rahasia1", "10M") };
        assert!(rejected(plan_add_user(&sim, &bad_ip).await).contains("IPv4"));
        assert!(sim.calls.lock().unwrap().is_empty(), "validasi tidak boleh menulis");

        let plan = plan_add_user(&sim, &NewUser { remote_address: Some("100.64.0.8".into()), comment: Some("VIP".into()), ..nu("sari", "rahasia1", "10M") }).await.unwrap();
        assert!(plan.changes.iter().any(|c| c.field == "password" && c.to.as_deref() == Some("diatur")));
        assert!(!format!("{plan:?}").contains("rahasia1"), "password tidak boleh tercetak");
        assert_eq!(apply(&sim, &plan).await.unwrap(), 1);
        let secrets = sim.secrets.lock().unwrap();
        let s = secrets.iter().find(|r| r["name"] == "sari").unwrap();
        assert_eq!((s["profile"].as_str(), s["remote-address"].as_str(), s["password"].as_str()), ("10M", "100.64.0.8", "rahasia1"));
    }

    #[tokio::test]
    async fn ubah_user_hanya_yang_berbeda() {
        let sim = Sim::new();
        let none = rejected(plan_edit_user(&sim, "budi", &UserPatch { profile: Some("10M".into()), comment: Some(None), ..Default::default() }).await);
        assert_eq!(none, "tidak ada perubahan");
        assert!(rejected(plan_edit_user(&sim, "hantu", &UserPatch::default()).await).contains("tidak ada"));
        assert!(rejected(plan_edit_user(&sim, "budi", &UserPatch { profile: Some("99M".into()), ..Default::default() }).await).contains("tidak ada di router"));

        let plan = plan_edit_user(&sim, "budi", &UserPatch { profile: Some("20M".into()), password: Some("baru-baru".into()), comment: Some(Some("pindah".into())), ..Default::default() }).await.unwrap();
        let fields: Vec<_> = plan.changes.iter().map(|c| c.field.as_str()).collect();
        assert_eq!(fields, vec!["password", "profile", "comment"]);
        let prof = plan.changes.iter().find(|c| c.field == "profile").unwrap();
        assert_eq!((prof.from.as_deref(), prof.to.as_deref()), (Some("10M"), Some("20M")));
        assert!(!format!("{plan:?}").contains("baru-baru"));
        apply(&sim, &plan).await.unwrap();
        let secrets = sim.secrets.lock().unwrap();
        assert_eq!((secrets[0]["profile"].as_str(), secrets[0]["password"].as_str(), secrets[0]["comment"].as_str()), ("20M", "baru-baru", "pindah"));
    }

    #[tokio::test]
    async fn hapus_soft_menonaktifkan_dan_memutus_sesi() {
        let sim = Sim::new();
        let plan = plan_soft_delete_user(&sim, "budi").await.unwrap();
        assert_eq!(plan.steps.len(), 2);
        assert!(plan.changes.iter().any(|c| c.field == "sesi aktif"));
        assert_eq!(apply(&sim, &plan).await.unwrap(), 2);
        assert_eq!(sim.secrets.lock().unwrap().len(), 1, "secret tidak dihapus dari router");
        assert_eq!(sim.secrets.lock().unwrap()[0]["disabled"], "true");
        assert!(sim.active.lock().unwrap().is_empty());
        assert!(rejected(plan_soft_delete_user(&sim, "budi").await).contains("sudah dinonaktifkan"));

        let restore = plan_restore_user(&sim, "budi").await.unwrap();
        assert_eq!(restore.steps.len(), 1);
        apply(&sim, &restore).await.unwrap();
        assert_eq!(sim.secrets.lock().unwrap()[0]["disabled"], "false");
        assert!(rejected(plan_restore_user(&sim, "budi").await).contains("tidak dinonaktifkan"));
    }

    #[tokio::test]
    async fn langkah_opsional_boleh_gagal() {
        let sim = Sim::new();
        let plan = plan_soft_delete_user(&sim, "budi").await.unwrap();
        sim.active.lock().unwrap().clear(); // sesi putus sendiri sebelum diterapkan
        assert_eq!(apply(&sim, &plan).await.unwrap(), 1);
    }

    #[tokio::test]
    async fn perintah_di_luar_daftar_ditolak_sebelum_terkirim() {
        let sim = Sim::new();
        for cmd in ["/ppp/secret/remove", "/ppp/profile/remove", "/system/reboot", "/user/add", "/ppp/secret/disable"] {
            let plan = Plan { title: "x".into(), changes: vec![], steps: vec![Step { cmd: "/ppp/secret/set".into(), attrs: vec![], optional: false }, Step { cmd: cmd.into(), attrs: vec![], optional: false }] };
            let (done, e) = apply(&sim, &plan).await.unwrap_err();
            assert_eq!(done, 0, "{cmd}: tidak boleh ada yang terkirim");
            assert!(matches!(e, Error::Rejected(_)), "{cmd}");
        }
        assert!(sim.calls.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn gagal_di_router_melaporkan_langkah_selesai() {
        let sim = Sim::new();
        let plan = Plan { title: "x".into(), changes: vec![], steps: vec![
            Step { cmd: "/ppp/profile/add".into(), attrs: vec![attr("name", "30M")], optional: false },
            Step { cmd: "/ppp/profile/add".into(), attrs: vec![attr("name", "30M")], optional: false },
        ] };
        let (done, e) = apply(&sim, &plan).await.unwrap_err();
        assert_eq!(done, 1);
        assert!(matches!(e, Error::Trap(_)));
    }

    #[tokio::test]
    async fn profile_tambah_ubah_dan_hapus() {
        let sim = Sim::new();
        let np = |n: &str| NewProfile { name: n.into(), ..Default::default() };
        assert!(rejected(plan_add_profile(&sim, &np("")).await).contains("nama"));
        assert!(rejected(plan_add_profile(&sim, &np("10M")).await).contains("sudah ada"));
        assert!(rejected(plan_add_profile(&sim, &NewProfile { rate_limit: Some("10Mbps".into()), ..np("50M") }).await).contains("rate limit"));
        assert!(rejected(plan_add_profile(&sim, &NewProfile { only_one: Some("maybe".into()), ..np("50M") }).await).contains("only-one"));
        assert!(rejected(plan_add_profile(&sim, &NewProfile { local_address: Some("x".into()), ..np("50M") }).await).contains("Local"));
        let plan = plan_add_profile(&sim, &NewProfile { rate_limit: Some("50M/50M".into()), only_one: Some("yes".into()), ..np("50M") }).await.unwrap();
        apply(&sim, &plan).await.unwrap();
        assert_eq!(sim.profiles.lock().unwrap().len(), 3);

        assert_eq!(rejected(plan_edit_profile(&sim, "10M", &ProfilePatch { rate_limit: Some(Some("10M/10M".into())), ..Default::default() }).await), "tidak ada perubahan");
        let e = plan_edit_profile(&sim, "10M", &ProfilePatch { rate_limit: Some(Some("15M/15M".into())), ..Default::default() }).await.unwrap();
        assert_eq!(e.changes[0].from.as_deref(), Some("10M/10M"));
        apply(&sim, &e).await.unwrap();
        assert_eq!(sim.profiles.lock().unwrap()[0]["rate-limit"], "15M/15M");

        assert!(rejected(check_profile_deletable(&sim, "10M").await).contains("masih dipakai 1 user"));
        assert!(check_profile_deletable(&sim, "20M").await.is_ok());
    }

    #[test]
    fn validator() {
        assert!(valid_rate("10M") && valid_rate("10M/20M") && valid_rate("512k/1M") && valid_rate("100"));
        assert!(!valid_rate("") && !valid_rate("M") && !valid_rate("10MM") && !valid_rate("10/") && !valid_rate("1/2/3") && !valid_rate("10Mbps"));
        assert!(valid_username("budi.s@x-1") && !valid_username("a") && !valid_username("a b") && !valid_username(&"x".repeat(41)));
    }

    #[tokio::test]
    async fn tulis_lalu_sinkron_ke_store() {
        use crate::monitor::{SharedStore, sync_router};
        use crate::store::Store;
        use std::sync::Arc;
        let sim = Sim::new();
        let store: SharedStore = Arc::new(Mutex::new(Store::open_in_memory().unwrap()));
        sync_router(&sim, &store, "r1").await.unwrap();
        let plan = plan_add_user(&sim, &NewUser { name: "tika".into(), password: "rahasia1".into(), profile: "20M".into(), ..Default::default() }).await.unwrap();
        apply(&sim, &plan).await.unwrap();
        let del = plan_soft_delete_user(&sim, "budi").await.unwrap();
        apply(&sim, &del).await.unwrap();
        sync_router(&sim, &store, "r1").await.unwrap();
        let users = store.lock().unwrap().list_users(Some("r1"), false).unwrap();
        let by = |n: &str| users.iter().find(|u| u.username == n).unwrap().clone();
        assert_eq!(by("tika").profile, "20M");
        assert_eq!(by("budi").status, crate::poller::UserStatus::Disabled);
    }
}
