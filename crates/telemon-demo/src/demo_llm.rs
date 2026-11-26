//! Model bahasa palsu untuk server demo: memilih alat baca dari kata kunci pertanyaan (Indonesia dan English)
//! lalu merangkum hasilnya. Hanya untuk mencoba UI tanpa kunci API; tidak memanggil jaringan.

use serde_json::{Value, json};
use telemon_ai::{Completion, Llm, LlmConfig, LlmError, LlmFuture, Msg, ToolCall, ToolSpec, Usage};

pub struct DemoLlm;

/// Jenis widget yang disebut di kalimat (kata kunci demo), untuk usulan tata letak.
fn widget_type(q: &str) -> Option<&'static str> {
    let has = |words: &[&str]| words.iter().any(|w| q.contains(w));
    if has(&["event per jam", "grafik event"]) {
        Some("events-per-hour")
    } else if has(&["per user", "rata-rata per", "per pelanggan"]) {
        Some("bandwidth-per-user")
    } else if has(&["total bandwidth", "bandwidth total", "total pelanggan"]) {
        Some("bandwidth-total")
    } else if has(&["bandwidth", "grafik", "trafik"]) {
        Some("bandwidth-chart")
    } else if has(&["komposisi"]) {
        Some("user-status-mix")
    } else if has(&["profil"]) {
        Some("users-per-profile")
    } else {
        None
    }
}

fn is_layout_request(q: &str) -> bool {
    let has = |words: &[&str]| words.iter().any(|w| q.contains(w));
    has(&["tambah", "tambahkan", "hapus", "buang", "ubah", "ganti", "add ", "remove"]) && (has(&["widget", "grafik", "chart"]) || widget_type(q).is_some())
}

/// Langkah kedua untuk permintaan tata letak: dari `get_layout` ke `propose_layout_change` (hanya usulan, tidak menulis).
fn propose_from(layout: &Value, question: &str) -> (&'static str, Value) {
    let q = question.to_lowercase();
    let kind = widget_type(&q);
    let widgets = layout["widgets"].as_array().cloned().unwrap_or_default();
    let find = |k: &str| widgets.iter().find(|w| w["type"] == k).and_then(|w| w["id"].as_str().map(str::to_string));
    let range = ["1 jam", "6 jam", "24 jam", "7 hari", "30 hari"].iter().zip(["1h", "6h", "24h", "7d", "30d"]).find(|(w, _)| q.contains(*w)).map(|(_, r)| r);
    let change = if ["hapus", "buang", "remove"].iter().any(|w| q.contains(w)) {
        match kind.and_then(find) {
            Some(id) => json!({ "op": "remove", "id": id }),
            None => json!({ "op": "remove", "id": "tidak-ditemukan" }),
        }
    } else if ["ubah", "ganti"].iter().any(|w| q.contains(w)) {
        match kind.and_then(find) {
            Some(id) => json!({ "op": "configure", "id": id, "config": { "range": range.unwrap_or("24h") } }),
            None => json!({ "op": "configure", "id": "tidak-ditemukan", "config": {} }),
        }
    } else {
        let mut c = json!({ "op": "add", "type": kind.unwrap_or("bandwidth-chart") });
        // Router dan antarmuka yang disebut di kalimat (demo): model nyata mengisinya dari pertanyaan bebas.
        let router = [("rb-utara", "RB-Utara"), ("rb-selatan", "RB-Selatan")].iter().find(|(k, _)| q.contains(k)).map(|(_, v)| *v);
        let iface = ["ether1", "ether2", "bridge-lan"].iter().find(|i| q.contains(*i));
        let mut cfg = serde_json::Map::new();
        if let Some(r) = router {
            cfg.insert("router".into(), json!(r));
        }
        if let Some(i) = iface {
            cfg.insert("iface".into(), json!(i));
        }
        if let Some(r) = range {
            cfg.insert("range".into(), json!(r));
        }
        if !cfg.is_empty() {
            c["config"] = serde_json::Value::Object(cfg);
        }
        c
    };
    ("propose_layout_change", json!({ "changes": [change] }))
}

fn pick(q: &str) -> (&'static str, Value) {
    let q = q.to_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| q.contains(w));
    if is_layout_request(&q) {
        ("get_layout", json!({}))
    } else if has(&["statistik event", "analisis event", "tren event", "event per jam", "pola event", "paling sering"]) {
        ("event_stats", json!({ "hours": if has(&["minggu", "7 hari"]) { 168 } else { 24 } }))
    } else if has(&["kesehatan router", "downtime", "ketersediaan", "uptime"]) {
        ("router_health", json!({ "hours": 24 }))
    } else if has(&["bandwidth", "trafik", "kecepatan", "pemakaian internet", "unduh", "unggah"]) {
        ("bandwidth_summary", json!({ "range": if has(&["minggu", "7 hari"]) { "7d" } else if has(&["1 jam", "sejam"]) { "1h" } else { "24h" } }))
    } else if has(&["gagal", "auth", "failed", "salah password"]) {
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
    if v["status"] == "proposal_ready" {
        let n = v["changes"].as_array().map_or(0, Vec::len);
        return format!("Saya usulkan {n} perubahan (data demo). Belum ada yang berubah: periksa kartu usulan di atas lalu tekan Terapkan bila setuju.");
    }
    if let Some(n) = v["total_events"].as_i64() {
        return format!("Ada {n} event dalam rentang itu (data demo). Statistik, grafik, dan daftar teratas ada di kartu di atas.");
    }
    if v["download"].is_object() || v["in"].is_object() {
        let key = if v["download"].is_object() { "download" } else { "in" };
        return format!("Rata-rata {} {}, puncak {} (data demo). Rinciannya di kartu di atas.", key, v[key]["average"].as_str().unwrap_or("-"), v[key]["peak"].as_str().unwrap_or("-"));
    }
    if v["downtime_visible"].is_boolean() {
        return "Kesehatan router sudah dihitung (data demo); lihat kartu di atas.".into();
    }
    if v["available_types"].is_array() {
        return "Widget yang ada sudah terbaca (data demo).".into();
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
            Some(Msg::ToolResult { content, .. }) => {
                let v: Value = serde_json::from_str(content).unwrap_or(Value::Null);
                if v["available_types"].is_array() && v["error"].is_null() {
                    // Dari get_layout ke usulan: butuh pertanyaan asli di riwayat.
                    let q = msgs.iter().rev().find_map(|m| if let Msg::User(q) = m { Some(q.as_str()) } else { None }).unwrap_or("");
                    let (name, args) = propose_from(&v, q);
                    Ok(Completion { text: String::new(), tool_calls: vec![ToolCall { id: "demo-2".into(), name: name.into(), args }], usage: Usage { input: 400, output: 80 } })
                } else {
                    Ok(Completion { text: summarize(content), tool_calls: vec![], usage: Usage { input: 420, output: 60 } })
                }
            }
            Some(Msg::User(q)) => {
                let (name, args) = pick(q);
                Ok(Completion { text: String::new(), tool_calls: vec![ToolCall { id: "demo-1".into(), name: name.into(), args }], usage: Usage { input: 380, output: 30 } })
            }
            _ => Err(LlmError("tidak ada pertanyaan".into())),
        };
        Box::pin(async move { out })
    }
}
