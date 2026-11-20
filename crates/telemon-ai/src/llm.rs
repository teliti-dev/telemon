//! Klien model bahasa. Satu antarmuka (`Llm`) agar agen bisa diuji dengan model palsu.
//! `HttpLlm` bicara dengan dua keluarga API: kompatibel OpenAI (OpenAI, DeepSeek, Gemini, 9Router) dan
//! Anthropic Messages. Pemanggilan HTTP memakai `ureq` (blocking) di `spawn_blocking`; kunci API hanya dipakai
//! sebagai header dan tidak pernah masuk pesan galat.

use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use serde_json::{Value, json};

use crate::types::ProviderKind;

#[derive(Debug, Clone, PartialEq)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub args: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Msg {
    User(String),
    Assistant { text: String, tool_calls: Vec<ToolCall> },
    ToolResult { call_id: String, content: String },
}

#[derive(Debug, Clone, PartialEq)]
pub struct ToolSpec {
    pub name: &'static str,
    pub description: &'static str,
    /// JSON Schema objek parameter.
    pub parameters: Value,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Usage {
    pub input: i64,
    pub output: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Completion {
    pub text: String,
    pub tool_calls: Vec<ToolCall>,
    pub usage: Usage,
}

#[derive(Debug, Clone)]
pub struct LlmConfig {
    pub kind: ProviderKind,
    pub base_url: String,
    pub api_key: String,
    pub model: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LlmError(pub String);

impl std::fmt::Display for LlmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

pub type LlmFuture<'a> = Pin<Box<dyn Future<Output = Result<Completion, LlmError>> + Send + 'a>>;

pub trait Llm: Send + Sync {
    fn complete<'a>(&'a self, cfg: &'a LlmConfig, system: &'a str, msgs: &'a [Msg], tools: &'a [ToolSpec]) -> LlmFuture<'a>;
}

/// Alamat dasar bawaan penyedia resmi; 9Router wajib memberi alamat sendiri.
pub fn default_base_url(kind: ProviderKind) -> Option<&'static str> {
    match kind {
        ProviderKind::Openai => Some("https://api.openai.com/v1"),
        ProviderKind::Deepseek => Some("https://api.deepseek.com/v1"),
        ProviderKind::Gemini => Some("https://generativelanguage.googleapis.com/v1beta/openai"),
        ProviderKind::Anthropic => Some("https://api.anthropic.com/v1"),
        ProviderKind::Ninerouter => None,
    }
}

pub struct HttpLlm {
    timeout: Duration,
}

impl Default for HttpLlm {
    fn default() -> Self {
        Self { timeout: Duration::from_secs(90) }
    }
}

impl HttpLlm {
    pub fn with_timeout(timeout: Duration) -> Self {
        Self { timeout }
    }
}

impl Llm for HttpLlm {
    fn complete<'a>(&'a self, cfg: &'a LlmConfig, system: &'a str, msgs: &'a [Msg], tools: &'a [ToolSpec]) -> LlmFuture<'a> {
        let (url, headers, body) = build_request(cfg, system, msgs, tools);
        let timeout = self.timeout;
        let kind = cfg.kind;
        Box::pin(async move {
            let raw = tokio::task::spawn_blocking(move || post(&url, &headers, &body, timeout)).await.map_err(|e| LlmError(format!("tugas gagal: {e}")))??;
            parse_response(kind, &raw)
        })
    }
}

fn post(url: &str, headers: &[(String, String)], body: &[u8], timeout: Duration) -> Result<String, LlmError> {
    let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(timeout)).http_status_as_error(false).build().into();
    let mut req = agent.post(url).header("content-type", "application/json");
    for (k, v) in headers {
        req = req.header(k.as_str(), v.as_str());
    }
    let mut resp = req.send(body).map_err(|e| LlmError(format!("tidak bisa menghubungi penyedia: {}", clean(&e.to_string()))))?;
    let status = resp.status().as_u16();
    let text = resp.body_mut().read_to_string().map_err(|e| LlmError(format!("membaca jawaban gagal: {}", clean(&e.to_string()))))?;
    if (200..300).contains(&status) {
        Ok(text)
    } else {
        Err(LlmError(format!("penyedia menjawab {status}: {}", provider_message(&text))))
    }
}

/// Pesan galat dari JSON penyedia, dipendekkan; bila bukan JSON, potongan awal teksnya.
fn provider_message(body: &str) -> String {
    let v: Option<Value> = serde_json::from_str(body).ok();
    let msg = v
        .as_ref()
        .and_then(|v| v.pointer("/error/message").or_else(|| v.pointer("/error")).or_else(|| v.pointer("/message")))
        .and_then(|m| m.as_str().map(str::to_string))
        .unwrap_or_else(|| body.to_string());
    clean(&msg)
}

fn clean(s: &str) -> String {
    let one_line: String = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if one_line.chars().count() > 300 { format!("{}…", one_line.chars().take(300).collect::<String>()) } else { one_line }
}

fn is_anthropic(kind: ProviderKind) -> bool {
    kind == ProviderKind::Anthropic
}

