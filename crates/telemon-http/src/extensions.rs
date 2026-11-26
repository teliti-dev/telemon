//! Extension (ADR 0028), prototipe 1: hanya tingkat **official**, hanya UI (widget).
//!
//! - Mati kecuali `HttpConfig.extensions` diisi (desktop: bawaan; server: `TELEMON_EXTENSIONS=1`).
//! - Kelola (lihat daftar penuh, periksa, pasang, ubah, hapus): izin Admin; hanya Lihat untuk daftar,
//!   Ubah untuk sisanya. Ubah selalu butuh 2FA (kebijakan akun) dan tidak lewat akses Publik.
//! - `GET /api/extensions/active`: daftar ringkas extension aktif untuk UI (semua akun yang masuk).
//! - `GET /ext/{id}/{*path}`: berkas UI dari paket, hanya untuk extension aktif; paket diverifikasi ulang
//!   saat dimuat dari disk (bukan hanya saat pasang).
//! - Sumber pasang prototipe ini: unggah berkas. Tautan dan toko official menyusul (ADR 0028).

use std::path::PathBuf;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Json, Router};
use bytes::Bytes;
use serde::Deserialize;
use serde_json::{Value, json};
use telemon_core::auth::{Level, Resource, Via};
use telemon_core::{AuditEntry, AuditOutcome, Change, Connector, StoredExtension};
use telemon_ext::{Catalog, Dialect, Limits, Manifest, Package, Problem, Tier, check_install, dep_status, dependents_of, host_compatible, migration_sql, open_catalog, namespace, parse_migrations, plan_migrations, valid_id};

use crate::error::{ApiError, ApiResult};
use crate::guard::Caller;
use crate::state::{AppState, ExtConfig};

type St<C> = Arc<AppState<C>>;

pub fn routes<C: Connector>() -> Router<St<C>> {
    Router::new()
        .route("/api/extensions", get(list::<C>).post(install::<C>).layer(DefaultBodyLimit::max(Limits::default().max_package + 4096)))
        .route("/api/extensions/inspect", post(inspect::<C>).layer(DefaultBodyLimit::max(Limits::default().max_package + 4096)))
        .route("/api/extensions/active", get(active::<C>))
        .route("/api/extensions/store", get(store_list::<C>))
        .route("/api/extensions/store/install", post(store_install::<C>))
        .route("/api/extensions/{id}", axum::routing::patch(patch::<C>).delete(remove::<C>))
        .route("/api/extensions/{id}/access", get(access_get::<C>))
        .route("/api/extensions/{id}/access/{role}", axum::routing::put(access_put::<C>))
        .route("/ext/{id}/{*path}", get(file::<C>))
}

pub(crate) fn cfg<C: Connector>(st: &St<C>) -> Result<&ExtConfig, ApiError> {
    st.cfg.extensions.as_ref().ok_or_else(|| ApiError::new(StatusCode::NOT_IMPLEMENTED, "extensions_off", "Extension mati di server ini (TELEMON_EXTENSIONS=1 untuk menyalakan)"))
}

pub(crate) fn manage<C: Connector>(st: &St<C>, c: &Caller, level: Level) -> ApiResult<()> {
    st.auth.authorize(Some(&c.session), Resource::Admin, level).map_err(ApiError::from)?;
    if level == Level::Update && c.session.via == Via::Publik {
        return Err(ApiError::forbidden("Extension tidak bisa dikelola lewat akses Publik"));
    }
    Ok(())
}

pub(crate) fn e<T, E: std::fmt::Display>(r: Result<T, E>) -> Result<T, String> {
    r.map_err(|x| x.to_string())
}

fn problem_text(p: &Problem) -> String {
    match p {
        Problem::TelemonIncompatible { needs, have } => format!("Butuh Telemon {needs} (terpasang {have})"),
        Problem::MissingDependency { id, needs } => format!("Butuh extension {id} {needs} yang belum terpasang"),
        Problem::IncompatibleDependency { id, needs, have } => format!("Butuh {id} {needs} (terpasang {have})"),
        Problem::BreaksDependent { id, needs } => format!("{id} membutuhkan versi {needs} dari extension ini"),
        Problem::Cycle { id } => format!("Dependensi melingkar di {id}"),
    }
}

fn manifests(rows: &[StoredExtension]) -> Vec<Manifest> {
    rows.iter().filter_map(|r| serde_json::from_str::<Manifest>(&r.manifest).ok()).collect()
}

fn versions(rows: &[StoredExtension]) -> Vec<(String, String)> {
    rows.iter().map(|r| (r.id.clone(), r.version.clone())).collect()
}

