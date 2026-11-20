//! Alat baca untuk AI. Tiap alat menegakkan izin penanya (otorisasi pada alat, bukan pada teks pertanyaan,
//! ADR 0007), menyamarkan IP/MAC bila diminta, membatasi jumlah baris, dan tidak pernah memuat password,
//! komentar bebas, atau rahasia. Alat tidak punya jalur tulis sama sekali.

use serde_json::{Value, json};
use telemon_core::auth::{Auth, Level, Resource, Session};
use telemon_core::{EventKind, EventQuery, SharedStore, UserRow, UserStatus};

use crate::llm::ToolSpec;
use crate::types::{AiLink, AiTone, AiWidget, Field, ListRow, StatItem};

pub const MAX_ROWS: usize = 50;
const MAX_JSON: usize = 12_000;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Id,
    En,
}

impl Lang {
    pub fn parse(s: Option<&str>) -> Self {
        if s == Some("en") { Self::En } else { Self::Id }
    }
    fn t(self, id: &'static str, en: &'static str) -> &'static str {
        if self == Self::En { en } else { id }
    }
}

pub struct ToolCtx<'a> {
    pub store: &'a SharedStore,
    pub auth: &'a Auth,
    /// `None` = aplikasi desktop (pemilik, tanpa login).
    pub caller: Option<&'a Session>,
    pub mask: bool,
    pub now_ms: i64,
    pub lang: Lang,
}

pub struct ToolOutput {
    /// Teks JSON yang dikirim ke model.
    pub for_model: String,
    pub widgets: Vec<AiWidget>,
    pub source: Option<String>,
    /// Ringkasan langkah untuk ditampilkan ke pengguna.
    pub step: String,
}

pub fn specs() -> Vec<ToolSpec> {
    let limit = json!({ "type": "integer", "minimum": 1, "maximum": MAX_ROWS, "description": "Jumlah baris maksimum (bawaan 20)" });
    vec![
        ToolSpec { name: "get_summary", description: "Ringkasan kondisi sekarang: router online, jumlah user per status, dan kegagalan autentikasi 1 jam terakhir.", parameters: json!({ "type": "object", "properties": {} }) },
        ToolSpec { name: "list_routers", description: "Daftar router beserta statusnya.", parameters: json!({ "type": "object", "properties": {} }) },
        ToolSpec {
            name: "find_users",
            description: "Cari user PPPoE. Filter opsional: bagian username, status, router (nama), profil. Mengembalikan total yang cocok dan sebagian baris.",
            parameters: json!({ "type": "object", "properties": {
                "query": { "type": "string", "description": "Bagian dari username" },
                "status": { "type": "string", "enum": ["active", "inactive", "disabled", "auth_failed"], "description": "auth_failed = event terakhir gagal autentikasi" },
                "router": { "type": "string", "description": "Nama router" },
                "profile": { "type": "string", "description": "Nama profil PPP" },
                "limit": limit.clone() } }),
        },
        ToolSpec {
            name: "get_user",
            description: "Detail satu user (username persis) beserta event terbarunya.",
            parameters: json!({ "type": "object", "properties": { "username": { "type": "string" }, "router": { "type": "string", "description": "Nama router, bila username ada di lebih dari satu router" } }, "required": ["username"] }),
        },
        ToolSpec {
            name: "list_events",
            description: "Event terbaru: gagal autentikasi, terhubung, terputus, router down/up.",
            parameters: json!({ "type": "object", "properties": {
                "kind": { "type": "string", "enum": ["auth_failed", "authenticated", "connected", "disconnected", "router_down", "router_up"] },
                "router": { "type": "string" },
                "username": { "type": "string", "description": "Bagian dari username" },
                "hours": { "type": "integer", "minimum": 1, "maximum": 720, "description": "Rentang waktu ke belakang (bawaan 24)" },
                "limit": limit.clone() } }),
        },
        ToolSpec {
            name: "list_profiles",
            description: "Daftar profil PPP beserta jumlah user yang memakainya.",
            parameters: json!({ "type": "object", "properties": { "router": { "type": "string" } } }),
        },
    ]
}

fn err(step: String, msg: impl Into<String>) -> ToolOutput {
    ToolOutput { for_model: json!({ "error": msg.into() }).to_string(), widgets: vec![], source: None, step }
}

