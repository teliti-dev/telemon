//! Layanan AI: provider, anggaran, dasbor pemakaian, riwayat chat, dan pertanyaan. Tidak bergantung pada HTTP;
//! `telemon-http` hanya membungkusnya menjadi rute.

use std::collections::HashMap;
use std::hash::{BuildHasher, Hasher};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use telemon_core::auth::{Auth, Level, Resource, Session};
use telemon_core::{ChatRow, ProviderRow, SecretStore, SharedStore, UsageRow};

use crate::agent::{self, Run};
use crate::llm::{Llm, LlmConfig, Msg, Usage, default_base_url};
use crate::tools::{Lang, ToolCtx};
use crate::types::*;

const DAY_MS: i64 = 86_400_000;
const MAX_CHAT_BYTES: usize = 512 * 1024;
const MAX_CHATS_PER_ACCOUNT: usize = 200;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    Invalid,
    NotFound,
    Forbidden,
    /// AI mati, belum ada provider, atau anggaran habis.
    Unavailable,
    Upstream,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiError {
    pub kind: Kind,
    pub message: String,
}

impl std::fmt::Display for AiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

fn e(kind: Kind, m: impl Into<String>) -> AiError {
    AiError { kind, message: m.into() }
}

pub type Res<T> = Result<T, AiError>;

fn db<T>(r: rusqlite_free::R<T>) -> Res<T> {
    r.map_err(|x| e(Kind::Invalid, format!("penyimpanan: {x}")))
}

/// Alias tipis agar modul ini tidak bergantung langsung pada rusqlite.
mod rusqlite_free {
    pub type R<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;
}

pub struct AiService {
    store: SharedStore,
    secrets: Arc<dyn SecretStore>,
    auth: Arc<Auth>,
    llm: Arc<dyn Llm>,
    clock: Arc<dyn Fn() -> i64 + Send + Sync>,
}

fn system_now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

fn new_id() -> String {
    let a = std::collections::hash_map::RandomState::new().build_hasher().finish();
    let b = std::collections::hash_map::RandomState::new().build_hasher().finish();
    format!("{a:016x}{b:016x}")
}

fn secret_key(id: &str) -> String {
    format!("ai.key.{id}")
}

fn key_hint(key: &str) -> String {
    let n = key.chars().count();
    if n >= 12 { format!("{}…{}", key.chars().take(3).collect::<String>(), key.chars().skip(n - 4).collect::<String>()) } else { format!("…{}", key.chars().skip(n.saturating_sub(2)).collect::<String>()) }
}

fn valid_url(u: &str) -> bool {
    (u.starts_with("http://") || u.starts_with("https://")) && !u.contains(char::is_whitespace) && u.len() <= 300 && !u.contains('@')
}

fn to_provider(r: ProviderRow) -> AiProvider {
    AiProvider {
        id: r.id,
        name: r.name,
        kind: ProviderKind::parse(&r.kind).unwrap_or(ProviderKind::Openai),
        model: r.model,
        base_url: r.base_url,
        key_hint: r.key_hint,
        status: r.status,
        error: r.error,
        latency_ms: r.latency_ms,
        checked_at: r.checked_at,
        is_default: r.is_default,
        balance_usd: None,
        price_in_per_mtok: r.price_in,
        price_out_per_mtok: r.price_out,
    }
}

// ---------------------------------------------------------------- tanggal

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Awal bulan UTC yang memuat `ms` dan awal bulan berikutnya.
pub fn month_bounds(ms: i64) -> (i64, i64) {
    let (y, m, _) = civil_from_days(ms.div_euclid(DAY_MS));
    let start = days_from_civil(y, m, 1) * DAY_MS;
    let (ny, nm) = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
    (start, days_from_civil(ny, nm, 1) * DAY_MS)
}

// ---------------------------------------------------------------- agregasi dasbor