/// Syarat wajib yang belum terpenuhi untuk dimuat: tidak terpasang, mati, atau versinya tidak cocok.
pub(crate) fn blocked_by(m: &Manifest, rows: &[StoredExtension]) -> Vec<String> {
    let active: Vec<(String, String)> = rows.iter().filter(|r| r.enabled).map(|r| (r.id.clone(), r.version.clone())).collect();
    dep_status(m, &active).into_iter().filter(|d| !d.optional && d.state != "ok").map(|d| d.id).collect()
}

pub(crate) fn all_rows<C: Connector>(st: &St<C>) -> ApiResult<Vec<StoredExtension>> {
    st.telemon.with_store(|s| s.ext_list()).map_err(ApiError::internal)
}

pub(crate) fn path_of(dir: &std::path::Path, id: &str) -> PathBuf {
    dir.join(format!("{id}.tmx"))
}

pub(crate) fn audit<C: Connector>(st: &St<C>, c: &Caller, action: &str, id: &str, title: String, changes: Vec<Change>) {
    let e = AuditEntry { at_ms: st.now(), actor: c.session.username.clone(), router_id: String::new(), action: action.into(), target: id.into(), title, changes, outcome: AuditOutcome::Ok };
    let _ = st.telemon.with_store(|s| s.record_audit(&e));
}

fn summary(p: &Package) -> Value {
    json!({
        "id": p.manifest.id, "name": p.manifest.name, "version": p.manifest.version, "publisher": p.manifest.publisher,
        "description": p.manifest.description, "tier": p.tier, "signed": p.signed,
        "widgets": p.manifest.contributes.widgets, "permissions": p.manifest.permissions,
    })
}

fn parse<C: Connector>(st: &St<C>, bytes: &[u8]) -> ApiResult<Package> {
    let cfg = cfg(st)?;
    let p = Package::open(bytes, &cfg.keys, &Limits::default()).map_err(|e| ApiError::bad(e.to_string()))?;
    // Prototipe 1: hanya official. Public menunggu runtime iframe (ADR 0028).
    if p.tier != Tier::Official {
        return Err(ApiError::bad("Extension non-official belum didukung (menunggu runtime sandbox)"));
    }
    Ok(p)
}

async fn inspect<C: Connector>(State(st): State<St<C>>, c: Caller, body: Bytes) -> ApiResult<Json<Value>> {
    manage(&st, &c, Level::Update)?;
    let p = parse(&st, &body)?;
    let rows = all_rows(&st)?;
    let existing = rows.iter().find(|r| r.id == p.manifest.id).map(|e| e.version.clone());
    let rest: Vec<StoredExtension> = rows.into_iter().filter(|r| r.id != p.manifest.id).collect();
    let problems = check_install(&p.manifest, &st.cfg.host_version, &versions(&rest), &manifests(&rest));
    let texts: Vec<String> = problems.iter().map(problem_text).collect();
    Ok(Json(json!({ "package": summary(&p), "installedVersion": existing, "requires": dep_status(&p.manifest, &versions(&rest)), "problems": problems, "problemTexts": texts })))
}

/// Pasang atau perbarui. Pasang baru: mati, izin wajib/opsional menyala, izin sensitif mati.
/// Pembaruan: status dan izin lama dipertahankan; izin baru mati sampai admin memutuskan.
async fn install<C: Connector>(State(st): State<St<C>>, c: Caller, body: Bytes) -> ApiResult<Json<Value>> {
    manage(&st, &c, Level::Update)?;
    Ok(Json(do_install(&st, &c, &body)?))
}

