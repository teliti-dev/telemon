//! Rute API JSON. Semua izin ditegakkan di sini lewat `Auth::authorize`; UI hanya menyembunyikan.

use std::collections::BTreeMap;
use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, patch, post, put};
use axum::{Json, Router};
use serde_json::{Value, json};
use telemon_core::auth::{Auth, AuthError, Level, LoginOutcome, NewAccount, Resource, RoleInput};
use telemon_core::service::Probe;
use telemon_core::store::{AuditQuery, EventQuery, NewRouter, RouterPatch, UserMeta};
use telemon_core::write::{NewProfile, NewUser, ProfilePatch, UserPatch};
use telemon_core::{AuditEntry, AuditOutcome, Change, Connector, EventKind, TlsMode};

use crate::dto::*;
use crate::error::{ApiError, ApiResult};
use crate::guard::{Caller, Client, DESKTOP_COOKIE, Surface, clear_cookie, ct_eq, is_desktop, session_cookie};
use crate::state::{AppState, PendingPlan};

type St<C> = State<Arc<AppState<C>>>;

pub fn routes<C: Connector>() -> Router<Arc<AppState<C>>> {
    Router::new()
        .route("/api/health", get(health::<C>))
        // masuk
        .route("/api/login", post(login::<C>))
        .route("/api/login/2fa", post(login_2fa::<C>))
        .route("/api/login/2fa-setup", post(login_2fa_setup::<C>))
        .route("/api/logout", post(logout::<C>))
        .route("/api/session", get(session::<C>))
        // akun sendiri
        .route("/api/me/profile", patch(me_profile::<C>))
        .route("/api/me/password", post(me_password::<C>))
        .route("/api/me/2fa/start", post(me_2fa_start::<C>))
        .route("/api/me/2fa/confirm", post(me_2fa_confirm::<C>))
        .route("/api/me/2fa/disable", post(me_2fa_disable::<C>))
        .route("/api/me/recovery-codes", post(me_recovery::<C>))
        .route("/api/me/sessions", get(me_sessions::<C>))
        .route("/api/me/sessions/{id}", delete(me_session_revoke::<C>))
        // admin
        .route("/api/accounts", get(accounts_list::<C>).post(accounts_create::<C>))
        .route("/api/accounts/{id}/password", post(account_password::<C>))
        .route("/api/accounts/{id}/disabled", post(account_disabled::<C>))
        .route("/api/accounts/{id}/reset-2fa", post(account_reset_2fa::<C>))
        .route("/api/accounts/{id}/role", post(account_role::<C>))
        .route("/api/accounts/{id}", delete(account_delete::<C>))
        .route("/api/roles", get(roles_list::<C>).post(roles_create::<C>))
        .route("/api/roles/{id}", put(roles_update::<C>).delete(roles_delete::<C>))
        .route("/api/roles/{id}/reset", post(roles_reset::<C>))
        // berbagi (hanya aplikasi desktop)
        .route("/api/share", get(share_get::<C>))
        .route("/api/share/lan", post(share_lan::<C>))
        .route("/api/share/publik", post(share_publik::<C>))
        .route("/api/share/require-2fa", post(share_require_2fa::<C>))
        // pembaruan aplikasi (hanya aplikasi desktop)
        .route("/api/update", get(update_get::<C>))
        .route("/api/update/check", post(update_check::<C>))
        .route("/api/update/install", post(update_install::<C>))
        .route("/api/update/channel", post(update_channel::<C>))
        // pengaturan dan penyimpanan
        .merge(crate::ai::routes::<C>())
        .merge(crate::dashboard::routes::<C>())
        .merge(crate::traffic::routes::<C>())
        .route("/api/settings", get(settings_get::<C>).put(settings_put::<C>))
        .route("/api/storage", get(storage_get::<C>))
        .route("/api/storage/purge", post(storage_purge::<C>))
        // data
        .route("/api/snapshot", get(snapshot::<C>))
        .route("/api/events", get(events::<C>))
        .route("/api/audit", get(audit::<C>))
        .route("/api/users/{id}/password", get(user_password::<C>))
        .route("/api/users/{id}/meta", put(user_meta::<C>))
        // router dan penulisan
        .route("/api/routers", post(router_add::<C>))
        .route("/api/routers/probe", post(router_probe::<C>))
        .route("/api/routers/fingerprint", post(router_fingerprint::<C>))
        .route("/api/routers/{id}", patch(router_edit::<C>).delete(router_delete::<C>))
        .route("/api/routers/{id}/import", post(router_import::<C>))
        .route("/api/routers/{id}/plans", post(plan_create::<C>))
        .route("/api/plans/{id}/apply", post(plan_apply::<C>))
        .route("/api/profiles/{id}/soft-delete", post(profile_soft_delete::<C>))
        .route("/api/profiles/{id}/restore", post(profile_restore::<C>))
}

// ---------------- pembantu ----------------

async fn blocking<C, T, F>(st: &Arc<AppState<C>>, f: F) -> ApiResult<T>
where
    C: Connector,
    F: FnOnce(&Auth) -> Result<T, AuthError> + Send + 'static,
    T: Send + 'static,
{
    let auth = st.auth.clone();
    tokio::task::spawn_blocking(move || f(&auth)).await.map_err(ApiError::internal)?.map_err(Into::into)
}