pub struct Aggregate {
    pub usage: Vec<AiUsageDay>,
    pub by_model: Vec<AiUsageRow>,
    pub by_account: Vec<AiUsageRow>,
    pub error_rate: f64,
    pub p50_ms: i64,
}

pub fn aggregate(rows: &[UsageRow], days: i64, now: i64, label: &dyn Fn(&str) -> String) -> Aggregate {
    let today = now - now.rem_euclid(DAY_MS);
    let first = today - (days - 1) * DAY_MS;
    let mut usage: Vec<AiUsageDay> = (0..days).map(|i| AiUsageDay { day: first + i * DAY_MS, input_tokens: 0, output_tokens: 0, cost_usd: 0.0, requests: 0 }).collect();
    let mut by_model: HashMap<String, AiUsageRow> = HashMap::new();
    let mut by_account: HashMap<String, AiUsageRow> = HashMap::new();
    let mut lat: Vec<i64> = vec![];
    let (mut total, mut failed) = (0i64, 0i64);
    for r in rows.iter().filter(|r| r.at >= first) {
        if let Some(d) = usage.iter_mut().find(|d| r.at >= d.day && r.at < d.day + DAY_MS) {
            d.input_tokens += r.input_tokens;
            d.output_tokens += r.output_tokens;
            d.cost_usd += r.cost_usd;
            d.requests += 1;
        }
        for (map, key, lbl) in [(&mut by_model, r.model.clone(), r.model.clone()), (&mut by_account, r.account.clone(), label(&r.account))] {
            let row = map.entry(key.clone()).or_insert(AiUsageRow { key, label: lbl, requests: 0, input_tokens: 0, output_tokens: 0, cost_usd: 0.0 });
            row.requests += 1;
            row.input_tokens += r.input_tokens;
            row.output_tokens += r.output_tokens;
            row.cost_usd += r.cost_usd;
        }
        total += 1;
        if !r.ok {
            failed += 1;
        } else {
            lat.push(r.latency_ms);
        }
    }
    lat.sort_unstable();
    let sorted = |m: HashMap<String, AiUsageRow>| {
        let mut v: Vec<AiUsageRow> = m.into_values().collect();
        v.sort_by(|a, b| b.cost_usd.partial_cmp(&a.cost_usd).unwrap_or(std::cmp::Ordering::Equal).then(b.requests.cmp(&a.requests)).then(a.key.cmp(&b.key)));
        v
    };
    Aggregate { usage, by_model: sorted(by_model), by_account: sorted(by_account), error_rate: if total == 0 { 0.0 } else { failed as f64 / total as f64 }, p50_ms: lat.get(lat.len() / 2).copied().unwrap_or(0) }
}

// ---------------------------------------------------------------- layanan

impl AiService {
    pub fn new(store: SharedStore, secrets: Arc<dyn SecretStore>, auth: Arc<Auth>, llm: Arc<dyn Llm>) -> Self {
        Self { store, secrets, auth, llm, clock: Arc::new(system_now) }
    }

    pub fn with_clock(mut self, clock: Arc<dyn Fn() -> i64 + Send + Sync>) -> Self {
        self.clock = clock;
        self
    }

    fn now(&self) -> i64 {
        (self.clock)()
    }

    fn with<T>(&self, f: impl FnOnce(&mut telemon_core::Store) -> rusqlite_free::R<T>) -> Res<T> {
        db(f(&mut self.store.lock().expect("lock")))
    }

    // ---- provider

    pub fn providers(&self) -> Res<Vec<AiProvider>> {
        Ok(self.with(|s| s.list_ai_providers().map_err(Into::into))?.into_iter().map(to_provider).collect())
    }

    fn provider_row(&self, id: &str) -> Res<ProviderRow> {
        self.with(|s| s.get_ai_provider(id).map_err(Into::into))?.ok_or_else(|| e(Kind::NotFound, "Provider tidak ditemukan"))
    }