/// Inti pemasangan, dipakai unggah berkas dan toko: verifikasi, kompatibilitas, migrasi, tulis berkas, simpan.
fn do_install<C: Connector>(st: &St<C>, c: &Caller, body: &[u8]) -> ApiResult<Value> {
    let p = parse(st, body)?;
    if let Some(b) = &p.manifest.backend {
        let wasm = p.file(&b.entry).ok_or_else(|| ApiError::bad("backend.wasm tidak ada"))?;
        telemon_ext::Sandbox::new(wasm).map_err(|x| ApiError::bad(x.to_string()))?;
    }
    let dir = cfg(st)?.dir.clone();
    let id = p.manifest.id.clone();
    let now = st.now();
    {
        let rows: Vec<StoredExtension> = all_rows(st)?.into_iter().filter(|r| r.id != id).collect();
        let problems = check_install(&p.manifest, &st.cfg.host_version, &versions(&rows), &manifests(&rows));
        if !problems.is_empty() {
            let msg = problems.iter().map(problem_text).collect::<Vec<_>>().join("; ");
            return Err(ApiError::new(StatusCode::CONFLICT, "incompatible", msg));
        }
    }
    let manifest = serde_json::to_string(&p.manifest).map_err(ApiError::internal)?;
    let prev = st.telemon.with_store(|s| s.ext_get(&id)).map_err(ApiError::internal)?;
    std::fs::create_dir_all(&dir).map_err(ApiError::internal)?;
    run_migrations(st, &dir, &p, &id, now)?;
    let tmp = dir.join(format!(".{id}.tmx.part"));
    std::fs::write(&tmp, body).map_err(ApiError::internal)?;
    std::fs::rename(&tmp, path_of(&dir, &id)).map_err(ApiError::internal)?;
    let tier = "official";
    st.telemon
        .with_store(|s| -> Result<(), String> {
            e(s.ext_upsert(&id, &p.manifest.version, tier, p.key_id.as_deref(), p.signed, &manifest, now))?;
            let known: Vec<String> = e(s.ext_perms(&id))?.into_iter().map(|(k, _)| k).collect();
            for d in &p.manifest.permissions {
                if !known.contains(&d.id) {
                    e(s.ext_set_perm(&id, &d.id, prev.is_none() && !d.sensitive))?;
                }
            }
            Ok(())
        })
        .map_err(ApiError::internal)?;
    st.ext_cache.lock().expect("lock").remove(&id);
    st.ext_wasm.lock().expect("lock").remove(&id);
    // Versi baru: worker mulai bersih (tidak lagi dijeda oleh versi lama).
    st.ext_workers.lock().expect("lock").retain(|k, _| !k.starts_with(&format!("{id}/")));
    // Pasang baru: peran yang boleh mengubah menu Admin mendapat akses penuh; peran lain tanpa akses
    // sampai admin mengaturnya di halaman Akses.
    if prev.is_none()
        && let Ok(roles) = st.auth.list_roles(None)
    {
        let all: Vec<String> = p.manifest.actions.iter().map(|a| a.id.clone()).collect();
        for r in roles.iter().filter(|r| r.permissions.get(Resource::Admin) == Level::Update) {
            let _ = st.telemon.with_store(|s| s.ext_access_set(&id, &r.id, "update", &all));
        }
    }
    let from = prev.as_ref().map(|e| e.version.clone());
    audit(st, c, if prev.is_some() { "extension.update" } else { "extension.install" }, &id, format!("Pasang extension {} {}", p.manifest.name, p.manifest.version), vec![Change { field: "versi".into(), from, to: Some(p.manifest.version.clone()) }]);
    view(st, &id)
}

// ---------------- toko official (ADR 0028) ----------------

/// Pengunduh HTTPS untuk toko. Implementasi memastikan skema `https`, batas ukuran, dan waktu tunggu; dipasang
/// aplikasi desktop atau server lewat `AppState::set_ext_fetcher`.
pub trait ExtFetch: Send + Sync {
    fn get(&self, url: &str, max_bytes: usize) -> Result<Vec<u8>, String>;
}

const MAX_CATALOG: usize = 2 << 20;

fn store_deps<C: Connector>(st: &St<C>) -> ApiResult<(String, Arc<dyn ExtFetch>)> {
    let url = cfg(st)?.store_url.clone().ok_or_else(|| ApiError::new(StatusCode::NOT_IMPLEMENTED, "store_off", "Toko extension mati di server ini"))?;
    let f = st.ext_fetch.lock().expect("lock").clone().ok_or_else(|| ApiError::new(StatusCode::NOT_IMPLEMENTED, "store_off", "Toko extension belum tersedia (tanpa pengunduh)"))?;
    Ok((url, f))
}

/// Ambil dan verifikasi katalog (tanda tangan terhadap kunci official). Blokir: jalankan dari `spawn_blocking`.
fn fetch_catalog(url: &str, f: &dyn ExtFetch, official: &[String]) -> Result<Catalog, String> {
    let bytes = f.get(url, MAX_CATALOG)?;
    let sig = String::from_utf8(f.get(&format!("{url}.sig"), 1024)?).map_err(|_| "Tanda tangan katalog bukan teks".to_string())?;
    open_catalog(&bytes, sig.trim(), official)
}

/// Alamat paket = folder katalog + nama berkas yang sudah divalidasi (hanya `[A-Za-z0-9._-]`).
fn package_url(catalog_url: &str, file: &str) -> String {
    let base = catalog_url.rsplit_once('/').map_or(catalog_url, |(b, _)| b);
    format!("{base}/{file}")
}