fn need(ctx: &ToolCtx, r: Resource, label: &str, step: &str) -> Result<(), ToolOutput> {
    ctx.auth
        .authorize(ctx.caller, r, Level::Read)
        .map_err(|_| err(format!("{step} — {}", ctx.lang.t("tidak ada izin", "no permission")), format!("peran penanya tidak boleh melihat {label}; jangan jawab dari data itu dan beri tahu pengguna")))
}

pub fn execute(ctx: &ToolCtx, name: &str, args: &Value) -> ToolOutput {
    let out = match name {
        "get_summary" => summary(ctx),
        "list_routers" => routers(ctx),
        "find_users" => find_users(ctx, args),
        "get_user" => get_user(ctx, args),
        "list_events" => list_events(ctx, args),
        "list_profiles" => profiles(ctx, args),
        other => err(format!("alat tidak dikenal: {other}"), format!("alat '{other}' tidak ada")),
    };
    cap(out)
}

fn cap(mut o: ToolOutput) -> ToolOutput {
    if o.for_model.len() > MAX_JSON {
        let mut cut = MAX_JSON;
        while !o.for_model.is_char_boundary(cut) {
            cut -= 1;
        }
        o.for_model = format!("{}…[dipotong]", &o.for_model[..cut]);
    }
    o
}

// ---------------------------------------------------------------- penyamaran dan format

pub fn mask_ip(s: &str) -> String {
    let parts: Vec<&str> = s.split('.').collect();
    if parts.len() == 4 && parts.iter().all(|p| p.parse::<u8>().is_ok()) {
        return format!("{}.{}.•.•", parts[0], parts[1]);
    }
    if s.contains(':') {
        return format!("{}::•", s.split(':').next().unwrap_or(""));
    }
    "•".into()
}

pub fn mask_mac(s: &str) -> String {
    let g: Vec<&str> = s.split(':').collect();
    if g.len() == 6 {
        return format!("{}:{}:••:••:••:••", g[0], g[1]);
    }
    "••".into()
}

fn ip(ctx: &ToolCtx, s: &Option<String>) -> Option<String> {
    s.as_ref().map(|v| if ctx.mask { mask_ip(v) } else { v.clone() })
}

fn mac(ctx: &ToolCtx, s: &Option<String>) -> Option<String> {
    s.as_ref().map(|v| if ctx.mask { mask_mac(v) } else { v.clone() })
}

/// "2026-11-19 08:17 UTC" tanpa pustaka tanggal (algoritma hari-sipil Howard Hinnant).
pub fn fmt_utc(ms: i64) -> String {
    let secs = ms.div_euclid(1000);
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02} UTC", rem / 3600, rem % 3600 / 60)
}

fn ago(ctx: &ToolCtx, at: i64) -> String {
    let mins = ((ctx.now_ms - at).max(0)) / 60_000;
    match (mins, ctx.lang) {
        (0, Lang::Id) => "baru saja".into(),
        (0, Lang::En) => "just now".into(),
        (m, Lang::Id) if m < 60 => format!("{m} mnt lalu"),
        (m, Lang::En) if m < 60 => format!("{m} min ago"),
        (m, Lang::Id) if m < 1440 => format!("{} jam lalu", m / 60),
        (m, Lang::En) if m < 1440 => format!("{} h ago", m / 60),
        (m, Lang::Id) => format!("{} hari lalu", m / 1440),
        (m, Lang::En) => format!("{} d ago", m / 1440),
    }
}

fn status_str(s: UserStatus) -> &'static str {
    match s {
        UserStatus::Active => "active",
        UserStatus::Inactive => "inactive",
        UserStatus::Disabled => "disabled",
    }
}

fn status_label(ctx: &ToolCtx, s: UserStatus) -> &'static str {
    match s {
        UserStatus::Active => ctx.lang.t("Aktif", "Active"),
        UserStatus::Inactive => ctx.lang.t("Tidak aktif", "Inactive"),
        UserStatus::Disabled => ctx.lang.t("Nonaktif", "Disabled"),
    }
}

fn tone_of(u: &UserRow) -> AiTone {
    if u.last_event == Some(EventKind::AuthFailed) && u.status != UserStatus::Active {
        return AiTone::Bad;
    }
    match u.status {
        UserStatus::Active => AiTone::Good,
        UserStatus::Inactive => AiTone::Warn,
        UserStatus::Disabled => AiTone::Muted,
    }
}

fn link(page: &str, params: &[(&str, &str)]) -> AiLink {
    AiLink { page: page.into(), params: if params.is_empty() { None } else { Some(params.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()) } }
}