    pub fn save_provider(&self, id: Option<&str>, input: AiProviderInput) -> Res<AiProvider> {
        let name = input.name.trim();
        let model = input.model.trim();
        if name.is_empty() || name.chars().count() > 60 {
            return Err(e(Kind::Invalid, "Nama provider wajib diisi (maksimal 60 karakter)"));
        }
        if model.is_empty() || model.chars().count() > 100 || model.contains(char::is_whitespace) {
            return Err(e(Kind::Invalid, "Nama model wajib diisi tanpa spasi (maksimal 100 karakter)"));
        }
        let base_url = input.base_url.as_deref().map(str::trim).filter(|u| !u.is_empty()).map(|u| u.trim_end_matches('/').to_string());
        if let Some(u) = &base_url
            && !valid_url(u)
        {
            return Err(e(Kind::Invalid, "URL dasar harus diawali http:// atau https:// dan tidak boleh memuat spasi atau kredensial"));
        }
        if input.kind == ProviderKind::Ninerouter && base_url.is_none() {
            return Err(e(Kind::Invalid, "9Router butuh URL dasar (mis. http://host:20128/v1)"));
        }
        for p in [input.price_in_per_mtok, input.price_out_per_mtok].into_iter().flatten() {
            if !(0.0..=100_000.0).contains(&p) {
                return Err(e(Kind::Invalid, "Harga per juta token harus 0 sampai 100000 USD"));
            }
        }
        let existing = match id {
            Some(id) => Some(self.provider_row(id)?),
            None => None,
        };
        let new_key = input.api_key.as_deref().map(str::trim).filter(|k| !k.is_empty());
        if existing.is_none() && new_key.is_none() {
            return Err(e(Kind::Invalid, "Kunci API wajib diisi"));
        }
        if let Some(k) = new_key
            && (k.len() > 400 || k.contains(char::is_whitespace))
        {
            return Err(e(Kind::Invalid, "Kunci API tidak valid"));
        }
        let pid = existing.as_ref().map(|r| r.id.clone()).unwrap_or_else(new_id);
        let hint = match new_key {
            Some(k) => {
                self.secrets.set(&secret_key(&pid), k).map_err(|m| e(Kind::Invalid, format!("menyimpan kunci gagal: {m}")))?;
                Some(key_hint(k))
            }
            None => existing.as_ref().and_then(|r| r.key_hint.clone()),
        };
        let key_changed = new_key.is_some();
        let changed = existing.as_ref().is_none_or(|r| key_changed || r.model != model || r.kind != input.kind.as_str() || r.base_url != base_url);
        let row = ProviderRow {
            id: pid,
            name: name.into(),
            kind: input.kind.as_str().into(),
            model: model.into(),
            base_url,
            key_hint: hint,
            // Perubahan penting membuat hasil uji lama tidak berlaku lagi.
            status: if changed { "unconfigured".into() } else { existing.as_ref().map(|r| r.status.clone()).unwrap_or_else(|| "unconfigured".into()) },
            error: if changed { None } else { existing.as_ref().and_then(|r| r.error.clone()) },
            latency_ms: if changed { None } else { existing.as_ref().and_then(|r| r.latency_ms) },
            checked_at: if changed { None } else { existing.as_ref().and_then(|r| r.checked_at) },
            is_default: existing.as_ref().is_some_and(|r| r.is_default),
            price_in: input.price_in_per_mtok,
            price_out: input.price_out_per_mtok,
            created_at: existing.as_ref().map(|r| r.created_at).unwrap_or_else(|| self.now()),
        };
        self.with(|s| s.save_ai_provider(&row).map_err(Into::into))?;
        Ok(to_provider(self.provider_row(&row.id)?))
    }

    pub fn remove_provider(&self, id: &str) -> Res<()> {
        if !self.with(|s| s.delete_ai_provider(id).map_err(Into::into))? {
            return Err(e(Kind::NotFound, "Provider tidak ditemukan"));
        }
        self.secrets.delete(&secret_key(id));
        Ok(())
    }