async fn store_list<C: Connector>(State(st): State<St<C>>, c: Caller) -> ApiResult<Json<Value>> {
    manage(&st, &c, Level::Read)?;
    let (url, f) = store_deps(&st)?;
    let official = cfg(&st)?.keys.official.clone();
    let cat = tokio::task::spawn_blocking(move || fetch_catalog(&url, f.as_ref(), &official))
        .await
        .map_err(ApiError::internal)?
        .map_err(|m| ApiError::new(StatusCode::BAD_GATEWAY, "store_unreachable", m))?;
    let rows = all_rows(&st)?;
    let installed = versions(&rows);
    let items: Vec<Value> = cat
        .extensions
        .iter()
        .map(|e| {
            let m = &e.manifest;
            let have = installed.iter().find(|(id, _)| *id == m.id).map(|(_, v)| v.clone());
            let (will, problem) = match cat.plan(&m.id, &st.cfg.host_version, &installed) {
                Ok(p) => (p.iter().map(|x| x.manifest.id.clone()).filter(|i| *i != m.id).collect::<Vec<_>>(), None),
                Err(p) => (vec![], Some(problem_text(&p))),
            };
            json!({
                "id": m.id, "name": m.name, "version": m.version, "publisher": m.publisher, "description": m.description,
                "telemon": m.telemon, "requires": m.requires, "size": e.size, "installedVersion": have,
                "upToDate": have.as_deref() == Some(m.version.as_str()),
                "alsoInstalls": will, "problem": problem,
            })
        })
        .collect();
    Ok(Json(json!({ "generated": cat.generated, "extensions": items })))
}

#[derive(Deserialize)]
struct StoreInstallReq {
    id: String,
}

/// Pasang dari katalog: rencana (dependensi dulu), unduh tiap paket, cocokkan SHA-256 dan ukuran, lalu jalur
/// pasang yang sama dengan unggah (tanda tangan paket, kompatibilitas, migrasi). Berhenti di langkah yang gagal;
/// yang sudah terpasang sebelumnya tetap (tiap paket sah berdiri sendiri).
async fn store_install<C: Connector>(State(st): State<St<C>>, c: Caller, Json(req): Json<StoreInstallReq>) -> ApiResult<Json<Value>> {
    manage(&st, &c, Level::Update)?;
    let (url, f) = store_deps(&st)?;
    if !valid_id(&req.id) {
        return Err(ApiError::not_found());
    }
    let official = cfg(&st)?.keys.official.clone();
    let installed = versions(&all_rows(&st)?);
    let host = st.cfg.host_version.clone();
    let id = req.id.clone();
    // Unduh semuanya dulu (di thread blokir), pasang sesudahnya.
    let downloads: Vec<Vec<u8>> = tokio::task::spawn_blocking(move || -> Result<Vec<Vec<u8>>, (StatusCode, &'static str, String)> {
        let cat = fetch_catalog(&url, f.as_ref(), &official).map_err(|m| (StatusCode::BAD_GATEWAY, "store_unreachable", m))?;
        if cat.find(&id).is_none() {
            return Err((StatusCode::NOT_FOUND, "not_found", "Extension tidak ada di katalog".into()));
        }
        let plan = cat.plan(&id, &host, &installed).map_err(|p| (StatusCode::CONFLICT, "incompatible", problem_text(&p)))?;
        let mut out = Vec::new();
        for e in plan {
            let bytes = f.get(&package_url(&url, &e.file), e.size as usize + 1024).map_err(|m| (StatusCode::BAD_GATEWAY, "store_unreachable", m))?;
            e.check_download(&bytes).map_err(|m| (StatusCode::BAD_GATEWAY, "store_corrupt", format!("{}: {m}", e.manifest.id)))?;
            out.push(bytes);
        }
        Ok(out)
    })
    .await
    .map_err(ApiError::internal)?
    .map_err(|(s, code, m)| ApiError::new(s, code, m))?;
    let mut done = Vec::new();
    for bytes in &downloads {
        done.push(do_install(&st, &c, bytes)?);
    }
    Ok(Json(json!({ "installed": done })))
}

// ---------------- akses per peran (ADR 0028) ----------------

pub(crate) fn lvl(s: &str) -> u8 {
    match s {
        "update" => 2,
        "read" => 1,
        _ => 0,
    }
}

pub(crate) fn lvl_name(n: u8) -> &'static str {
    match n {
        2 => "update",
        1 => "read",
        _ => "none",
    }
}

/// Akses efektif pemanggil pada satu extension: pemilik desktop = penuh; akun = baris perannya (tanpa baris =
/// tanpa akses). Aksi efektif = aksi yang diberikan dan dideklarasikan, yang level minimalnya tidak melebihi level peran.
pub(crate) fn effective<C: Connector>(st: &St<C>, c: &Caller, id: &str, m: &Manifest) -> (u8, Vec<String>) {
    if c.desktop {
        return (2, m.actions.iter().map(|a| a.id.clone()).collect());
    }
    let rows = st.telemon.with_store(|s| s.ext_access_list(id)).unwrap_or_default();
    let Some((_, level, granted)) = rows.into_iter().find(|(r, _, _)| *r == c.session.role_id) else { return (0, vec![]) };
    let l = lvl(&level);
    let actions = m.actions.iter().filter(|a| granted.contains(&a.id) && lvl(&a.level) <= l).map(|a| a.id.clone()).collect();
    (l, actions)
}