fn s(args: &Value, key: &str) -> Option<String> {
    args.get(key).and_then(Value::as_str).map(str::trim).filter(|v| !v.is_empty()).map(str::to_string)
}

fn n(args: &Value, key: &str, default: i64, max: i64) -> i64 {
    args.get(key).and_then(Value::as_i64).unwrap_or(default).clamp(1, max)
}

struct Names {
    by_id: Vec<(String, String)>,
}

impl Names {
    fn load(ctx: &ToolCtx) -> Self {
        let routers = ctx.store.lock().map(|g| g.list_routers().unwrap_or_default()).unwrap_or_default();
        Self { by_id: routers.into_iter().map(|r| (r.id, r.name)).collect() }
    }
    fn name(&self, id: &str) -> String {
        self.by_id.iter().find(|(i, _)| i == id).map(|(_, n)| n.clone()).unwrap_or_else(|| id.to_string())
    }
    /// Cocokkan nama router (tanpa membedakan huruf) atau id.
    fn resolve(&self, wanted: &str) -> Option<String> {
        self.by_id.iter().find(|(i, n)| i == wanted || n.eq_ignore_ascii_case(wanted)).map(|(i, _)| i.clone())
    }
}

// ---------------------------------------------------------------- alat

fn summary(ctx: &ToolCtx) -> ToolOutput {
    let step = ctx.lang.t("Mengambil ringkasan", "Fetching the summary").to_string();
    let (may_users, may_events, may_routers) = (
        ctx.auth.authorize(ctx.caller, Resource::User, Level::Read).is_ok(),
        ctx.auth.authorize(ctx.caller, Resource::Event, Level::Read).is_ok(),
        ctx.auth.authorize(ctx.caller, Resource::Router, Level::Read).is_ok(),
    );
    if let Err(e) = need(ctx, Resource::Ringkasan, "ringkasan", &step) {
        return e;
    }
    let st = ctx.store.lock().expect("lock");
    let mut v = json!({});
    let mut items: Vec<StatItem> = vec![];
    if may_routers {
        let r = st.list_routers().unwrap_or_default();
        let online = r.iter().filter(|x| x.status.as_str() == "online").count();
        v["routers"] = json!({ "total": r.len(), "online": online });
        items.push(StatItem { label: ctx.lang.t("Router online", "Routers online").into(), value: format!("{online}/{}", r.len()), tone: Some(if online == r.len() { AiTone::Good } else { AiTone::Warn }) });
    }
    if may_users {
        let u = st.list_users(None, false).unwrap_or_default();
        let c = |s: UserStatus| u.iter().filter(|x| x.status == s).count();
        v["users"] = json!({ "total": u.len(), "active": c(UserStatus::Active), "inactive": c(UserStatus::Inactive), "disabled": c(UserStatus::Disabled) });
        items.push(StatItem { label: ctx.lang.t("User aktif", "Active users").into(), value: c(UserStatus::Active).to_string(), tone: Some(AiTone::Good) });
        items.push(StatItem { label: ctx.lang.t("Tidak aktif", "Inactive").into(), value: c(UserStatus::Inactive).to_string(), tone: Some(AiTone::Warn) });
        items.push(StatItem { label: ctx.lang.t("Nonaktif", "Disabled").into(), value: c(UserStatus::Disabled).to_string(), tone: Some(AiTone::Muted) });
    }
    if may_events {
        let q = EventQuery { kinds: vec![EventKind::AuthFailed], since_ms: Some(ctx.now_ms - 3_600_000), limit: 500, ..Default::default() };
        let failed = st.list_events(&q).unwrap_or_default();
        let mut names: Vec<&str> = failed.iter().filter_map(|e| e.event.username.as_deref()).collect();
        names.sort_unstable();
        names.dedup();
        v["auth_failed_last_hour"] = json!({ "events": failed.len(), "distinct_users": names.len() });
        items.push(StatItem { label: ctx.lang.t("Gagal auth (1 jam)", "Auth failed (1 h)").into(), value: names.len().to_string(), tone: Some(if names.is_empty() { AiTone::Good } else { AiTone::Bad }) });
    }
    v["generated_at"] = json!(fmt_utc(ctx.now_ms));
    ToolOutput { for_model: v.to_string(), widgets: vec![AiWidget::Stats { items }], source: Some(ctx.lang.t("Ringkasan Telemon", "Telemon overview").into()), step }
}