    pub fn set_default(&self, id: &str) -> Res<()> {
        if !self.with(|s| s.set_default_ai_provider(id).map_err(Into::into))? {
            return Err(e(Kind::NotFound, "Provider tidak ditemukan"));
        }
        Ok(())
    }

    fn llm_config(&self, p: &ProviderRow) -> Res<LlmConfig> {
        let kind = ProviderKind::parse(&p.kind).ok_or_else(|| e(Kind::Invalid, "Jenis provider tidak dikenal"))?;
        let api_key = self.secrets.get(&secret_key(&p.id)).ok_or_else(|| e(Kind::Unavailable, "Kunci API provider tidak ditemukan; masukkan ulang kuncinya"))?;
        let base_url = p.base_url.clone().or_else(|| default_base_url(kind).map(str::to_string)).ok_or_else(|| e(Kind::Invalid, "Provider tidak punya URL dasar"))?;
        Ok(LlmConfig { kind, base_url, api_key, model: p.model.clone() })
    }

    /// Uji koneksi: satu permintaan kecil tanpa alat. Hasilnya disimpan sebagai status provider.
    pub async fn test_provider(&self, id: &str) -> Res<AiProvider> {
        let p = self.provider_row(id)?;
        let cfg = self.llm_config(&p)?;
        let started = self.now();
        let out = self.llm.complete(&cfg, "Reply with the single word: ok", &[Msg::User("ping".into())], &[]).await;
        let latency = (self.now() - started).max(0);
        let at = self.now();
        match out {
            Ok(c) => {
                self.record_usage(&p, "system", c.usage, true, latency);
                self.with(|s| s.set_ai_provider_status(id, "ok", None, Some(latency), at).map_err(Into::into))?;
            }
            Err(err) => {
                self.record_usage(&p, "system", Usage::default(), false, latency);
                self.with(|s| s.set_ai_provider_status(id, "error", Some(&err.0), Some(latency), at).map_err(Into::into))?;
            }
        }
        Ok(to_provider(self.provider_row(id)?))
    }

    fn record_usage(&self, p: &ProviderRow, account: &str, u: Usage, ok: bool, latency_ms: i64) {
        let cost = u.input as f64 / 1e6 * p.price_in.unwrap_or(0.0) + u.output as f64 / 1e6 * p.price_out.unwrap_or(0.0);
        let row = UsageRow { at: self.now(), provider_id: p.id.clone(), model: p.model.clone(), account: account.into(), input_tokens: u.input, output_tokens: u.output, cost_usd: cost, ok, latency_ms };
        let _ = self.with(|s| s.add_ai_usage(&row).map_err(Into::into));
    }

    // ---- anggaran dan dasbor

    fn budget_raw(&self) -> AiBudgetInput {
        let v = self.store.lock().ok().and_then(|s| s.kv_get("ai.budget").ok().flatten());
        let parse = |t: &str| -> Option<AiBudgetInput> {
            let v: serde_json::Value = serde_json::from_str(t).ok()?;
            Some(AiBudgetInput { monthly_usd: v["monthlyUsd"].as_f64(), alert_percent: v["alertPercent"].as_u64().unwrap_or(80).clamp(1, 100) as u32, hard_stop: v["hardStop"].as_bool().unwrap_or(false) })
        };
        v.as_deref().and_then(parse).unwrap_or(AiBudgetInput { monthly_usd: None, alert_percent: 80, hard_stop: false })
    }

    pub fn budget(&self) -> Res<AiBudget> {
        let b = self.budget_raw();
        let now = self.now();
        let (start, next) = month_bounds(now);
        let spent: f64 = self.with(|s| s.ai_usage_since(start).map_err(Into::into))?.iter().map(|r| r.cost_usd).sum();
        Ok(AiBudget { monthly_usd: b.monthly_usd, alert_percent: b.alert_percent, hard_stop: b.hard_stop, spent_month_usd: spent, reset_at: next })
    }

