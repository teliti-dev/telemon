//! Bentuk JSON yang dipakai UI (`src/lib/types.ts`); nama kolom camelCase.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiLink {
    pub page: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<std::collections::BTreeMap<String, String>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AiTone {
    Good,
    Warn,
    Bad,
    Muted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatItem {
    pub label: String,
    pub value: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tone: Option<AiTone>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BarItem {
    pub label: String,
    pub value: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListRow {
    pub primary: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secondary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trailing: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tone: Option<AiTone>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<AiLink>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Field {
    pub label: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum AiWidget {
    Stats { items: Vec<StatItem> },
    Bars { title: String, data: Vec<BarItem> },
    List { #[serde(default, skip_serializing_if = "Option::is_none")] title: Option<String>, rows: Vec<ListRow> },
    User { username: String, router: String, status: String, tone: AiTone, fields: Vec<Field>, to: AiLink },
    /// Usulan perubahan tata letak Ringkasan (ADR 0027). Belum mengubah apa pun: UI menerapkannya hanya setelah
    /// pengguna menekan Terapkan, memakai izin pengguna itu sendiri.
    Proposal {
        ops: Vec<LayoutOp>,
        /// Diisi UI setelah pengguna memutuskan (`applied` atau `dismissed`), supaya tersimpan di riwayat chat.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        state: Option<String>,
    },
}

/// Satu perubahan tata letak yang diusulkan. `config` memakai id router (bukan nama) dan sudah divalidasi alat.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "lowercase")]
pub enum LayoutOp {
    Add {
        #[serde(rename = "type")]
        widget: String,
        #[serde(default)]
        config: serde_json::Map<String, serde_json::Value>,
    },
    Remove { id: String },
    Configure { id: String, config: serde_json::Map<String, serde_json::Value> },
}

/// Keadaan tampilan Ringkasan yang dikirim klien bersama pertanyaan, supaya AI tahu widget apa yang ada dan
/// jenis apa yang tersedia (daftar jenis dimiliki UI). Tidak dipercaya untuk hal lain: hanya dipakai membuat usulan
/// yang diperiksa ulang UI sebelum diterapkan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct UiContext {
    #[serde(default)]
    pub widgets: Vec<UiWidget>,
    #[serde(default)]
    pub catalog: Vec<UiWidgetType>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiWidget {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub config: Option<serde_json::Map<String, serde_json::Value>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiWidgetType {
    #[serde(rename = "type")]
    pub kind: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub configurable: bool,
}

impl UiContext {
    /// Batas bentuk dan ukuran (klien tidak dipercaya untuk ukuran).
    pub fn validate(&self) -> Result<(), String> {
        let ident = |s: &str| !s.is_empty() && s.len() <= 40 && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
        if self.widgets.len() > 80 || self.catalog.len() > 40 {
            return Err("Konteks tampilan terlalu besar".into());
        }
        for w in &self.widgets {
            if !ident(&w.id) || !ident(&w.kind) || w.config.as_ref().is_some_and(|c| serde_json::Value::Object(c.clone()).to_string().len() > 2048) {
                return Err("Widget pada konteks tampilan tidak valid".into());
            }
        }
        for t in &self.catalog {
            if !ident(&t.kind) || t.title.chars().count() > 120 || t.description.chars().count() > 400 {
                return Err("Jenis widget pada konteks tampilan tidak valid".into());
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiAction {
    pub id: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<AiLink>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copy: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChatRole {
    User,
    Assistant,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiMessage {
    pub id: String,
    pub role: ChatRole,
    pub text: String,
    pub at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub widgets: Option<Vec<AiWidget>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actions: Option<Vec<AiAction>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub follow_ups: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sources: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub steps: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feedback: Option<String>,
}

/// Potongan jawaban yang dialirkan (satu objek JSON per baris).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum AiStreamEvent {
    Step { text: String },
    Text { text: String },
    Widget { widget: AiWidget },
    Actions { actions: Vec<AiAction> },
    Followups { items: Vec<String> },
    Sources { items: Vec<String> },
    /// Tambahan server: jawaban gagal dibuat (UI menampilkan sebagai galat).
    Error { message: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiChat {
    pub id: String,
    pub title: String,
    pub pinned: bool,
    pub created_at: i64,
    pub updated_at: i64,
    pub messages: Vec<AiMessage>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiChatSummary {
    pub id: String,
    pub title: String,
    pub pinned: bool,
    pub updated_at: i64,
    pub message_count: usize,
    pub preview: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderKind {
    Ninerouter,
    Anthropic,
    Openai,
    Deepseek,
    Gemini,
}

impl ProviderKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ninerouter => "ninerouter",
            Self::Anthropic => "anthropic",
            Self::Openai => "openai",
            Self::Deepseek => "deepseek",
            Self::Gemini => "gemini",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "ninerouter" => Self::Ninerouter,
            "anthropic" => Self::Anthropic,
            "openai" => Self::Openai,
            "deepseek" => Self::Deepseek,
            "gemini" => Self::Gemini,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiProvider {
    pub id: String,
    pub name: String,
    pub kind: ProviderKind,
    pub model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key_hint: Option<String>,
    /// `ok` | `error` | `unconfigured`.
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checked_at: Option<i64>,
    pub is_default: bool,
    /// Penyedia umumnya tidak melaporkan saldo lewat API biasa: selalu null.
    pub balance_usd: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub price_in_per_mtok: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub price_out_per_mtok: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiProviderInput {
    pub name: String,
    pub kind: ProviderKind,
    pub model: String,
    #[serde(default)]
    pub base_url: Option<String>,
    /// Kosong saat edit = kunci tetap.
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub price_in_per_mtok: Option<f64>,
    #[serde(default)]
    pub price_out_per_mtok: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiUsageDay {
    pub day: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cost_usd: f64,
    pub requests: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiBudget {
    pub monthly_usd: Option<f64>,
    pub alert_percent: u32,
    pub hard_stop: bool,
    pub spent_month_usd: f64,
    pub reset_at: i64,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiBudgetInput {
    pub monthly_usd: Option<f64>,
    pub alert_percent: u32,
    pub hard_stop: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiUsageRow {
    pub key: String,
    pub label: String,
    pub requests: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cost_usd: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiDashboard {
    pub providers: Vec<AiProvider>,
    pub budget: AiBudget,
    pub usage: Vec<AiUsageDay>,
    pub by_model: Vec<AiUsageRow>,
    pub by_account: Vec<AiUsageRow>,
    pub error_rate: f64,
    pub p50_ms: i64,
}