fn routers(ctx: &ToolCtx) -> ToolOutput {
    let step = ctx.lang.t("Membaca daftar router", "Reading the router list").to_string();
    if let Err(e) = need(ctx, Resource::Router, "router", &step) {
        return e;
    }
    let list = ctx.store.lock().expect("lock").list_routers().unwrap_or_default();
    let rows: Vec<Value> = list
        .iter()
        .map(|r| json!({ "name": r.name, "status": r.status.as_str(), "model": r.model, "version": r.version, "last_poll": r.last_poll_at.map(fmt_utc), "error": r.error }))
        .collect();
    let widget_rows = list
        .iter()
        .map(|r| ListRow {
            primary: r.name.clone(),
            secondary: Some(format!("{} {}", r.model, r.version).trim().to_string()),
            trailing: Some(r.status.as_str().into()),
            tone: Some(match r.status.as_str() { "online" => AiTone::Good, "offline" => AiTone::Warn, _ => AiTone::Bad }),
            to: Some(link("router", &[])),
        })
        .collect();
    ToolOutput { for_model: json!({ "routers": rows }).to_string(), widgets: vec![AiWidget::List { title: Some(ctx.lang.t("Router", "Routers").into()), rows: widget_rows }], source: Some("Router".into()), step }
}

fn find_users(ctx: &ToolCtx, a: &Value) -> ToolOutput {
    let step = ctx.lang.t("Mencari user", "Searching users").to_string();
    if let Err(e) = need(ctx, Resource::User, "Users", &step) {
        return e;
    }
    let names = Names::load(ctx);
    let router = match s(a, "router") {
        Some(w) => match names.resolve(&w) {
            Some(id) => Some(id),
            None => return err(step, format!("router '{w}' tidak ditemukan")),
        },
        None => None,
    };
    let query = s(a, "query").map(|q| q.to_lowercase());
    let status = s(a, "status");
    let profile = s(a, "profile").map(|p| p.to_lowercase());
    let limit = n(a, "limit", 20, MAX_ROWS as i64) as usize;
    let all = ctx.store.lock().expect("lock").list_users(router.as_deref(), false).unwrap_or_default();
    let matched: Vec<&UserRow> = all
        .iter()
        .filter(|u| query.as_ref().is_none_or(|q| u.username.to_lowercase().contains(q)))
        .filter(|u| profile.as_ref().is_none_or(|p| u.profile.to_lowercase() == *p))
        .filter(|u| match status.as_deref() {
            Some("auth_failed") => u.last_event == Some(EventKind::AuthFailed) && u.status != UserStatus::Active,
            Some(st) => status_str(u.status) == st,
            None => true,
        })
        .collect();
    let rows: Vec<Value> = matched
        .iter()
        .take(limit)
        .map(|u| {
            json!({ "username": u.username, "router": names.name(&u.router_id), "profile": u.profile, "status": status_str(u.status),
                "address": ip(ctx, &u.address), "last_event": u.last_event.map(|k| k.as_str()), "last_event_at": u.last_event_at.map(fmt_utc), "last_seen": u.last_seen_at.map(fmt_utc) })
        })
        .collect();
    let widget_rows = matched
        .iter()
        .take(limit)
        .map(|u| ListRow {
            primary: u.username.clone(),
            secondary: Some(format!("{} · {}", names.name(&u.router_id), u.profile)),
            trailing: Some(status_label(ctx, u.status).into()),
            tone: Some(tone_of(u)),
            to: Some(link("user", &[("q", u.username.as_str()), ("router", u.router_id.as_str())])),
        })
        .collect();
    let truncated = matched.len() > limit;
    ToolOutput {
        for_model: json!({ "total_matched": matched.len(), "returned": rows.len(), "truncated": truncated, "users": rows }).to_string(),
        widgets: if matched.is_empty() { vec![] } else { vec![AiWidget::List { title: Some(format!("{} ({})", ctx.lang.t("User", "Users"), matched.len())), rows: widget_rows }] },
        source: Some("Users".into()),
        step: format!("{} — {}", step, matched.len()),
    }
}