    pub fn save_budget(&self, b: AiBudgetInput) -> Res<()> {
        if b.monthly_usd.is_some_and(|m| !(0.0..=1_000_000.0).contains(&m)) {
            return Err(e(Kind::Invalid, "Batas bulanan harus 0 sampai 1000000 USD"));
        }
        if !(1..=100).contains(&b.alert_percent) {
            return Err(e(Kind::Invalid, "Persen peringatan harus 1 sampai 100"));
        }
        let v = serde_json::json!({ "monthlyUsd": b.monthly_usd, "alertPercent": b.alert_percent, "hardStop": b.hard_stop });
        self.with(|s| s.kv_set("ai.budget", &v.to_string()).map_err(Into::into))
    }

    pub fn dashboard(&self, days: i64) -> Res<AiDashboard> {
        let days = if days >= 30 { 30 } else { 7 };
        let now = self.now();
        let rows = self.with(|s| s.ai_usage_since(now - days * DAY_MS).map_err(Into::into))?;
        let names: HashMap<String, String> = self.auth.list_accounts(None).map(|v| v.into_iter().map(|a| (a.id, a.username)).collect()).unwrap_or_default();
        let label = |acc: &str| match acc {
            "desktop" => "Desktop".to_string(),
            "system" => "(tes)".to_string(),
            other => names.get(other).cloned().unwrap_or_else(|| other.to_string()),
        };
        let agg = aggregate(&rows, days, now, &label);
        Ok(AiDashboard { providers: self.providers()?, budget: self.budget()?, usage: agg.usage, by_model: agg.by_model, by_account: agg.by_account, error_rate: agg.error_rate, p50_ms: agg.p50_ms })
    }

    /// Fitur siap dipakai: pemilik menyalakannya dan ada provider bawaan dengan kunci.
    pub fn ready(&self) -> bool {
        let enabled = self.store.lock().ok().and_then(|s| s.get_settings().ok()).map(|s| s.ai.enabled).unwrap_or(false);
        enabled && self.with(|s| s.default_ai_provider().map_err(Into::into)).ok().flatten().is_some_and(|p| self.secrets.get(&secret_key(&p.id)).is_some())
    }

    // ---- pertanyaan

    /// Jawab satu pertanyaan. `account` = id akun penanya (`desktop` untuk aplikasi desktop).
    pub async fn ask(&self, caller: Option<&Session>, account: &str, messages: &[AiMessage], lang: Lang, emit: &mut (dyn FnMut(AiStreamEvent) + Send)) -> Res<()> {
        self.auth.authorize(caller, Resource::Ai, Level::Read).map_err(|x| e(Kind::Forbidden, x.to_string()))?;
        let settings = self.store.lock().expect("lock").get_settings().map_err(|x| e(Kind::Invalid, x.to_string()))?;
        if !settings.ai.enabled {
            return Err(e(Kind::Unavailable, "AI chat dimatikan oleh pemilik"));
        }
        let provider = self.with(|s| s.default_ai_provider().map_err(Into::into))?.ok_or_else(|| e(Kind::Unavailable, "Belum ada provider AI; atur di Pengaturan"))?;
        let b = self.budget()?;
        if b.hard_stop && b.monthly_usd.is_some_and(|m| b.spent_month_usd >= m) {
            return Err(e(Kind::Unavailable, "Anggaran AI bulan ini sudah habis"));
        }
        let cfg = self.llm_config(&provider)?;
        let tools = ToolCtx { store: &self.store, auth: &self.auth, caller, mask: settings.ai.mask_sensitive, now_ms: self.now(), lang };
        let run = Run { llm: self.llm.as_ref(), cfg: &cfg, tools: &tools };
        let now = self.clock.clone();
        let mut record = |u: Usage, ok: bool, ms: i64| self.record_usage(&provider, account, u, ok, ms);
        agent::run(&run, messages, emit, &mut record, &*now).await.map_err(|m| e(Kind::Upstream, m))
    }