fn need<C: Connector>(st: &AppState<C>, c: &Caller, r: Resource, l: Level) -> ApiResult<()> {
    st.auth.authorize(Some(&c.session), r, l).map_err(Into::into)
}

fn device(h: &HeaderMap) -> String {
    h.get(header::USER_AGENT).and_then(|v| v.to_str().ok()).unwrap_or("").chars().take(120).collect()
}

fn with_cookie(mut r: Response, token: &str, st_hours: u32, secure: bool) -> Response {
    r.headers_mut().append(header::SET_COOKIE, session_cookie(token, st_hours * 3600, secure));
    r
}

fn secure<C: Connector>(st: &AppState<C>, via: telemon_core::auth::Via) -> bool {
    st.cfg.force_secure_cookie || via == telemon_core::auth::Via::Publik
}

fn limit<C: Connector>(st: &AppState<C>, ip: &str) -> ApiResult<()> {
    st.limiter.check(ip, st.now()).map_err(ApiError::too_many)
}

// ---------------- masuk ----------------

async fn login<C: Connector>(State(st): St<C>, Client(info): Client, headers: HeaderMap, Json(req): Json<LoginReq>) -> ApiResult<Response> {
    limit(&st, &info.ip)?;
    let (via, ip, dev) = (info.via, info.ip.clone(), device(&headers));
    let out = blocking(&st, move |a| a.login(&req.username, &req.password, via, &dev, &ip)).await?;
    Ok(match out {
        LoginOutcome::Ok { token, session } => {
            let hours = st.auth.policy().session_hours;
            with_cookie(Json(json!({ "status": "ok", "session": SessionDto::from(&session) })).into_response(), &token, hours, secure(&st, info.via))
        }
        LoginOutcome::TwoFactorRequired { challenge } => Json(json!({ "status": "2fa_required", "challengeId": challenge })).into_response(),
        LoginOutcome::TwoFactorSetupRequired { challenge, secret, otpauth_url } => {
            Json(json!({ "status": "2fa_setup_required", "challengeId": challenge, "secret": secret, "otpauthUrl": otpauth_url })).into_response()
        }
    })
}

async fn login_2fa<C: Connector>(State(st): St<C>, Client(info): Client, Json(req): Json<TwoFaReq>) -> ApiResult<Response> {
    limit(&st, &info.ip)?;
    let a = blocking(&st, move |a| a.verify_2fa(&req.challenge_id, &req.code, req.recovery)).await?;
    let hours = st.auth.policy().session_hours;
    Ok(with_cookie(Json(json!({ "status": "ok", "session": SessionDto::from(&a.session) })).into_response(), &a.token, hours, secure(&st, a.session.via)))
}

async fn login_2fa_setup<C: Connector>(State(st): St<C>, Client(info): Client, Json(req): Json<TwoFaReq>) -> ApiResult<Response> {
    limit(&st, &info.ip)?;
    let (a, codes) = blocking(&st, move |a| a.complete_2fa_setup(&req.challenge_id, &req.code)).await?;
    let hours = st.auth.policy().session_hours;
    Ok(with_cookie(Json(json!({ "session": SessionDto::from(&a.session), "recoveryCodes": codes })).into_response(), &a.token, hours, secure(&st, a.session.via)))
}

async fn logout<C: Connector>(State(st): St<C>, c: Caller) -> ApiResult<Response> {
    let t = c.token.clone();
    blocking(&st, move |a| a.logout(&t)).await?;
    let mut r = StatusCode::NO_CONTENT.into_response();
    r.headers_mut().append(header::SET_COOKIE, clear_cookie(secure(&st, c.info.via)));
    Ok(r)
}

async fn session<C: Connector>(c: Caller) -> Json<SessionDto> {
    Json(SessionDto::from(&c.session))
}

// ---------------- akun sendiri ----------------

async fn me_profile<C: Connector>(State(st): St<C>, c: Caller, Json(r): Json<ProfileReq>) -> ApiResult<Json<SessionDto>> {
    let s = c.session.clone();
    let out = blocking(&st, move |a| a.update_my_profile(&s, &r.display_name)).await?;
    Ok(Json(SessionDto::from(&out)))
}