fn get_user(ctx: &ToolCtx, a: &Value) -> ToolOutput {
    let step = ctx.lang.t("Membaca detail user", "Reading user details").to_string();
    if let Err(e) = need(ctx, Resource::User, "Users", &step) {
        return e;
    }
    let Some(username) = s(a, "username") else { return err(step, "username wajib diisi") };
    let names = Names::load(ctx);
    let router = s(a, "router").and_then(|w| names.resolve(&w));
    let all = ctx.store.lock().expect("lock").list_users(router.as_deref(), false).unwrap_or_default();
    let found: Vec<&UserRow> = all.iter().filter(|u| u.username.eq_ignore_ascii_case(&username)).collect();
    if found.is_empty() {
        return err(step, format!("user '{username}' tidak ditemukan"));
    }
    let may_events = ctx.auth.authorize(ctx.caller, Resource::Event, Level::Read).is_ok();
    let mut widgets = vec![];
    let mut out = vec![];
    for u in found.iter().take(3) {
        let events = if may_events {
            let q = EventQuery { router_id: Some(u.router_id.clone()), username: Some(u.username.clone()), limit: 10, ..Default::default() };
            ctx.store.lock().expect("lock").list_events(&q).unwrap_or_default()
        } else {
            vec![]
        };
        let ev: Vec<Value> = events.iter().filter(|e| e.event.username.as_deref().is_some_and(|x| x.eq_ignore_ascii_case(&u.username))).map(|e| json!({ "kind": e.event.kind.as_str(), "at": fmt_utc(e.event.at_ms) })).collect();
        out.push(json!({
            "username": u.username, "router": names.name(&u.router_id), "profile": u.profile, "status": status_str(u.status),
            "address": ip(ctx, &u.address), "static_address": ip(ctx, &u.static_address), "caller_id": mac(ctx, &u.caller_id),
            "connected_at": u.connected_at.map(fmt_utc), "last_seen": u.last_seen_at.map(fmt_utc), "recent_events": ev,
            "events_visible": may_events,
        }));
        let mut fields = vec![Field { label: ctx.lang.t("Profil", "Profile").into(), value: u.profile.clone() }];
        if let Some(v) = ip(ctx, &u.address) {
            fields.push(Field { label: "IP".into(), value: v });
        }
        if let Some(v) = mac(ctx, &u.caller_id) {
            fields.push(Field { label: "MAC".into(), value: v });
        }
        if let Some(t) = u.last_seen_at {
            fields.push(Field { label: ctx.lang.t("Terakhir terlihat", "Last seen").into(), value: ago(ctx, t) });
        }
        widgets.push(AiWidget::User { username: u.username.clone(), router: names.name(&u.router_id), status: status_label(ctx, u.status).into(), tone: tone_of(u), fields, to: link("user", &[("q", u.username.as_str()), ("router", u.router_id.as_str())]) });
    }
    ToolOutput { for_model: json!({ "matches": out }).to_string(), widgets, source: Some("Users".into()), step: format!("{step}: {username}") }
}

fn list_events(ctx: &ToolCtx, a: &Value) -> ToolOutput {
    let step = ctx.lang.t("Membaca event", "Reading events").to_string();
    if let Err(e) = need(ctx, Resource::Event, "Event", &step) {
        return e;
    }
    let names = Names::load(ctx);
    let router = match s(a, "router") {
        Some(w) => match names.resolve(&w) {
            Some(id) => Some(id),
            None => return err(step, format!("router '{w}' tidak ditemukan")),
        },
        None => None,
    };
    let kinds = match s(a, "kind") {
        Some(k) => match EventKind::parse(&k) {
            Some(k) => vec![k],
            None => return err(step, format!("jenis event '{k}' tidak dikenal")),
        },
        None => vec![],
    };
    let hours = n(a, "hours", 24, 720);
    let limit = n(a, "limit", 20, MAX_ROWS as i64) as usize;
    let q = EventQuery { router_id: router, kinds, username: s(a, "username"), since_ms: Some(ctx.now_ms - hours * 3_600_000), limit, ..Default::default() };
    let ev = ctx.store.lock().expect("lock").list_events(&q).unwrap_or_default();
    let rows: Vec<Value> = ev
        .iter()
        .map(|e| json!({ "kind": e.event.kind.as_str(), "username": e.event.username, "router": names.name(&e.router_id), "at": fmt_utc(e.event.at_ms), "address": ip(ctx, &e.event.address) }))
        .collect();
    let widget_rows = ev
        .iter()
        .map(|e| ListRow {
            primary: format!("{} {}", e.event.username.clone().unwrap_or_else(|| "—".into()), kind_label(ctx, e.event.kind)),
            secondary: Some(names.name(&e.router_id)),
            trailing: Some(ago(ctx, e.event.at_ms)),
            tone: Some(match e.event.kind { EventKind::AuthFailed | EventKind::RouterDown => AiTone::Bad, EventKind::Disconnected => AiTone::Warn, _ => AiTone::Good }),
            to: Some(link("event", &[("type", e.event.kind.as_str())])),
        })
        .collect();
    ToolOutput {
        for_model: json!({ "hours": hours, "returned": rows.len(), "events": rows }).to_string(),
        widgets: if ev.is_empty() { vec![] } else { vec![AiWidget::List { title: Some(ctx.lang.t("Event terbaru", "Latest events").into()), rows: widget_rows }] },
        source: Some(format!("Event ({hours} {})", ctx.lang.t("jam", "h"))),
        step: format!("{step} — {}", ev.len()),
    }
}

