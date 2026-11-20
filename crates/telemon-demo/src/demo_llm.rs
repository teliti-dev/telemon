//! Model bahasa palsu untuk server demo: memilih alat baca dari kata kunci pertanyaan (Indonesia dan English)
//! lalu merangkum hasilnya. Hanya untuk mencoba UI tanpa kunci API; tidak memanggil jaringan.

use serde_json::{Value, json};
use telemon_ai::{Completion, Llm, LlmConfig, LlmError, LlmFuture, Msg, ToolCall, ToolSpec, Usage};

pub struct DemoLlm;

fn pick(q: &str) -> (&'static str, Value) {
    let q = q.to_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| q.contains(w));
    if has(&["gagal", "auth", "failed", "salah password"]) {
        ("list_events", json!({ "kind": "auth_failed", "hours": 24 }))
    } else if has(&["tidak aktif", "inactive", "offline", "mati"]) {
        ("find_users", json!({ "status": "inactive", "limit": 10 }))
    } else if has(&["nonaktif", "disabled", "dinonaktifkan"]) {
        ("find_users", json!({ "status": "disabled", "limit": 10 }))
    } else if has(&["router"]) {
        ("list_routers", json!({}))
    } else if has(&["profil", "profile", "paket"]) {
        ("list_profiles", json!({}))
    } else if has(&["event", "kejadian", "terputus", "disconnect"]) {
        ("list_events", json!({ "hours": 24, "limit": 10 }))
    } else if has(&["user", "pengguna", "pelanggan", "siapa"]) {
        ("find_users", json!({ "limit": 10 }))
    } else {
        ("get_summary", json!({}))
    }
}

fn summarize(result: &str) -> String {
    let v: Value = serde_json::from_str(result).unwrap_or(Value::Null);
    if let Some(e) = v["error"].as_str() {
        return format!("Saya tidak bisa memakai data itu: {e}.");
    }
    if let Some(n) = v["total_matched"].as_i64() {
        return format!("Ketemu {n} user yang cocok (data demo). Daftarnya ada di kartu di atas.");
    }
    if let Some(a) = v["events"].as_array() {
        return format!("Ada {} event dalam rentang itu (data demo). Rinciannya di kartu di atas.", a.len());
    }
    if let Some(a) = v["routers"].as_array() {
        return format!("Ada {} router terpantau (data demo).", a.len());
    }
    if let Some(a) = v["profiles"].as_array() {
        return format!("Ada {} profil PPP (data demo).", a.len());
    }
    if let Some(u) = v["users"]["total"].as_i64() {
        return format!("Ringkasan (data demo): {u} user, {} aktif, {} tidak aktif.", v["users"]["active"], v["users"]["inactive"]);
    }
    "Selesai (data demo).".into()
}

impl Llm for DemoLlm {
    fn complete<'a>(&'a self, _cfg: &'a LlmConfig, _system: &'a str, msgs: &'a [Msg], _tools: &'a [ToolSpec]) -> LlmFuture<'a> {
        let out = match msgs.last() {
            Some(Msg::ToolResult { content, .. }) => Ok(Completion { text: summarize(content), tool_calls: vec![], usage: Usage { input: 420, output: 60 } }),
            Some(Msg::User(q)) => {
                let (name, args) = pick(q);
                Ok(Completion { text: String::new(), tool_calls: vec![ToolCall { id: "demo-1".into(), name: name.into(), args }], usage: Usage { input: 380, output: 30 } })
            }
            _ => Err(LlmError("tidak ada pertanyaan".into())),
        };
        Box::pin(async move { out })
    }
}