pub(crate) fn manifest_of(row: &StoredExtension) -> ApiResult<Manifest> {
    serde_json::from_str(&row.manifest).map_err(ApiError::internal)
}

async fn access_get<C: Connector>(State(st): State<St<C>>, c: Caller, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    manage(&st, &c, Level::Read)?;
    cfg(&st)?;
    let row = st.telemon.with_store(|s| s.ext_get(&id)).map_err(ApiError::internal)?.ok_or_else(ApiError::not_found)?;
    let m = manifest_of(&row)?;
    let rows = st.telemon.with_store(|s| s.ext_access_list(&id)).map_err(ApiError::internal)?;
    let roles = st.auth.list_roles(Some(&c.session)).map_err(ApiError::from)?;
    let out: Vec<Value> = roles
        .iter()
        .map(|r| {
            let (level, actions) = rows.iter().find(|(rid, _, _)| *rid == r.id).map(|(_, l, a)| (l.clone(), a.clone())).unwrap_or(("none".into(), vec![]));
            json!({ "id": r.id, "name": r.name, "builtin": r.builtin, "level": level, "actions": actions, "own": r.id == c.session.role_id && !c.desktop })
        })
        .collect();
    let (my_level, my_actions) = effective(&st, &c, &id, &m);
    Ok(Json(json!({ "actions": m.actions, "roles": out, "me": { "level": lvl_name(my_level), "actions": my_actions } })))
}

#[derive(Deserialize)]
struct AccessReq {
    level: String,
    #[serde(default)]
    actions: Vec<String>,
}

/// Aturan anti-kenaikan izin (ADR 0006) berlaku: tidak mengubah peran sendiri, tidak memberi lebih daripada
/// yang dimiliki sendiri, tidak mengelola peran yang aksesnya sudah lebih tinggi daripada milik sendiri.
async fn access_put<C: Connector>(State(st): State<St<C>>, c: Caller, Path((id, role)): Path<(String, String)>, Json(req): Json<AccessReq>) -> ApiResult<Json<Value>> {
    manage(&st, &c, Level::Update)?;
    cfg(&st)?;
    let row = st.telemon.with_store(|s| s.ext_get(&id)).map_err(ApiError::internal)?.ok_or_else(ApiError::not_found)?;
    let m = manifest_of(&row)?;
    if !matches!(req.level.as_str(), "none" | "read" | "update") {
        return Err(ApiError::bad("Level harus none, read, atau update"));
    }
    let want = lvl(&req.level);
    let roles = st.auth.list_roles(Some(&c.session)).map_err(ApiError::from)?;
    if !roles.iter().any(|r| r.id == role) {
        return Err(ApiError::not_found());
    }
    let mut actions: Vec<String> = Vec::new();
    for a in &req.actions {
        let decl = m.actions.iter().find(|d| d.id == *a).ok_or_else(|| ApiError::bad(format!("Aksi tidak dikenal: {a}")))?;
        if lvl(&decl.level) > want {
            return Err(ApiError::bad(format!("Aksi {a} butuh level {}", decl.level)));
        }
        if !actions.contains(a) {
            actions.push(a.clone());
        }
    }
    if want == 0 {
        actions.clear();
    }
    let rows = st.telemon.with_store(|s| s.ext_access_list(&id)).map_err(ApiError::internal)?;
    let (cur_level, cur_actions) = rows.iter().find(|(r, _, _)| *r == role).map(|(_, l, a)| (lvl(l), a.clone())).unwrap_or((0, vec![]));
    if !c.desktop {
        let (mine, my_actions) = effective(&st, &c, &id, &m);
        if role == c.session.role_id {
            return Err(ApiError::forbidden("Tidak boleh mengubah akses peran Anda sendiri"));
        }
        if want > mine || actions.iter().any(|a| !my_actions.contains(a)) {
            return Err(ApiError::forbidden("Tidak boleh memberi akses yang melebihi akses Anda sendiri"));
        }
        if cur_level > mine {
            return Err(ApiError::forbidden("Tidak boleh mengelola peran yang aksesnya lebih tinggi daripada Anda"));
        }
    }
    st.telemon.with_store(|s| s.ext_access_set(&id, &role, &req.level, &actions)).map_err(ApiError::internal)?;
    let name = roles.iter().find(|r| r.id == role).map(|r| r.name.clone()).unwrap_or_default();
    audit(
        &st,
        &c,
        "extension.access",
        &id,
        format!("Ubah akses peran {name} pada extension {}", m.name),
        vec![
            Change { field: "level".into(), from: Some(lvl_name(cur_level).into()), to: Some(req.level.clone()) },
            Change { field: "aksi".into(), from: Some(cur_actions.join(",")), to: Some(actions.join(",")) },
        ],
    );
    Ok(Json(json!({ "role": role, "level": req.level, "actions": actions })))
}