    // ---- riwayat chat (per akun)

    pub fn chats(&self, account: &str) -> Res<Vec<AiChatSummary>> {
        let rows = self.with(|s| s.list_ai_chats(account).map_err(Into::into))?;
        Ok(rows
            .into_iter()
            .map(|(m, json)| {
                let msgs: Vec<AiMessage> = serde_json::from_str(&json).unwrap_or_default();
                let preview = msgs.iter().rev().find(|x| x.role == ChatRole::Assistant && !x.text.is_empty()).map(|x| x.text.chars().take(120).collect()).unwrap_or_default();
                AiChatSummary { id: m.id, title: m.title, pinned: m.pinned, updated_at: m.updated_at, message_count: msgs.len(), preview }
            })
            .collect())
    }

    pub fn chat(&self, account: &str, id: &str) -> Res<AiChat> {
        let r = self.with(|s| s.get_ai_chat(account, id).map_err(Into::into))?.ok_or_else(|| e(Kind::NotFound, "Percakapan tidak ditemukan"))?;
        Ok(AiChat { id: r.id, title: r.title, pinned: r.pinned, created_at: r.created_at, updated_at: r.updated_at, messages: serde_json::from_str(&r.messages).unwrap_or_default() })
    }

    pub fn save_chat(&self, account: &str, chat: &AiChat) -> Res<AiChatSummary> {
        let id_ok = !chat.id.is_empty() && chat.id.len() <= 64 && chat.id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
        if !id_ok {
            return Err(e(Kind::Invalid, "Id percakapan tidak valid"));
        }
        if chat.messages.len() > 500 {
            return Err(e(Kind::Invalid, "Percakapan terlalu panjang"));
        }
        let json = serde_json::to_string(&chat.messages).map_err(|x| e(Kind::Invalid, x.to_string()))?;
        if json.len() > MAX_CHAT_BYTES {
            return Err(e(Kind::Invalid, "Percakapan terlalu besar"));
        }
        let exists = self.with(|s| s.get_ai_chat(account, &chat.id).map_err(Into::into))?.is_some();
        if !exists && self.with(|s| s.list_ai_chats(account).map_err(Into::into))?.len() >= MAX_CHATS_PER_ACCOUNT {
            return Err(e(Kind::Invalid, "Terlalu banyak percakapan; hapus yang lama"));
        }
        let title: String = chat.title.trim().chars().take(120).collect();
        let now = self.now();
        let row = ChatRow { id: chat.id.clone(), account: account.into(), title: if title.is_empty() { "…".into() } else { title }, pinned: chat.pinned, created_at: if chat.created_at > 0 { chat.created_at } else { now }, updated_at: now, messages: json };
        if !self.with(|s| s.save_ai_chat(&row).map_err(Into::into))? {
            return Err(e(Kind::Forbidden, "Percakapan ini milik akun lain"));
        }
        self.chats(account)?.into_iter().find(|c| c.id == chat.id).ok_or_else(|| e(Kind::NotFound, "Percakapan tidak ditemukan"))
    }

    pub fn delete_chat(&self, account: &str, id: &str) -> Res<()> {
        if !self.with(|s| s.delete_ai_chat(account, id).map_err(Into::into))? {
            return Err(e(Kind::NotFound, "Percakapan tidak ditemukan"));
        }
        Ok(())
    }

    /// Bersihkan riwayat chat yang lebih lama dari `days` (retensi); `None` = selamanya.
    pub fn purge_chats(&self, days: Option<u32>) -> Res<u64> {
        let Some(d) = days else { return Ok(0) };
        let before = self.now() - d as i64 * DAY_MS;
        self.with(|s| s.purge_ai_chats(before).map_err(Into::into))
    }
}
