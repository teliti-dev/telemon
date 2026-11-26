//! `manifest.json` di akar paket.

use serde::{Deserialize, Serialize};

use crate::API_VERSION;
use crate::version::Range;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Manifest {
    /// Pengenal unik gaya domain terbalik, huruf kecil: `net.teliti.backup`.
    pub id: String,
    pub name: String,
    /// Semver `X.Y.Z` (tanpa praterbit pada prototipe ini).
    pub version: String,
    pub api_version: u32,
    pub publisher: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub ui: Option<UiDecl>,
    #[serde(default)]
    pub contributes: Contributes,
    #[serde(default)]
    pub permissions: Vec<PermissionDecl>,
    /// Rentang versi Telemon yang didukung, mis. `>=0.2.0 <0.4.0`. Kosong = tidak dibatasi.
    #[serde(default)]
    pub telemon: Option<String>,
    /// Extension lain yang dibutuhkan (dengan rentang versinya).
    #[serde(default)]
    pub requires: Vec<Requirement>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Requirement {
    pub id: String,
    /// Rentang versi, mis. `^1.2.0`.
    pub version: String,
    /// Opsional: tidak wajib terpasang; kalau ada, versinya tetap harus cocok.
    #[serde(default)]
    pub optional: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiDecl {
    /// Modul ES di dalam paket, mis. `ui/index.js`.
    pub entry: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Contributes {
    #[serde(default)]
    pub widgets: Vec<WidgetDecl>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WidgetDecl {
    /// Jenis widget lokal (huruf kecil, angka, `-`); jenis penuh di Ringkasan = `ext:<id>:<type>`.
    #[serde(rename = "type")]
    pub kind: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PermissionDecl {
    /// Mis. `data.events`, `data.traffic`, `feature.ai`.
    pub id: String,
    /// Tanpa izin ini extension tidak bisa bekerja (jadi terbatas bila dimatikan).
    #[serde(default)]
    pub required: bool,
    /// Sensitif: mulai mati dan harus dinyalakan sendiri oleh admin.
    #[serde(default)]
    pub sensitive: bool,
    #[serde(default)]
    pub description: String,
}

/// Huruf kecil, angka, `-`, `_`, dan `.`; tidak diawali atau diakhiri titik; maks 64.
pub fn valid_id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && !s.starts_with('.')
        && !s.ends_with('.')
        && !s.contains("..")
        && s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '-' | '_' | '.'))
}

fn valid_semver(s: &str) -> bool {
    let p: Vec<&str> = s.split('.').collect();
    p.len() == 3 && p.iter().all(|x| !x.is_empty() && x.len() <= 6 && x.chars().all(|c| c.is_ascii_digit()))
}

impl Manifest {
    pub fn validate(&self) -> Result<(), String> {
        if !valid_id(&self.id) || !self.id.contains('.') {
            return Err("Id extension tidak valid (huruf kecil, gaya domain terbalik, mis. net.teliti.backup)".into());
        }
        if self.name.trim().is_empty() || self.name.len() > 80 {
            return Err("Nama extension kosong atau terlalu panjang".into());
        }
        if self.publisher.trim().is_empty() || self.publisher.len() > 80 {
            return Err("Penerbit kosong atau terlalu panjang".into());
        }
        if self.description.len() > 500 {
            return Err("Deskripsi terlalu panjang".into());
        }
        if !valid_semver(&self.version) {
            return Err("Versi harus X.Y.Z".into());
        }
        if self.api_version != API_VERSION {
            return Err(format!("apiVersion {} tidak didukung (build ini memahami {})", self.api_version, API_VERSION));
        }
        if let Some(ui) = &self.ui {
            if !ui.entry.starts_with("ui/") || !ui.entry.ends_with(".js") || !crate::package::safe_path(&ui.entry) {
                return Err("ui.entry harus berkas .js di dalam ui/".into());
            }
        }
        if self.contributes.widgets.len() > 20 || self.permissions.len() > 50 {
            return Err("Terlalu banyak widget atau izin".into());
        }
        let mut seen = std::collections::HashSet::new();
        for w in &self.contributes.widgets {
            if !valid_id(&w.kind) || w.kind.contains('.') || w.title.trim().is_empty() || w.title.len() > 60 || w.description.len() > 200 {
                return Err("Widget tidak valid".into());
            }
            if !seen.insert(w.kind.clone()) {
                return Err("Jenis widget ganda".into());
            }
        }
        if !self.contributes.widgets.is_empty() && self.ui.is_none() {
            return Err("Widget butuh ui.entry".into());
        }
        if let Some(t) = &self.telemon {
            Range::parse(t).map_err(|e| format!("telemon: {e}"))?;
        }
        if self.requires.len() > 20 {
            return Err("Terlalu banyak dependensi".into());
        }
        let mut seen = std::collections::HashSet::new();
        for r in &self.requires {
            if !valid_id(&r.id) || !r.id.contains('.') || r.id == self.id {
                return Err("Dependensi tidak valid (id salah atau mengacu ke diri sendiri)".into());
            }
            Range::parse(&r.version).map_err(|e| format!("requires {}: {e}", r.id))?;
            if !seen.insert(r.id.clone()) {
                return Err("Dependensi ganda".into());
            }
        }
        let mut seen = std::collections::HashSet::new();
        for p in &self.permissions {
            if !valid_id(&p.id) || p.description.len() > 200 {
                return Err("Izin tidak valid".into());
            }
            if !seen.insert(p.id.clone()) {
                return Err("Izin ganda".into());
            }
        }
        Ok(())
    }
}