/// Terapkan migrasi database paket yang belum terapan (ADR 0028). Urutan: periksa rencana (migrasi yang sudah
/// terapan tidak boleh berubah, data tidak boleh lebih baru daripada paket), cadangkan basis data, lalu jalankan
/// semuanya dalam satu transaksi. Gagal = tidak ada perubahan dan pemasangan dibatalkan.
fn run_migrations<C: Connector>(st: &St<C>, dir: &std::path::Path, p: &Package, id: &str, now: i64) -> ApiResult<()> {
    let all = parse_migrations(id, &p.files).map_err(|e| ApiError::new(StatusCode::CONFLICT, "migration_conflict", e))?;
    let applied = st.telemon.with_store(|s| s.ext_applied_migrations(id)).map_err(ApiError::internal)?;
    let pending = plan_migrations(&applied, &all).map_err(|e| ApiError::new(StatusCode::CONFLICT, "migration_conflict", e))?;
    if pending.is_empty() {
        return Ok(());
    }
    let items: Vec<(u32, String, String, Vec<String>)> = pending.iter().map(|m| (m.version, m.name.clone(), m.checksum.clone(), migration_sql(m, Dialect::Sqlite))).collect();
    let backups = dir.join("backups");
    std::fs::create_dir_all(&backups).map_err(ApiError::internal)?;
    let dest = backups.join(format!("before-{id}-{}-{now}.db", p.manifest.version));
    st.telemon
        .with_store(|s| -> Result<(), String> {
            e(s.backup_to(&dest))?;
            e(s.ext_apply_migrations(id, now, &items))
        })
        .map_err(|m| ApiError::new(StatusCode::UNPROCESSABLE_ENTITY, "migration_failed", format!("Migrasi database gagal dan dibatalkan (cadangan: {}): {m}", dest.display())))
}

/// Status: `disabled` (mati), `limited` (izin wajib dimatikan), `ok`.
pub(crate) fn status(e: &StoredExtension, m: &Manifest, perms: &[(String, bool)], blocked: &[String], host: &str) -> &'static str {
    if !e.enabled {
        return "disabled";
    }
    // Telemon diperbarui ke versi di luar rentang yang dinyatakan paket: tidak dimuat.
    if !host_compatible(m, host) {
        return "incompatible";
    }
    if !blocked.is_empty() {
        return "limited";
    }
    let granted = |id: &str| perms.iter().any(|(k, g)| k == id && *g);
    if m.permissions.iter().any(|d| d.required && !granted(&d.id)) { "limited" } else { "ok" }
}

fn item<C: Connector>(st: &St<C>, e: &StoredExtension, perms: &[(String, bool)], all: &[StoredExtension], host: &str, migrated: usize) -> Value {
    let m: Option<Manifest> = serde_json::from_str(&e.manifest).ok();
    let Some(m) = m else {
        return json!({ "id": e.id, "version": e.version, "tier": e.tier, "enabled": e.enabled, "status": "broken" });
    };
    let permissions: Vec<Value> = m.permissions.iter().map(|d| json!({ "id": d.id, "required": d.required, "sensitive": d.sensitive, "description": d.description, "granted": perms.iter().any(|(k, g)| *k == d.id && *g) })).collect();
    let blocked = blocked_by(&m, all);
    let others: Vec<StoredExtension> = all.iter().filter(|r| r.id != e.id).cloned().collect();
    let dependents: Vec<String> = dependents_of(&e.id, &manifests(all)).iter().map(|d| d.id.clone()).collect();
    json!({
        "id": e.id, "name": m.name, "version": e.version, "publisher": m.publisher, "description": m.description,
        "tier": e.tier, "signed": e.signed, "enabled": e.enabled, "status": status(e, &m, perms, &blocked, host),
        "hasData": migrated > 0, "telemon": m.telemon, "requires": dep_status(&m, &versions(&others)), "blockedBy": blocked, "dependents": dependents,
        "widgets": m.contributes.widgets, "pages": m.contributes.pages, "actions": m.actions, "routes": m.contributes.routes, "hasBackend": m.backend.is_some(), "workers": crate::extbackend::worker_view(st, &m), "permissions": permissions, "installedAt": e.installed_at, "updatedAt": e.updated_at,
    })
}

fn view<C: Connector>(st: &St<C>, id: &str) -> ApiResult<Value> {
    let (e, perms) = st
        .telemon
        .with_store(|s| -> Result<_, String> { Ok((e(s.ext_get(id))?, e(s.ext_perms(id))?)) })
        .map_err(ApiError::internal)?;
    let e = e.ok_or_else(ApiError::not_found)?;
    let all = all_rows(st)?;
    let migrated = st.telemon.with_store(|s| s.ext_applied_migrations(id)).map_err(ApiError::internal)?.len();
    Ok(item(st, &e, &perms, &all, &st.cfg.host_version, migrated))
}

