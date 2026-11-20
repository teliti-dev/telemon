//! AI chat Telemon (ADR 0007, 0024): hanya membaca, dengan alat baca yang menegakkan izin penanya.
//! Klien model ada di `llm` (diganti model palsu di tes), alat di `tools`, loop di `agent`, dan layanan
//! (provider, anggaran, dasbor, riwayat) di `service`.

pub mod agent;
pub mod llm;
pub mod service;
pub mod tools;
pub mod types;

pub use llm::{Completion, HttpLlm, Llm, LlmConfig, LlmError, LlmFuture, Msg, ToolCall, ToolSpec, Usage};
pub use service::{AiError, AiService, Kind};
pub use tools::Lang;
pub use types::*;