pub(crate) fn build_request(cfg: &LlmConfig, system: &str, msgs: &[Msg], tools: &[ToolSpec]) -> (String, Vec<(String, String)>, Vec<u8>) {
    let base = cfg.base_url.trim_end_matches('/');
    if is_anthropic(cfg.kind) {
        let messages: Vec<Value> = anthropic_messages(msgs);
        let mut body = json!({ "model": cfg.model, "max_tokens": 2048, "system": system, "messages": messages });
        if !tools.is_empty() {
            body["tools"] = Value::Array(tools.iter().map(|t| json!({ "name": t.name, "description": t.description, "input_schema": t.parameters })).collect());
        }
        (
            format!("{base}/messages"),
            vec![("x-api-key".into(), cfg.api_key.clone()), ("anthropic-version".into(), "2023-06-01".into())],
            serde_json::to_vec(&body).expect("json"),
        )
    } else {
        let mut messages = vec![json!({ "role": "system", "content": system })];
        for m in msgs {
            messages.push(match m {
                Msg::User(t) => json!({ "role": "user", "content": t }),
                Msg::Assistant { text, tool_calls } if tool_calls.is_empty() => json!({ "role": "assistant", "content": text }),
                Msg::Assistant { text, tool_calls } => json!({
                    "role": "assistant",
                    "content": if text.is_empty() { Value::Null } else { Value::String(text.clone()) },
                    "tool_calls": tool_calls.iter().map(|c| json!({ "id": c.id, "type": "function", "function": { "name": c.name, "arguments": c.args.to_string() } })).collect::<Vec<_>>(),
                }),
                Msg::ToolResult { call_id, content } => json!({ "role": "tool", "tool_call_id": call_id, "content": content }),
            });
        }
        let mut body = json!({ "model": cfg.model, "messages": messages });
        if !tools.is_empty() {
            body["tools"] = Value::Array(
                tools.iter().map(|t| json!({ "type": "function", "function": { "name": t.name, "description": t.description, "parameters": t.parameters } })).collect(),
            );
        }
        (format!("{base}/chat/completions"), vec![("authorization".into(), format!("Bearer {}", cfg.api_key))], serde_json::to_vec(&body).expect("json"))
    }
}

/// Anthropic: hasil alat harus dikirim sebagai blok `tool_result` dalam pesan `user`, berurutan dengan `tool_use`.
fn anthropic_messages(msgs: &[Msg]) -> Vec<Value> {
    let mut out: Vec<Value> = Vec::new();
    for m in msgs {
        match m {
            Msg::User(t) => out.push(json!({ "role": "user", "content": t })),
            Msg::Assistant { text, tool_calls } => {
                let mut blocks: Vec<Value> = Vec::new();
                if !text.is_empty() {
                    blocks.push(json!({ "type": "text", "text": text }));
                }
                for c in tool_calls {
                    blocks.push(json!({ "type": "tool_use", "id": c.id, "name": c.name, "input": c.args }));
                }
                if blocks.is_empty() {
                    blocks.push(json!({ "type": "text", "text": "…" }));
                }
                out.push(json!({ "role": "assistant", "content": blocks }));
            }
            Msg::ToolResult { call_id, content } => {
                let block = json!({ "type": "tool_result", "tool_use_id": call_id, "content": content });
                match out.last_mut() {
                    Some(last) if last["role"] == "user" && last["content"].is_array() => last["content"].as_array_mut().expect("array").push(block),
                    _ => out.push(json!({ "role": "user", "content": [block] })),
                }
            }
        }
    }
    out
}