async fn list<C: Connector>(State(st): State<St<C>>, c: Caller) -> ApiResult<Json<Value>> {
    manage(&st, &c, Level::Read)?;
    let Some(_) = st.cfg.extensions.as_ref() else {
        return Ok(Json(json!({ "available": false, "extensions": [] })));
    };
    let rows = st
        .telemon
        .with_store(|s| -> Result<_, String> {
            let mut out = Vec::new();
            for x in e(s.ext_list())? {
                let p = e(s.ext_perms(&x.id))?;
                out.push((x, p));
            }
            Ok(out)
        })
        .map_err(ApiError::internal)?;
    let all: Vec<StoredExtension> = rows.iter().map(|(e, _)| e.clone()).collect();
    let extensions: Vec<Value> = rows
        .iter()
        .map(|(e, p)| {
            let migrated = st.telemon.with_store(|s| s.ext_applied_migrations(&e.id)).map(|v| v.len()).unwrap_or(0);
            item(&st, e, p, &all, &st.cfg.host_version, migrated)
        })
        .collect();
    Ok(Json(json!({ "available": true, "extensions": extensions })))
}

/// Untuk pemuat UI: hanya yang aktif dan official, dengan izin yang diberikan.
async fn active<C: Connector>(State(st): State<St<C>>, c: Caller) -> ApiResult<Json<Value>> {
    if st.cfg.extensions.is_none() {
        return Ok(Json(json!({ "extensions": [] })));
    }
    let rows = st
        .telemon
        .with_store(|s| -> Result<_, String> {
            let mut out = Vec::new();
            for x in e(s.ext_list())?.into_iter().filter(|x| x.enabled && x.tier == "official") {
                let p = e(s.ext_perms(&x.id))?;
                out.push((x, p));
            }
            Ok(out)
        })
        .map_err(ApiError::internal)?;
    let all = all_rows(&st)?;
    let mut out = Vec::new();
    for (ex, perms) in rows {
        let Ok(m) = serde_json::from_str::<Manifest>(&ex.manifest) else { continue };
        if matches!(status(&ex, &m, &perms, &blocked_by(&m, &all), &st.cfg.host_version), "limited" | "incompatible") {
            continue;
        }
        let Some(ui) = &m.ui else { continue };
        let (level, actions) = effective(&st, &c, &ex.id, &m);
        if level == 0 {
            continue;
        }
        let granted: Vec<&str> = perms.iter().filter(|(_, g)| *g).map(|(k, _)| k.as_str()).collect();
        out.push(json!({ "id": ex.id, "name": m.name, "version": ex.version, "entry": format!("/ext/{}/{}?v={}", ex.id, ui.entry, ex.version), "widgets": m.contributes.widgets, "pages": m.contributes.pages, "permissions": granted, "access": { "level": lvl_name(level), "actions": actions } }));
    }
    Ok(Json(json!({ "extensions": out })))
}

#[derive(Deserialize)]
struct PatchReq {
    enabled: Option<bool>,
    /// Izin → nyala/mati. Hanya izin yang dideklarasikan manifest.
    perms: Option<std::collections::BTreeMap<String, bool>>,
}

async fn patch<C: Connector>(State(st): State<St<C>>, c: Caller, Path(id): Path<String>, Json(req): Json<PatchReq>) -> ApiResult<Json<Value>> {
    manage(&st, &c, Level::Update)?;
    cfg(&st)?;
    let cur = st.telemon.with_store(|s| s.ext_get(&id)).map_err(ApiError::internal)?.ok_or_else(ApiError::not_found)?;
    let m: Manifest = serde_json::from_str(&cur.manifest).map_err(ApiError::internal)?;
    let now = st.now();
    let mut changes = Vec::new();
    if let Some(perms) = &req.perms {
        let before = st.telemon.with_store(|s| s.ext_perms(&id)).map_err(ApiError::internal)?;
        for (k, v) in perms {
            if !m.permissions.iter().any(|d| d.id == *k) {
                return Err(ApiError::bad(format!("Izin tidak dikenal: {k}")));
            }
            let was = before.iter().any(|(p, g)| p == k && *g);
            if was != *v {
                changes.push(Change { field: format!("izin {k}"), from: Some(was.to_string()), to: Some(v.to_string()) });
            }
        }
        st.telemon
            .with_store(|s| -> Result<(), String> {
                for (k, v) in perms {
                    e(s.ext_set_perm(&id, k, *v))?;
                }
                Ok(())
            })
            .map_err(ApiError::internal)?;
    }
    if let Some(en) = req.enabled {
        if en != cur.enabled {
            changes.push(Change { field: "aktif".into(), from: Some(cur.enabled.to_string()), to: Some(en.to_string()) });
        }
        st.telemon.with_store(|s| s.ext_set_enabled(&id, en, now)).map_err(ApiError::internal)?;
    }
    if !changes.is_empty() {
        audit(&st, &c, "extension.toggle", &id, format!("Ubah akses extension {}", m.name), changes);
    }
    Ok(Json(view(&st, &id)?))
}