fn kind_label(ctx: &ToolCtx, k: EventKind) -> &'static str {
    match k {
        EventKind::AuthFailed => ctx.lang.t("gagal autentikasi", "auth failed"),
        EventKind::Authenticated => ctx.lang.t("terautentikasi", "authenticated"),
        EventKind::Connected => ctx.lang.t("terhubung", "connected"),
        EventKind::Disconnected => ctx.lang.t("terputus", "disconnected"),
        EventKind::RouterDown => ctx.lang.t("router down", "router down"),
        EventKind::RouterUp => ctx.lang.t("router up", "router up"),
    }
}

fn profiles(ctx: &ToolCtx, a: &Value) -> ToolOutput {
    let step = ctx.lang.t("Membaca profil PPP", "Reading PPP profiles").to_string();
    if let Err(e) = need(ctx, Resource::Profile, "Profil PPP", &step) {
        return e;
    }
    let names = Names::load(ctx);
    let router = s(a, "router").and_then(|w| names.resolve(&w));
    let (profiles, users) = {
        let st = ctx.store.lock().expect("lock");
        (st.list_profiles(router.as_deref(), false).unwrap_or_default(), if ctx.auth.authorize(ctx.caller, Resource::User, Level::Read).is_ok() { Some(st.list_users(router.as_deref(), false).unwrap_or_default()) } else { None })
    };
    let rows: Vec<Value> = profiles
        .iter()
        .take(MAX_ROWS)
        .map(|p| {
            let count = users.as_ref().map(|u| u.iter().filter(|x| x.router_id == p.router_id && x.profile == p.name).count());
            json!({ "name": p.name, "router": names.name(&p.router_id), "rate_limit": p.rate_limit, "users": count })
        })
        .collect();
    let widget_rows = profiles
        .iter()
        .take(MAX_ROWS)
        .map(|p| ListRow { primary: p.name.clone(), secondary: Some(names.name(&p.router_id)), trailing: p.rate_limit.clone(), tone: None, to: Some(link("profile", &[])) })
        .collect();
    ToolOutput { for_model: json!({ "profiles": rows }).to_string(), widgets: vec![AiWidget::List { title: Some(ctx.lang.t("Profil PPP", "PPP profiles").into()), rows: widget_rows }], source: Some("Profil PPP".into()), step }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn penyamaran_ip_dan_mac() {
        assert_eq!(mask_ip("100.64.1.25"), "100.64.•.•");
        assert_eq!(mask_ip("fd00::1"), "fd00::•");
        assert_eq!(mask_ip("bukan-ip"), "•");
        assert_eq!(mask_mac("AA:BB:CC:DD:EE:FF"), "AA:BB:••:••:••:••");
        assert_eq!(mask_mac("rusak"), "••");
    }

    #[test]
    fn format_waktu_utc() {
        assert_eq!(fmt_utc(0), "1970-01-01 00:00 UTC");
        assert_eq!(fmt_utc(1_795_075_890_000), "2026-11-19 08:11 UTC");
        assert_eq!(fmt_utc(951_782_400_000 + 3_600_000 * 5 + 60_000 * 7), "2000-02-29 05:07 UTC");
    }

    #[test]
    fn spesifikasi_alat_tidak_punya_alat_tulis() {
        for t in specs() {
            assert!(!["add", "edit", "delete", "remove", "write", "set_", "apply", "disable", "enable"].iter().any(|w| t.name.contains(w)), "{} terlihat seperti alat tulis", t.name);
            assert_eq!(t.parameters["type"], "object");
        }
    }
}
