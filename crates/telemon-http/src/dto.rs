//! Bentuk JSON untuk UI (camelCase), mengikuti tipe TypeScript di `src/lib/types.ts`.
//! Password pelanggan dan kata sandi router tidak pernah ada di sini.

use std::collections::BTreeMap;

use serde::{Deserialize, Deserializer, Serialize};
use telemon_core::auth::{AccountSummary, Level, Permissions, Resource, Role, Session, SessionDevice, Via};
use telemon_core::store::{ProfileRow, RouterRecord, RouterStatus, StoredAudit, StoredEvent, UserRow};
use telemon_core::poller::UserStatus;
use telemon_core::AuditOutcome;

fn perms(p: &Permissions) -> BTreeMap<&'static str, &'static str> {
    Resource::ALL.iter().map(|r| (r.as_str(), p.get(*r).as_str())).collect()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionDto {
    pub id: String,
    pub user_id: String,
    pub username: String,
    pub display_name: String,
    pub role_name: String,
    pub permissions: BTreeMap<&'static str, &'static str>,
    pub via: &'static str,
    pub two_factor: bool,
}

impl From<&Session> for SessionDto {
    fn from(s: &Session) -> Self {
        Self {
            id: s.id.clone(),
            user_id: s.account_id.clone(),
            username: s.username.clone(),
            display_name: s.display_name.clone(),
            role_name: s.role_name.clone(),
            permissions: perms(&s.permissions),
            via: if s.via == Via::Publik { "publik" } else { "lan" },
            two_factor: s.two_factor,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoleDto {
    pub id: String,
    pub name: String,
    pub description: String,
    pub permissions: BTreeMap<&'static str, &'static str>,
    pub account_count: u32,
    pub builtin: bool,
}

impl From<&Role> for RoleDto {
    fn from(r: &Role) -> Self {
        Self { id: r.id.clone(), name: r.name.clone(), description: r.description.clone(), permissions: perms(&r.permissions), account_count: r.account_count, builtin: r.builtin }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountDto {
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

impl From<&AccountSummary> for AccountDto {
    fn from(a: &AccountSummary) -> Self {
        Self { id: a.id.clone(), username: a.username.clone(), display_name: a.display_name.clone(), role_id: a.role_id.clone(), role_name: a.role_name.clone(), two_factor: a.two_factor, disabled: a.disabled, created_at: a.created_at, last_login_at: a.last_login_at }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceDto {
    pub id: String,
    pub device: String,
    pub ip: String,
    pub via: &'static str,
    pub created_at: i64,
    pub last_seen_at: i64,
    pub current: bool,
}

impl From<&SessionDevice> for DeviceDto {
    fn from(d: &SessionDevice) -> Self {
        Self { id: d.id.clone(), device: d.device.clone(), ip: d.ip.clone(), via: if d.via == Via::Publik { "publik" } else { "lan" }, created_at: d.created_at, last_seen_at: d.last_seen_at, current: d.current }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RouterDto {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub tls: bool,
    pub model: String,
    pub version: String,
    pub status: &'static str,
    pub last_poll_at: Option<i64>,
    pub can_write: bool,
    pub last_import_at: Option<i64>,
    pub error: Option<String>,
}

impl From<&RouterRecord> for RouterDto {
    fn from(r: &RouterRecord) -> Self {
        Self {
            id: r.id.clone(),
            name: r.name.clone(),
            host: r.host.clone(),
            port: r.port,
            tls: r.tls_pin.is_some(),
            model: r.model.clone(),
            version: r.version.clone(),
            status: match r.status {
                RouterStatus::Online => "online",
                RouterStatus::Offline => "offline",
                RouterStatus::Error => "error",
            },
            last_poll_at: r.last_poll_at,
            can_write: r.can_write,
            last_import_at: r.last_import_at,
            error: r.error.clone(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserDto {
    pub id: String,
    pub router_id: String,
    pub username: String,
    pub profile: String,
    pub status: &'static str,
    pub address: Option<String>,
    pub caller_id: Option<String>,
    pub connected_at: Option<i64>,
    pub last_seen_at: Option<i64>,
    pub last_event: Option<&'static str>,
    pub last_event_at: Option<i64>,
    pub comment: Option<String>,
    pub static_address: Option<String>,
    pub deleted_at: Option<i64>,
    pub deleted_reason: Option<String>,
}

impl From<&UserRow> for UserDto {
    fn from(u: &UserRow) -> Self {
        Self {
            id: u.id.clone(),
            router_id: u.router_id.clone(),
            username: u.username.clone(),
            profile: u.profile.clone(),
            status: match u.status {
                UserStatus::Active => "active",
                UserStatus::Inactive => "inactive",
                UserStatus::Disabled => "disabled",
            },
            address: u.address.clone(),
            caller_id: u.caller_id.clone(),
            connected_at: u.connected_at,
            last_seen_at: u.last_seen_at,
            last_event: u.last_event.map(|k| k.as_str()),
            last_event_at: u.last_event_at,
            comment: u.comment.clone(),
            static_address: u.static_address.clone(),
            deleted_at: u.deleted_at,
            deleted_reason: u.deleted_reason.clone(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileDto {
    pub id: String,
    pub router_id: String,
    pub name: String,
    pub rate_limit: Option<String>,
    pub local_address: Option<String>,
    pub remote_address: Option<String>,
    pub only_one: String,
    pub comment: Option<String>,
    pub deleted_at: Option<i64>,
    pub deleted_reason: Option<String>,
}

impl From<&ProfileRow> for ProfileDto {
    fn from(p: &ProfileRow) -> Self {
        Self {
            id: p.id.clone(),
            router_id: p.router_id.clone(),
            name: p.name.clone(),
            rate_limit: p.rate_limit.clone(),
            local_address: p.local_address.clone(),
            remote_address: p.remote_address.clone(),
            only_one: p.only_one.clone().unwrap_or_else(|| "default".into()),
            comment: p.comment.clone(),
            deleted_at: p.deleted_at,
            deleted_reason: p.deleted_reason.clone(),
        }
    }
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EventDto {
    pub id: String,
    pub at: i64,
    pub router_id: String,
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub username: Option<String>,
    pub address: Option<String>,
    pub caller_id: Option<String>,
    pub message: String,
}

impl EventDto {
    pub fn from_event(id: String, router_id: &str, e: &telemon_core::Event) -> Self {
        Self { id, at: e.at_ms, router_id: router_id.into(), kind: e.kind.as_str(), username: e.username.clone(), address: e.address.clone(), caller_id: e.caller_id.clone(), message: e.message.clone() }
    }
}

impl From<&StoredEvent> for EventDto {
    fn from(s: &StoredEvent) -> Self {
        Self::from_event(s.id.to_string(), &s.router_id, &s.event)
    }
}

#[derive(Serialize)]
pub struct ChangeDto {
    pub field: String,
    pub from: Option<String>,
    pub to: Option<String>,
}

impl From<&telemon_core::Change> for ChangeDto {
    fn from(c: &telemon_core::Change) -> Self {
        Self { field: c.field.clone(), from: c.from.clone(), to: c.to.clone() }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditDto {
    pub id: String,
    pub at: i64,
    pub actor: String,
    pub router_id: String,
    pub action: String,
    pub target: String,
    pub title: String,
    pub changes: Vec<ChangeDto>,
    pub ok: bool,
    pub steps_done: usize,
    pub error: Option<String>,
}

impl From<&StoredAudit> for AuditDto {
    fn from(a: &StoredAudit) -> Self {
        let (ok, steps_done, error) = match &a.entry.outcome {
            AuditOutcome::Ok => (true, 0, None),
            AuditOutcome::Failed { steps_done, error } => (false, *steps_done, Some(error.clone())),
        };
        Self {
            id: a.id.to_string(),
            at: a.entry.at_ms,
            actor: a.entry.actor.clone(),
            router_id: a.entry.router_id.clone(),
            action: a.entry.action.clone(),
            target: a.entry.target.clone(),
            title: a.entry.title.clone(),
            changes: a.entry.changes.iter().map(ChangeDto::from).collect(),
            ok,
            steps_done,
            error,
        }
    }
}

// ---------------- permintaan ----------------

#[derive(Deserialize)]
pub struct LoginReq {
    pub username: String,
    pub password: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TwoFaReq {
    pub challenge_id: String,
    pub code: String,
    #[serde(default)]
    pub recovery: bool,
}

#[derive(Deserialize)]
pub struct CodeReq {
    pub code: String,
}

#[derive(Deserialize)]
pub struct PasswordReq {
    pub password: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangePasswordReq {
    pub current: String,
    pub next: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileReq {
    pub display_name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewAccountReq {
    pub username: String,
    pub display_name: String,
    pub password: String,
    pub role_id: String,
}

#[derive(Deserialize)]
pub struct DisabledReq {
    pub disabled: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoleIdReq {
    pub role_id: String,
}

#[derive(Deserialize)]
pub struct RoleReq {
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub permissions: BTreeMap<String, String>,
}

impl RoleReq {
    pub fn permissions(&self) -> Result<Permissions, String> {
        let mut p = Permissions::default();
        for (k, v) in &self.permissions {
            let r = Resource::parse(k).ok_or_else(|| format!("menu tidak dikenal: {k}"))?;
            let l = Level::parse(v).ok_or_else(|| format!("level tidak dikenal: {v}"))?;
            p.set(r, l);
        }
        Ok(p)
    }
}

/// Kolom opsional yang bisa dikosongkan: tidak ada = tidak diubah, `null` = kosongkan.
pub fn double_option<'de, D, T>(d: D) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Ok(Some(Option::deserialize(d)?))
}

#[derive(Deserialize)]
#[serde(tag = "action")]
pub enum PlanReq {
    #[serde(rename = "user.add", rename_all = "camelCase")]
    UserAdd { name: String, password: String, profile: String, service: Option<String>, remote_address: Option<String>, comment: Option<String>, #[serde(default)] disabled: bool },
    #[serde(rename = "user.edit", rename_all = "camelCase")]
    UserEdit {
        name: String,
        password: Option<String>,
        profile: Option<String>,
        #[serde(default, deserialize_with = "double_option")]
        remote_address: Option<Option<String>>,
        #[serde(default, deserialize_with = "double_option")]
        comment: Option<Option<String>>,
        disabled: Option<bool>,
    },
    #[serde(rename = "user.soft_delete")]
    UserSoftDelete { name: String },
    #[serde(rename = "user.restore")]
    UserRestore { name: String },
    #[serde(rename = "profile.add", rename_all = "camelCase")]
    ProfileAdd { name: String, rate_limit: Option<String>, local_address: Option<String>, remote_address: Option<String>, only_one: Option<String>, comment: Option<String> },
    #[serde(rename = "profile.edit", rename_all = "camelCase")]
    ProfileEdit {
        name: String,
        #[serde(default, deserialize_with = "double_option")]
        rate_limit: Option<Option<String>>,
        #[serde(default, deserialize_with = "double_option")]
        local_address: Option<Option<String>>,
        #[serde(default, deserialize_with = "double_option")]
        remote_address: Option<Option<String>>,
        only_one: Option<String>,
        #[serde(default, deserialize_with = "double_option")]
        comment: Option<Option<String>>,
    },
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RouterAddReq {
    pub name: String,
    pub host: String,
    pub port: u16,
    #[serde(default)]
    pub tls: bool,
    pub fingerprint: Option<String>,
    pub username: String,
    pub password: String,
    #[serde(default)]
    pub allow_write: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RouterEditReq {
    pub name: Option<String>,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub tls: Option<bool>,
    pub fingerprint: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub allow_write: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeReq {
    pub host: String,
    pub port: u16,
    #[serde(default)]
    pub tls: bool,
    pub fingerprint: Option<String>,
    pub username: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct FingerprintReq {
    pub host: String,
    pub port: u16,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetaReq {
    pub customer_name: Option<String>,
    pub phone: Option<String>,
    pub note: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct EventParams {
    pub router_id: Option<String>,
    /// Daftar dipisah koma: `auth_failed,disconnected`.
    #[serde(rename = "type")]
    pub kinds: Option<String>,
    pub q: Option<String>,
    pub since: Option<i64>,
    pub until: Option<i64>,
    pub before: Option<i64>,
    pub limit: Option<usize>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AuditParams {
    pub router_id: Option<String>,
    pub actor: Option<String>,
    pub q: Option<String>,
    #[serde(default)]
    pub failed: bool,
    pub since: Option<i64>,
    pub until: Option<i64>,
    pub before: Option<i64>,
    pub limit: Option<usize>,
}