async fn me_password<C: Connector>(State(st): St<C>, c: Caller, Json(r): Json<ChangePasswordReq>) -> ApiResult<StatusCode> {
    let s = c.session.clone();
    blocking(&st, move |a| a.change_my_password(&s, &r.current, &r.next)).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn me_2fa_start<C: Connector>(State(st): St<C>, c: Caller) -> ApiResult<Json<Value>> {
    let s = c.session.clone();
    let (secret, url) = blocking(&st, move |a| a.start_2fa_setup(&s)).await?;
    Ok(Json(json!({ "secret": secret, "otpauthUrl": url })))
}

async fn me_2fa_confirm<C: Connector>(State(st): St<C>, c: Caller, Json(r): Json<CodeReq>) -> ApiResult<Json<Value>> {
    let s = c.session.clone();
    let codes = blocking(&st, move |a| a.confirm_2fa_setup(&s, &r.code)).await?;
    Ok(Json(json!({ "recoveryCodes": codes })))
}

async fn me_2fa_disable<C: Connector>(State(st): St<C>, c: Caller, Json(r): Json<PasswordReq>) -> ApiResult<StatusCode> {
    let s = c.session.clone();
    blocking(&st, move |a| a.disable_2fa(&s, &r.password)).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn me_recovery<C: Connector>(State(st): St<C>, c: Caller, Json(r): Json<PasswordReq>) -> ApiResult<Json<Value>> {
    let s = c.session.clone();
    let codes = blocking(&st, move |a| a.regenerate_recovery_codes(&s, &r.password)).await?;
    Ok(Json(json!({ "recoveryCodes": codes })))
}

async fn me_sessions<C: Connector>(State(st): St<C>, c: Caller) -> ApiResult<Json<Vec<DeviceDto>>> {
    let s = c.session.clone();
    let list = blocking(&st, move |a| a.list_my_sessions(&s)).await?;
    Ok(Json(list.iter().map(DeviceDto::from).collect()))
}

async fn me_session_revoke<C: Connector>(State(st): St<C>, c: Caller, Path(id): Path<String>) -> ApiResult<StatusCode> {
    let s = c.session.clone();
    blocking(&st, move |a| a.revoke_my_session(&s, &id)).await?;
    Ok(StatusCode::NO_CONTENT)
}

// ---------------- admin ----------------

async fn accounts_list<C: Connector>(State(st): St<C>, c: Caller) -> ApiResult<Json<Vec<AccountDto>>> {
    let s = c.session.clone();
    let l = blocking(&st, move |a| a.list_accounts(Some(&s))).await?;
    Ok(Json(l.iter().map(AccountDto::from).collect()))
}

async fn accounts_create<C: Connector>(State(st): St<C>, c: Caller, Json(r): Json<NewAccountReq>) -> ApiResult<(StatusCode, Json<AccountDto>)> {
    let s = c.session.clone();
    let n = NewAccount { username: r.username, display_name: r.display_name, password: r.password, role_id: r.role_id };
    let a = blocking(&st, move |a| a.create_account(Some(&s), &n)).await?;
    Ok((StatusCode::CREATED, Json(AccountDto::from(&a))))
}

async fn account_password<C: Connector>(State(st): St<C>, c: Caller, Path(id): Path<String>, Json(r): Json<PasswordReq>) -> ApiResult<StatusCode> {
    let s = c.session.clone();
    blocking(&st, move |a| a.reset_account_password(Some(&s), &id, &r.password)).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn account_disabled<C: Connector>(State(st): St<C>, c: Caller, Path(id): Path<String>, Json(r): Json<DisabledReq>) -> ApiResult<Json<AccountDto>> {
    let s = c.session.clone();
    let a = blocking(&st, move |a| a.set_account_disabled(Some(&s), &id, r.disabled)).await?;
    Ok(Json(AccountDto::from(&a)))
}

async fn account_reset_2fa<C: Connector>(State(st): St<C>, c: Caller, Path(id): Path<String>) -> ApiResult<Json<AccountDto>> {
    let s = c.session.clone();
    let a = blocking(&st, move |a| a.reset_account_2fa(Some(&s), &id)).await?;
    Ok(Json(AccountDto::from(&a)))
}

async fn account_role<C: Connector>(State(st): St<C>, c: Caller, Path(id): Path<String>, Json(r): Json<RoleIdReq>) -> ApiResult<Json<AccountDto>> {
    let s = c.session.clone();
    let a = blocking(&st, move |a| a.change_account_role(Some(&s), &id, &r.role_id)).await?;
    Ok(Json(AccountDto::from(&a)))
}

async fn account_delete<C: Connector>(State(st): St<C>, c: Caller, Path(id): Path<String>) -> ApiResult<StatusCode> {
    let s = c.session.clone();
    blocking(&st, move |a| a.delete_account(Some(&s), &id)).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn roles_list<C: Connector>(State(st): St<C>, c: Caller) -> ApiResult<Json<Vec<RoleDto>>> {
    let s = c.session.clone();
    let l = blocking(&st, move |a| a.list_roles(Some(&s))).await?;
    Ok(Json(l.iter().map(RoleDto::from).collect()))
}

fn role_input(r: &RoleReq) -> ApiResult<RoleInput> {
    Ok(RoleInput { name: r.name.clone(), description: r.description.clone(), permissions: r.permissions().map_err(ApiError::bad)? })
}

async fn roles_create<C: Connector>(State(st): St<C>, c: Caller, Json(r): Json<RoleReq>) -> ApiResult<(StatusCode, Json<RoleDto>)> {
    let (s, input) = (c.session.clone(), role_input(&r)?);
    let role = blocking(&st, move |a| a.create_role(Some(&s), &input)).await?;
    Ok((StatusCode::CREATED, Json(RoleDto::from(&role))))
}

async fn roles_update<C: Connector>(State(st): St<C>, c: Caller, Path(id): Path<String>, Json(r): Json<RoleReq>) -> ApiResult<Json<RoleDto>> {
    let (s, input) = (c.session.clone(), role_input(&r)?);
    let role = blocking(&st, move |a| a.update_role(Some(&s), &id, &input)).await?;
    Ok(Json(RoleDto::from(&role)))
}

async fn roles_delete<C: Connector>(State(st): St<C>, c: Caller, Path(id): Path<String>) -> ApiResult<StatusCode> {
    let s = c.session.clone();
    blocking(&st, move |a| a.delete_role(Some(&s), &id)).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn roles_reset<C: Connector>(State(st): St<C>, c: Caller, Path(id): Path<String>) -> ApiResult<Json<RoleDto>> {
    let s = c.session.clone();
    let role = blocking(&st, move |a| a.reset_builtin_role(Some(&s), &id)).await?;
    Ok(Json(RoleDto::from(&role)))
}

// ---------------- data ----------------

fn can(c: &Caller, r: Resource) -> bool {
    c.session.permissions.get(r) >= Level::Read
}

/// Foto keadaan sesuai izin: bagian yang tidak boleh dibaca dikirim kosong. Password tidak pernah ikut.
async fn snapshot<C: Connector>(State(st): St<C>, c: Caller) -> ApiResult<Json<Value>> {
    let t = &st.telemon;
    let db = |e: rusqlite_err::E| ApiError::internal(e.0);
    let routers = if can(&c, Resource::Router) { t.with_store(|s| s.list_routers().map_err(rusqlite_err::wrap)).map_err(db)? } else { vec![] };
    let (users, metas) = if can(&c, Resource::User) {
        let u = t.with_store(|s| s.list_users(None, true).map_err(rusqlite_err::wrap)).map_err(db)?;
        let m = t.with_store(|s| s.list_user_metas().map_err(rusqlite_err::wrap)).map_err(db)?;
        (u, m)
    } else {
        (vec![], vec![])
    };
    let profiles = if can(&c, Resource::Profile) { t.with_store(|s| s.list_profiles(None, true).map_err(rusqlite_err::wrap)).map_err(db)? } else { vec![] };
    let events = if can(&c, Resource::Event) { t.with_store(|s| s.list_events(&EventQuery { limit: 200, ..Default::default() }).map_err(rusqlite_err::wrap)).map_err(db)? } else { vec![] };
    let metas: BTreeMap<String, Value> = metas
        .into_iter()
        .map(|(id, m)| (id, json!({ "customerName": m.customer_name, "phone": m.phone, "note": m.note, "tags": m.tags, "updatedAt": m.updated_at })))
        .collect();
    Ok(Json(json!({
        "routers": routers.iter().map(RouterDto::from).collect::<Vec<_>>(),
        "users": users.iter().map(UserDto::from).collect::<Vec<_>>(),
        "profiles": profiles.iter().map(ProfileDto::from).collect::<Vec<_>>(),
        "events": events.iter().map(EventDto::from).collect::<Vec<_>>(),
        "metas": metas,
        "share": st.share_json(c.desktop).await,
    })))
}

/// Pembungkus galat basis data agar crate ini tidak bergantung pada rusqlite.
mod rusqlite_err {
    pub struct E(pub String);
    pub fn wrap<T: std::fmt::Display>(e: T) -> E {
        E(e.to_string())
    }
}

async fn events<C: Connector>(State(st): St<C>, c: Caller, Query(p): Query<EventParams>) -> ApiResult<Json<Value>> {
    need(&st, &c, Resource::Event, Level::Read)?;
    let kinds: Vec<EventKind> = p.kinds.as_deref().unwrap_or("").split(',').filter(|k| !k.is_empty()).filter_map(EventKind::parse).collect();
    let limit = p.limit.unwrap_or(50).clamp(1, 500);
    let q = EventQuery { router_id: p.router_id, kinds, username: p.q, since_ms: p.since, until_ms: p.until, before_id: p.before, limit };
    let rows = st.telemon.with_store(|s| s.list_events(&q).map_err(rusqlite_err::wrap)).map_err(|e| ApiError::internal(e.0))?;
    let next = (rows.len() == limit).then(|| rows.last().map(|r| r.id.to_string())).flatten();
    Ok(Json(json!({ "events": rows.iter().map(EventDto::from).collect::<Vec<_>>(), "nextBefore": next })))
}

async fn audit<C: Connector>(State(st): St<C>, c: Caller, Query(p): Query<AuditParams>) -> ApiResult<Json<Value>> {
    need(&st, &c, Resource::Admin, Level::Read)?;
    let limit = p.limit.unwrap_or(50).clamp(1, 500);
    let q = AuditQuery { router_id: p.router_id, actor: p.actor, target: p.q, only_failed: p.failed, since_ms: p.since, until_ms: p.until, before_id: p.before, limit };
    let rows = st.telemon.with_store(|s| s.list_audit(&q).map_err(rusqlite_err::wrap)).map_err(|e| ApiError::internal(e.0))?;
    let next = (rows.len() == limit).then(|| rows.last().map(|r| r.id.to_string())).flatten();
    Ok(Json(json!({ "entries": rows.iter().map(AuditDto::from).collect::<Vec<_>>(), "nextBefore": next })))
}

/// Password PPPoE hanya untuk izin Ubah pada Users (ADR 0009); setiap tampil tercatat di riwayat.
async fn user_password<C: Connector>(State(st): St<C>, c: Caller, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    need(&st, &c, Resource::User, Level::Update)?;
    let pw = st.telemon.with_store(|s| s.user_password(&id));
    let pw = match pw {
        Ok(Some(p)) => p,
        Ok(None) => return Err(ApiError::not_found()),
        Err(e) => return Err(telemon_core::service::ServiceError::from(e).into()),
    };
    let (router_id, target) = id.split_once(':').unwrap_or(("-", id.as_str()));
    let entry = AuditEntry {
        at_ms: st.now(),
        actor: c.session.username.clone(),
        router_id: router_id.into(),
        action: "user.password_view".into(),
        target: target.into(),
        title: format!("Lihat password {target}"),
        changes: Vec::<Change>::new(),
        outcome: AuditOutcome::Ok,
    };
    let _ = st.telemon.with_store(|s| s.record_audit(&entry));
    Ok(Json(json!({ "password": pw })))
}

async fn user_meta<C: Connector>(State(st): St<C>, c: Caller, Path(id): Path<String>, Json(r): Json<MetaReq>) -> ApiResult<Json<Value>> {
    need(&st, &c, Resource::User, Level::Update)?;
    let clean = |v: Option<String>, max: usize| v.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).map(|s| s.chars().take(max).collect::<String>());
    if r.tags.len() > 10 || r.tags.iter().any(|t| t.chars().count() > 30) {
        return Err(ApiError::bad("Maksimal 10 label, masing-masing 30 karakter"));
    }
    let (router_id, username) = id.split_once(':').ok_or_else(ApiError::not_found)?;
    let exists = st.telemon.with_store(|s| s.list_users(Some(router_id), true).map_err(rusqlite_err::wrap)).map_err(|e| ApiError::internal(e.0))?;
    if !exists.iter().any(|u| u.username == username) {
        return Err(ApiError::not_found());
    }
    let meta = UserMeta {
        customer_name: clean(r.customer_name, 80),
        phone: clean(r.phone, 30),
        note: clean(r.note, 1000),
        tags: r.tags.iter().map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect(),
        updated_at: st.now(),
    };
    st.telemon.with_store(|s| s.set_user_meta(&id, &meta).map_err(rusqlite_err::wrap)).map_err(|e| ApiError::internal(e.0))?;
    Ok(Json(json!({ "customerName": meta.customer_name, "phone": meta.phone, "note": meta.note, "tags": meta.tags, "updatedAt": meta.updated_at })))
}

// ---------------- router ----------------

fn tls_mode(tls: bool, fingerprint: &Option<String>) -> ApiResult<(TlsMode, Option<String>)> {
    if !tls {
        return Ok((TlsMode::Plain, None));
    }
    let pin = fingerprint.as_deref().and_then(telemon_core::normalize_fingerprint).ok_or_else(|| ApiError::bad("Sidik jari sertifikat wajib untuk TLS (ambil dulu lewat /api/routers/fingerprint)"))?;
    let fp = telemon_core::Fingerprint::parse(&pin).ok_or_else(|| ApiError::bad("Sidik jari tidak valid"))?;
    Ok((TlsMode::Pinned(fp), Some(pin)))
}

async fn router_add<C: Connector>(State(st): St<C>, c: Caller, Json(r): Json<RouterAddReq>) -> ApiResult<(StatusCode, Json<RouterDto>)> {
    need(&st, &c, Resource::Router, Level::Update)?;
    let (_, pin) = tls_mode(r.tls, &r.fingerprint)?;
    let n = NewRouter { name: r.name, host: r.host, port: r.port, tls_pin: pin, username: r.username, can_write: r.allow_write };
    let rec = st.telemon.add_router(&n, &r.password)?;
    let _ = st.telemon.start_monitor(&rec.id);
    Ok((StatusCode::CREATED, Json(RouterDto::from(&rec))))
}

async fn router_edit<C: Connector>(State(st): St<C>, c: Caller, Path(id): Path<String>, Json(r): Json<RouterEditReq>) -> ApiResult<Json<RouterDto>> {
    need(&st, &c, Resource::Router, Level::Update)?;
    let tls_pin = match (r.tls, &r.fingerprint) {
        (Some(false), _) => Some(None),
        (Some(true), f) | (None, f @ Some(_)) => Some(tls_mode(true, f)?.1),
        (None, None) => None,
    };
    let patch = RouterPatch { name: r.name, host: r.host, port: r.port, tls_pin, username: r.username, can_write: r.allow_write };
    let rec = st.telemon.update_router(&id, &patch, r.password.as_deref())?;
    Ok(Json(RouterDto::from(&rec)))
}

async fn router_delete<C: Connector>(State(st): St<C>, c: Caller, Path(id): Path<String>) -> ApiResult<StatusCode> {
    need(&st, &c, Resource::Router, Level::Update)?;
    st.telemon.remove_router(&id)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn router_probe<C: Connector>(State(st): St<C>, c: Caller, Json(r): Json<ProbeReq>) -> ApiResult<Json<Value>> {
    need(&st, &c, Resource::Router, Level::Update)?;
    let (mode, _) = tls_mode(r.tls, &r.fingerprint)?;
    let Probe { model, version, can_write, can_read_passwords } = st.telemon.probe(&format!("{}:{}", r.host.trim(), r.port), &r.username, &r.password, mode).await?;
    Ok(Json(json!({ "model": model, "version": version, "canWrite": can_write, "canReadPasswords": can_read_passwords })))
}

async fn router_fingerprint<C: Connector>(State(st): St<C>, c: Caller, Json(r): Json<FingerprintReq>) -> ApiResult<Json<Value>> {
    need(&st, &c, Resource::Router, Level::Update)?;
    let fp = st.telemon.fetch_fingerprint(&format!("{}:{}", r.host.trim(), r.port)).await?;
    Ok(Json(json!({ "fingerprint": fp.to_hex() })))
}

async fn router_import<C: Connector>(State(st): St<C>, c: Caller, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    need(&st, &c, Resource::Router, Level::Update)?;
    let (users, profiles) = st.telemon.import_now(&id).await?;
    let counts = |r: telemon_core::store::SyncReport| json!({ "added": r.added, "updated": r.updated + r.restored, "deleted": r.soft_deleted });
    Ok(Json(json!({ "routerId": id, "at": st.now(), "users": counts(users), "profiles": counts(profiles) })))
}

// ---------------- tulis dengan tinjauan ----------------

fn opt(v: Option<String>) -> Option<String> {
    v.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

async fn plan_create<C: Connector>(State(st): St<C>, c: Caller, Path(router_id): Path<String>, Json(req): Json<PlanReq>) -> ApiResult<Json<Value>> {
    let resource = match &req {
        PlanReq::UserAdd { .. } | PlanReq::UserEdit { .. } | PlanReq::UserSoftDelete { .. } | PlanReq::UserRestore { .. } => Resource::User,
        PlanReq::ProfileAdd { .. } | PlanReq::ProfileEdit { .. } => Resource::Profile,
    };
    need(&st, &c, resource, Level::Update)?;
    let t = &st.telemon;
    let plan = match req {
        PlanReq::UserAdd { name, password, profile, service, remote_address, comment, disabled } => {
            t.plan_add_user(&router_id, &NewUser { name, password, profile, service, remote_address: opt(remote_address), comment: opt(comment), disabled }).await?
        }
        PlanReq::UserEdit { name, password, profile, remote_address, comment, disabled } => {
            t.plan_edit_user(&router_id, &name, &UserPatch { password, profile, remote_address, comment, disabled }).await?
        }
        PlanReq::UserSoftDelete { name } => t.plan_soft_delete_user(&router_id, &name).await?,
        PlanReq::UserRestore { name } => t.plan_restore_user(&router_id, &name).await?,
        PlanReq::ProfileAdd { name, rate_limit, local_address, remote_address, only_one, comment } => {
            t.plan_add_profile(&router_id, &NewProfile { name, rate_limit, local_address, remote_address, only_one, comment }).await?
        }
        PlanReq::ProfileEdit { name, rate_limit, local_address, remote_address, only_one, comment } => {
            t.plan_edit_profile(&router_id, &name, &ProfilePatch { rate_limit, local_address, remote_address, only_one, comment }).await?
        }
    };
    let now = st.now();
    let expires = now + st.cfg.plan_ttl.as_millis() as i64;
    let changes: Vec<ChangeDto> = plan.changes.iter().map(ChangeDto::from).collect();
    let title = plan.title.clone();
    let id = st
        .plans
        .put(PendingPlan { plan, router_id, account_id: c.session.account_id.clone(), resource, expires }, now)
        .map_err(|e| match e {
            crate::state::PlanError::Full => ApiError::too_many(60),
            crate::state::PlanError::Random => ApiError::internal("gagal membuat id rencana"),
        })?;
    Ok(Json(json!({ "planId": id, "title": title, "changes": changes, "expiresAt": expires })))
}

async fn plan_apply<C: Connector>(State(st): St<C>, c: Caller, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    let p = st.plans.take(&id, &c.session.account_id, st.now()).ok_or_else(|| ApiError::new(StatusCode::GONE, "plan_expired", "Rencana tidak ada, kedaluwarsa, atau sudah diterapkan; buat ulang"))?;
    // Izin diperiksa lagi saat menerapkan: peran bisa berubah sejak tinjauan.
    need(&st, &c, p.resource, Level::Update)?;
    let done = st.telemon.apply(&p.router_id, &c.session.username, &p.plan).await?;
    Ok(Json(json!({ "stepsDone": done })))
}

// ---------------- pengaturan dan penyimpanan ----------------

async fn settings_get<C: Connector>(State(st): St<C>, c: Caller) -> ApiResult<Json<SettingsDto>> {
    need(&st, &c, Resource::Pengaturan, Level::Read)?;
    let s = st.telemon.with_store(|s| s.get_settings().map_err(rusqlite_err::wrap)).map_err(|e| ApiError::internal(e.0))?;
    Ok(Json(SettingsDto::from(&s)))
}

/// Simpan pengaturan. Izin tulis lewat akses publik hanya bisa diubah dari aplikasi desktop (ADR 0006).
async fn settings_put<C: Connector>(State(st): St<C>, c: Caller, Json(req): Json<SettingsDto>) -> ApiResult<Json<SettingsDto>> {
    need(&st, &c, Resource::Pengaturan, Level::Update)?;
    let next = telemon_core::Settings::from(&req);
    let old = st.telemon.with_store(|s| s.get_settings().map_err(rusqlite_err::wrap)).map_err(|e| ApiError::internal(e.0))?;
    if next.remote.allow_public_write != old.remote.allow_public_write && !c.desktop {
        return Err(ApiError::forbidden("Izin tulis lewat akses publik hanya bisa diubah dari aplikasi desktop"));
    }
    next.validate().map_err(ApiError::bad)?;
    st.telemon.with_store(|s| s.save_settings(&next).map_err(rusqlite_err::wrap)).map_err(|e| ApiError::internal(e.0))?;
    st.apply_policy(&next);
    st.telemon.apply_settings(&next)?;
    let changes: Vec<Change> = old.diff(&next).into_iter().map(|(k, from, to)| Change { field: k.into(), from: Some(from), to: Some(to) }).collect();
    if !changes.is_empty() {
        let entry = AuditEntry {
            at_ms: st.now(),
            actor: c.session.username.clone(),
            router_id: "-".into(),
            action: "settings.update".into(),
            target: "pengaturan".into(),
            title: "Ubah pengaturan".into(),
            changes,
            outcome: AuditOutcome::Ok,
        };
        let _ = st.telemon.with_store(|s| s.record_audit(&entry));
    }
    Ok(Json(SettingsDto::from(&next)))
}

fn purge_json(c: telemon_core::store::PurgeCounts) -> Value {
    json!({ "events": c.events, "users": c.users, "profiles": c.profiles, "chats": 0 })
}

/// Kartu Penyimpanan: ukuran, jumlah, dan yang akan dihapus bila "Bersihkan sekarang" dijalankan
/// dengan pengaturan tersimpan.
async fn storage_get<C: Connector>(State(st): St<C>, c: Caller) -> ApiResult<Json<Value>> {
    need(&st, &c, Resource::Pengaturan, Level::Read)?;
    let t = &st.telemon;
    let io = |e: rusqlite_err::E| ApiError::internal(e.0);
    let settings = t.with_store(|s| s.get_settings().map_err(rusqlite_err::wrap)).map_err(io)?;
    let (events, users, deleted_users, deleted_profiles) = t.with_store(|s| s.storage_counts().map_err(rusqlite_err::wrap)).map_err(io)?;
    let size = t.with_store(|s| s.size_bytes().map_err(rusqlite_err::wrap)).map_err(io)?;
    let preview = t.purge_preview(&settings.retention_policy())?;
    Ok(Json(json!({
        "path": st.cfg.db_path,
        "sizeBytes": size,
        "events": events,
        "users": users,
        "deletedUsers": deleted_users,
        "deletedProfiles": deleted_profiles,
        "chats": 0,
        "purgeable": purge_json(preview),
    })))
}

async fn storage_purge<C: Connector>(State(st): St<C>, c: Caller) -> ApiResult<Json<Value>> {
    need(&st, &c, Resource::Pengaturan, Level::Update)?;
    let settings = st.telemon.with_store(|s| s.get_settings().map_err(rusqlite_err::wrap)).map_err(|e| ApiError::internal(e.0))?;
    let done = st.telemon.purge_now(&settings.retention_policy())?;
    let entry = AuditEntry {
        at_ms: st.now(),
        actor: c.session.username.clone(),
        router_id: "-".into(),
        action: "storage.purge".into(),
        target: "penyimpanan".into(),
        title: "Bersihkan data lama".into(),
        changes: vec![
            Change { field: "event".into(), from: None, to: Some(done.events.to_string()) },
            Change { field: "user".into(), from: None, to: Some(done.users.to_string()) },
            Change { field: "profile".into(), from: None, to: Some(done.profiles.to_string()) },
            Change { field: "riwayat perubahan".into(), from: None, to: Some(done.audit.to_string()) },
        ],
        outcome: AuditOutcome::Ok,
    };
    let _ = st.telemon.with_store(|s| s.record_audit(&entry));
    Ok(Json(purge_json(done)))
}

// ---------------- profile: tanda hapus lokal ----------------

/// Profile hanya ditandai terhapus di Telemon (tidak ada perintah ke router); ditolak bila masih dipakai user.
async fn profile_mark<C: Connector>(st: &Arc<AppState<C>>, c: &Caller, id: &str, delete: bool) -> ApiResult<Json<ProfileDto>> {
    need(st, c, Resource::Profile, Level::Update)?;
    let (router_id, name) = id.split_once(':').ok_or_else(ApiError::not_found)?;
    let io = |e: rusqlite_err::E| ApiError::internal(e.0);
    let exists = st.telemon.with_store(|s| s.list_profiles(Some(router_id), true).map_err(rusqlite_err::wrap)).map_err(io)?;
    if !exists.iter().any(|p| p.name == name) {
        return Err(ApiError::not_found());
    }
    if delete {
        st.telemon.check_profile_deletable(router_id, name).await?;
    }
    let now = st.now();
    let changed = st
        .telemon
        .with_store(|s| if delete { s.soft_delete_profile(id, now) } else { s.restore_profile(id) }.map_err(rusqlite_err::wrap))
        .map_err(io)?;
    if changed {
        let entry = AuditEntry {
            at_ms: now,
            actor: c.session.username.clone(),
            router_id: router_id.into(),
            action: if delete { "profile.soft_delete" } else { "profile.restore" }.into(),
            target: name.into(),
            title: format!("{} profile {name}", if delete { "Hapus (soft)" } else { "Pulihkan" }),
            changes: Vec::new(),
            outcome: AuditOutcome::Ok,
        };
        let _ = st.telemon.with_store(|s| s.record_audit(&entry));
    }
    let rows = st.telemon.with_store(|s| s.list_profiles(Some(router_id), true).map_err(rusqlite_err::wrap)).map_err(io)?;
    let row = rows.iter().find(|p| p.name == name).ok_or_else(ApiError::not_found)?;
    Ok(Json(ProfileDto::from(row)))
}

async fn profile_soft_delete<C: Connector>(State(st): St<C>, c: Caller, Path(id): Path<String>) -> ApiResult<Json<ProfileDto>> {
    profile_mark(&st, &c, &id, true).await
}

async fn profile_restore<C: Connector>(State(st): St<C>, c: Caller, Path(id): Path<String>) -> ApiResult<Json<ProfileDto>> {
    profile_mark(&st, &c, &id, false).await
}

// ---------------- desktop dan berbagi ----------------

/// Kesehatan server. `desktop: true` hanya bila permintaan membawa token desktop yang sah.
async fn health<C: Connector>(
    axum::Extension(surface): axum::Extension<Surface>,
    axum::extract::ConnectInfo(peer): axum::extract::ConnectInfo<std::net::SocketAddr>,
    headers: HeaderMap,
) -> Json<Value> {
    Json(json!({ "ok": true, "desktop": is_desktop(&surface, &headers, peer) }))
}

#[derive(serde::Deserialize)]
pub struct EntryQuery {
    t: String,
}

/// Pintu masuk webview aplikasi desktop: menukar token dengan cookie lalu membuka UI.
pub async fn desktop_entry<C: Connector>(
    axum::Extension(surface): axum::Extension<Surface>,
    axum::extract::ConnectInfo(peer): axum::extract::ConnectInfo<std::net::SocketAddr>,
    Query(q): Query<EntryQuery>,
) -> Result<Response, ApiError> {
    let Surface::Desktop(token) = surface else { return Err(ApiError::not_found()) };
    if !peer.ip().is_loopback() || !ct_eq(q.t.as_bytes(), token.as_bytes()) {
        return Err(ApiError::forbidden("Token desktop tidak valid"));
    }
    let cookie = format!("{DESKTOP_COOKIE}={token}; HttpOnly; SameSite=Strict; Path=/");
    let mut r = axum::response::Redirect::to("/").into_response();
    r.headers_mut().append(header::SET_COOKIE, header::HeaderValue::from_str(&cookie).map_err(ApiError::internal)?);
    Ok(r)
}

fn desktop_only(c: &Caller) -> ApiResult<()> {
    if c.desktop { Ok(()) } else { Err(ApiError::forbidden("Hanya bisa dilakukan dari aplikasi desktop")) }
}

async fn share_get<C: Connector>(State(st): St<C>, c: Caller) -> ApiResult<Json<Value>> {
    desktop_only(&c)?;
    Ok(Json(st.share_json(true).await))
}

#[derive(serde::Deserialize)]
pub struct LanReq {
    enabled: bool,
    bind: String,
    port: u16,
}

async fn share_lan<C: Connector>(State(st): St<C>, c: Caller, Json(r): Json<LanReq>) -> ApiResult<Json<Value>> {
    desktop_only(&c)?;
    st.set_lan(r.enabled, &r.bind, r.port).await?;
    Ok(Json(st.share_json(true).await))
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublikReq {
    enabled: bool,
    #[serde(default)]
    duration_hours: u32,
}

/// Akses Publik lewat Cloudflare Quick Tunnel (ADR 0003): hanya dari desktop, butuh server LAN menyala.
async fn share_publik<C: Connector>(State(st): St<C>, c: Caller, Json(r): Json<PublikReq>) -> ApiResult<Json<Value>> {
    desktop_only(&c)?;
    st.set_public(r.enabled, r.duration_hours).await?;
    Ok(Json(st.share_json(true).await))
}

#[derive(serde::Deserialize)]
pub struct Require2faReq {
    required: bool,
}

async fn share_require_2fa<C: Connector>(State(st): St<C>, c: Caller, Json(r): Json<Require2faReq>) -> ApiResult<Json<Value>> {
    desktop_only(&c)?;
    let (_, public_on) = st.share_flags();
    st.set_share(r.required, public_on);
    st.telemon.with_store(|s| s.kv_set("share.require_2fa", if r.required { "true" } else { "false" }).map_err(rusqlite_err::wrap)).map_err(|e| ApiError::internal(e.0))?;
    Ok(Json(st.share_json(true).await))
}

// ---------------- pembaruan aplikasi (hanya desktop) ----------------

async fn update_get<C: Connector>(State(st): St<C>, c: Caller) -> ApiResult<Json<Value>> {
    desktop_only(&c)?;
    Ok(Json(st.update_json()))
}

async fn update_check<C: Connector>(State(st): St<C>, c: Caller) -> ApiResult<Json<Value>> {
    desktop_only(&c)?;
    st.check_update().await?;
    Ok(Json(st.update_json()))
}

async fn update_install<C: Connector>(State(st): St<C>, c: Caller) -> ApiResult<Json<Value>> {
    desktop_only(&c)?;
    Ok(Json(st.install_update().await?))
}

#[derive(serde::Deserialize)]
pub struct ChannelReq {
    channel: String,
}

async fn update_channel<C: Connector>(State(st): St<C>, c: Caller, Json(r): Json<ChannelReq>) -> ApiResult<Json<Value>> {
    desktop_only(&c)?;
    st.set_update_channel(&r.channel)?;
    Ok(Json(st.update_json()))
}