#[derive(Deserialize)]
struct RemoveQ {
    /// `delete` = hapus juga tabel dan catatan migrasi extension; selain itu data disimpan (bawaan).
    data: Option<String>,
}

async fn remove<C: Connector>(State(st): State<St<C>>, c: Caller, Path(id): Path<String>, Query(q): Query<RemoveQ>) -> ApiResult<Json<Value>> {
    manage(&st, &c, Level::Update)?;
    let dir = cfg(&st)?.dir.clone();
    if !valid_id(&id) {
        return Err(ApiError::not_found());
    }
    let rows = all_rows(&st)?;
    let cur = rows.iter().find(|r| r.id == id).cloned().ok_or_else(ApiError::not_found)?;
    let dep: Vec<String> = dependents_of(&id, &manifests(&rows)).iter().map(|d| d.id.clone()).collect();
    if !dep.is_empty() {
        return Err(ApiError::new(StatusCode::CONFLICT, "has_dependents", format!("Masih dibutuhkan oleh: {}. Hapus extension itu dulu.", dep.join(", "))));
    }
    let drop_data = q.data.as_deref() == Some("delete");
    let dropped = st
        .telemon
        .with_store(|s| -> Result<usize, String> {
            e(s.ext_delete(&id))?;
            if drop_data { e(s.ext_drop_data(&id, &namespace(&id))) } else { Ok(0) }
        })
        .map_err(ApiError::internal)?;
    let _ = std::fs::remove_file(path_of(&dir, &id));
    st.ext_cache.lock().expect("lock").remove(&id);
    st.ext_wasm.lock().expect("lock").remove(&id);
    st.ext_workers.lock().expect("lock").retain(|k, _| !k.starts_with(&format!("{id}/")));
    let mut changes = vec![Change { field: "versi".into(), from: Some(cur.version), to: None }];
    if drop_data {
        changes.push(Change { field: "data".into(), from: Some(format!("{dropped} tabel")), to: None });
    }
    audit(&st, &c, "extension.uninstall", &id, format!("Hapus extension {}", id), changes);
    Ok(Json(json!({ "removed": true, "dataDeleted": drop_data, "tablesDropped": dropped })))
}

/// Muat paket dari disk dan verifikasi ulang (cache per versi).
fn load<C: Connector>(st: &St<C>, id: &str, version: &str) -> Option<Arc<Package>> {
    if let Some((v, p)) = st.ext_cache.lock().expect("lock").get(id)
        && v == version
    {
        return Some(p.clone());
    }
    let cfg = st.cfg.extensions.as_ref()?;
    let bytes = std::fs::read(path_of(&cfg.dir, id)).ok()?;
    let p = Package::open(&bytes, &cfg.keys, &Limits::default()).ok()?;
    if p.tier != Tier::Official || p.manifest.id != id || p.manifest.version != version {
        return None;
    }
    let p = Arc::new(p);
    st.ext_cache.lock().expect("lock").insert(id.to_string(), (version.to_string(), p.clone()));
    Some(p)
}

fn mime(path: &str) -> &'static str {
    match path.rsplit('.').next().unwrap_or("") {
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "woff2" => "font/woff2",
        "txt" | "md" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

async fn file<C: Connector>(State(st): State<St<C>>, c: Caller, Path((id, path)): Path<(String, String)>) -> Result<Response, ApiError> {
    if st.cfg.extensions.is_none() || !valid_id(&id) || !path.starts_with("ui/") {
        return Err(ApiError::not_found());
    }
    let row = st.telemon.with_store(|s| s.ext_get(&id)).map_err(ApiError::internal)?;
    let Some(e) = row.filter(|e| e.enabled && e.tier == "official") else { return Err(ApiError::not_found()) };
    let m = manifest_of(&e)?;
    if effective(&st, &c, &id, &m).0 == 0 {
        return Err(ApiError::not_found());
    }
    let (st2, id2, v2) = (st.clone(), id.clone(), e.version.clone());
    let pkg = tokio::task::spawn_blocking(move || load(&st2, &id2, &v2)).await.map_err(ApiError::internal)?.ok_or_else(ApiError::not_found)?;
    let bytes = pkg.file(&path).ok_or_else(ApiError::not_found)?.to_vec();
    let mut res = Response::new(Body::from(bytes));
    res.headers_mut().insert(header::CONTENT_TYPE, HeaderValue::from_static(mime(&path)));
    res.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    Ok(res)
}