pub(crate) fn parse_response(kind: ProviderKind, raw: &str) -> Result<Completion, LlmError> {
    let v: Value = serde_json::from_str(raw).map_err(|_| LlmError("jawaban penyedia bukan JSON".into()))?;
    if is_anthropic(kind) {
        let mut text = String::new();
        let mut calls = Vec::new();
        for b in v["content"].as_array().into_iter().flatten() {
            match b["type"].as_str() {
                Some("text") => text.push_str(b["text"].as_str().unwrap_or("")),
                Some("tool_use") => calls.push(ToolCall { id: b["id"].as_str().unwrap_or("").into(), name: b["name"].as_str().unwrap_or("").into(), args: b["input"].clone() }),
                _ => {}
            }
        }
        let usage = Usage { input: v["usage"]["input_tokens"].as_i64().unwrap_or(0), output: v["usage"]["output_tokens"].as_i64().unwrap_or(0) };
        Ok(Completion { text, tool_calls: calls, usage })
    } else {
        let msg = v.pointer("/choices/0/message").ok_or_else(|| LlmError("jawaban penyedia tidak berisi pesan".into()))?;
        let text = msg["content"].as_str().unwrap_or("").to_string();
        let calls = msg["tool_calls"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|c| ToolCall {
                id: c["id"].as_str().unwrap_or("").into(),
                name: c.pointer("/function/name").and_then(Value::as_str).unwrap_or("").into(),
                // Argumen datang sebagai teks JSON; rusak/kosong dianggap tanpa argumen.
                args: c.pointer("/function/arguments").and_then(Value::as_str).and_then(|a| serde_json::from_str(a).ok()).unwrap_or_else(|| json!({})),
            })
            .collect();
        let usage = Usage { input: v["usage"]["prompt_tokens"].as_i64().unwrap_or(0), output: v["usage"]["completion_tokens"].as_i64().unwrap_or(0) };
        Ok(Completion { text, tool_calls: calls, usage })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(kind: ProviderKind) -> LlmConfig {
        LlmConfig { kind, base_url: "https://x.test/v1/".into(), api_key: "RAHASIA".into(), model: "m1".into() }
    }
    fn spec() -> Vec<ToolSpec> {
        vec![ToolSpec { name: "get_summary", description: "d", parameters: json!({"type":"object","properties":{}}) }]
    }

    #[test]
    fn permintaan_openai_memuat_alat_dan_hasil_alat() {
        let msgs = vec![
            Msg::User("halo".into()),
            Msg::Assistant { text: String::new(), tool_calls: vec![ToolCall { id: "c1".into(), name: "get_summary".into(), args: json!({"a":1}) }] },
            Msg::ToolResult { call_id: "c1".into(), content: "{}".into() },
        ];
        let (url, headers, body) = build_request(&cfg(ProviderKind::Openai), "SYS", &msgs, &spec());
        assert_eq!(url, "https://x.test/v1/chat/completions");
        assert_eq!(headers, [("authorization".to_string(), "Bearer RAHASIA".to_string())]);
        let b: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(b["messages"][0]["role"], "system");
        assert_eq!(b["messages"][2]["tool_calls"][0]["function"]["arguments"], "{\"a\":1}");
        assert_eq!(b["messages"][3]["role"], "tool");
        assert_eq!(b["tools"][0]["function"]["name"], "get_summary");
        assert!(!String::from_utf8_lossy(&body).contains("RAHASIA"), "kunci tidak boleh masuk badan permintaan");
    }

    #[test]
    fn permintaan_anthropic_menggabungkan_hasil_alat_dalam_pesan_user() {
        let msgs = vec![
            Msg::User("halo".into()),
            Msg::Assistant { text: "cek".into(), tool_calls: vec![ToolCall { id: "t1".into(), name: "a".into(), args: json!({}) }, ToolCall { id: "t2".into(), name: "b".into(), args: json!({}) }] },
            Msg::ToolResult { call_id: "t1".into(), content: "r1".into() },
            Msg::ToolResult { call_id: "t2".into(), content: "r2".into() },
        ];
        let (url, headers, body) = build_request(&cfg(ProviderKind::Anthropic), "SYS", &msgs, &spec());
        assert_eq!(url, "https://x.test/v1/messages");
        assert!(headers.iter().any(|(k, v)| k == "x-api-key" && v == "RAHASIA") && headers.iter().any(|(k, _)| k == "anthropic-version"));
        let b: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(b["system"], "SYS");
        assert_eq!(b["messages"].as_array().unwrap().len(), 3, "dua hasil alat berurutan menjadi satu pesan user");
        assert_eq!(b["messages"][2]["content"].as_array().unwrap().len(), 2);
        assert_eq!(b["tools"][0]["input_schema"]["type"], "object");
    }

    #[test]
    fn jawaban_openai_dan_anthropic_diurai() {
        let o = parse_response(
            ProviderKind::Deepseek,
            r#"{"choices":[{"message":{"content":"hai","tool_calls":[{"id":"c9","function":{"name":"find_users","arguments":"{\"limit\":3}"}}]}}],"usage":{"prompt_tokens":11,"completion_tokens":4}}"#,
        )
        .unwrap();
        assert_eq!((o.text.as_str(), o.usage), ("hai", Usage { input: 11, output: 4 }));
        assert_eq!(o.tool_calls[0].args["limit"], 3);
        let rusak = parse_response(ProviderKind::Openai, r#"{"choices":[{"message":{"tool_calls":[{"id":"x","function":{"name":"n","arguments":"bukan json"}}]}}]}"#).unwrap();
        assert_eq!(rusak.tool_calls[0].args, json!({}));
        let a = parse_response(
            ProviderKind::Anthropic,
            r#"{"content":[{"type":"text","text":"ok"},{"type":"tool_use","id":"u1","name":"get_summary","input":{"x":1}}],"usage":{"input_tokens":7,"output_tokens":2}}"#,
        )
        .unwrap();
        assert_eq!((a.text.as_str(), a.usage), ("ok", Usage { input: 7, output: 2 }));
        assert_eq!(a.tool_calls[0].name, "get_summary");
        assert!(parse_response(ProviderKind::Openai, "<html>").is_err());
        assert!(parse_response(ProviderKind::Openai, "{}").is_err());
    }

    #[test]
    fn pesan_galat_penyedia_dipendekkan_tanpa_kunci() {
        assert_eq!(provider_message(r#"{"error":{"message":"Invalid  API key"}}"#), "Invalid API key");
        assert!(provider_message(&"x".repeat(1000)).chars().count() <= 301);
    }
}
