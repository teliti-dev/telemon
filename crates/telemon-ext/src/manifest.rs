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
    /// Backend WASM (ADR 0028): modul di paket yang menjawab rute API dan menjalankan worker.
    #[serde(default)]
    pub backend: Option<BackendDecl>,
    #[serde(default)]
    pub contributes: Contributes,
    #[serde(default)]
    pub permissions: Vec<PermissionDecl>,
    /// Aksi halus yang bisa diberikan per peran di halaman Akses (selain level menu).
    #[serde(default)]
    pub actions: Vec<ActionDecl>,
    /// Rentang versi Telemon yang didukung, mis. `>=0.2.0 <0.4.0`. Kosong = tidak dibatasi.
    #[serde(default)]
    pub telemon: Option<String>,
    /// Extension lain yang dibutuhkan (dengan rentang versinya).
    #[serde(default)]
    pub requires: Vec<Requirement>,
    /// Lisensi extension ini, ditentukan penerbitnya (bebas: gratis, open source, freemium, berbayar, ...). Telemon hanya
    /// menampilkannya; Telemon tidak menjual, memverifikasi, atau menegakkan lisensi extension (ADR 0030).
    #[serde(default)]
    pub license: Option<LicenseDecl>,
}

/// Jenis lisensi extension yang boleh dideklarasikan.
pub const LICENSE_KINDS: &[&str] = &["open-source", "free", "freemium", "paid", "trial", "proprietary"];

/// Lisensi sebuah extension, ditulis penerbitnya di manifest.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LicenseDecl {
    /// `open-source`, `free` (gratis, kode tertutup), `freemium`, `paid`, `trial` atau `proprietary` (syarat khusus).
    pub kind: String,
    /// Pengenal SPDX bila open source, mis. `MIT` atau `AGPL-3.0-only`. Wajib untuk `open-source`.
    #[serde(default)]
    pub spdx: Option<String>,
    /// Tautan https ke syarat lengkap (lisensi, harga, atau halaman penerbit).
    #[serde(default)]
    pub url: Option<String>,
    /// Ringkasan satu-dua kalimat untuk pemakai, mis. "Gratis sampai 5 router; Pro berbayar".
    #[serde(default)]
    pub summary: String,
}

impl LicenseDecl {
    fn validate(&self) -> Result<(), String> {
        if !LICENSE_KINDS.contains(&self.kind.as_str()) {
            return Err(format!("Jenis lisensi tidak dikenal: pilih salah satu dari {}", LICENSE_KINDS.join(", ")));
        }
        match self.spdx.as_deref() {
            None if self.kind == "open-source" => return Err("Lisensi open-source wajib memuat pengenal SPDX (mis. MIT)".into()),
            Some(x) if x.is_empty() || x.len() > 100 || !x.chars().all(|c| c.is_ascii_alphanumeric() || ".+-() ".contains(c)) => {
                return Err("Pengenal SPDX tidak valid".into());
            }
            _ => {}
        }
        if let Some(u) = self.url.as_deref() {
            if !u.starts_with("https://") || u.len() > 300 || u.chars().any(|c| c.is_whitespace() || c.is_control()) {
                return Err("Tautan lisensi harus https:// tanpa spasi (maks. 300 karakter)".into());
            }
        }
        if self.summary.chars().count() > 200 {
            return Err("Ringkasan lisensi terlalu panjang (maks. 200 karakter)".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActionDecl {
    pub id: String,
    pub label: String,
    /// Level menu minimal yang dibutuhkan peran agar aksi ini bisa diberikan: `read` (bawaan) atau `update`.
    #[serde(default = "read_level")]
    pub level: String,
    #[serde(default)]
    pub description: String,
}

fn read_level() -> String {
    "read".into()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PageDecl {
    /// Id halaman: `#/ext/<id extension>/<id halaman>`.
    pub id: String,
    pub title: String,
    /// `dashboard` (halaman utama extension) atau `page`.
    #[serde(default = "page_kind")]
    pub kind: String,
}

fn page_kind() -> String {
    "page".into()
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BackendDecl {
    /// Berkas `.wasm` di akar paket, mis. `backend.wasm`.
    pub entry: String,
}

/// Rute API extension: `/api/ext/<id><path>`; pencocokan persis (tanpa parameter jalur pada versi ini).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RouteDecl {
    pub id: String,
    /// GET, POST, PUT, atau DELETE.
    pub method: String,
    /// Diawali `/`, huruf kecil, angka, `-`, `_`, `/` (maks 80).
    pub path: String,
    /// Level menu minimal peran: `read` (bawaan) atau `update` (update tidak lewat akses Publik).
    #[serde(default = "read_level")]
    pub level: String,
    /// Aksi yang dideklarasikan di `actions[]` dan harus diberikan ke peran pemanggil.
    #[serde(default)]
    pub action: Option<String>,
}

/// Worker terjadwal: pemanggilan WASM singkat tiap `everySeconds`; dijeda otomatis setelah gagal berulang.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkerDecl {
    pub id: String,
    /// 10 sampai 86400.
    pub every_seconds: u64,
}

/// Sesi interaktif yang disediakan core untuk UI extension (aliran dua arah lewat WebSocket). Jenis `router-ssh`:
/// terminal SSH ke router (ADR 0029). Core yang memegang sambungan dan kredensial; extension hanya menampilkannya.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SessionDecl {
    pub id: String,
    /// Saat ini hanya `router-ssh`.
    pub kind: String,
    /// Aksi (level update) yang harus diberikan ke peran pemanggil.
    pub action: String,
}

/// Aksi pada satu router (menu aksi di halaman Router). UI extension mendaftarkan penanganannya dengan `registerRouterAction`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RouterActionDecl {
    pub id: String,
    pub title: String,
    /// Aksi (di `actions[]`) yang harus dimiliki peran agar entri ini tampil; kosong = semua yang punya akses ke extension.
    #[serde(default)]
    pub action: Option<String>,
}

/// Panel di bawah aplikasi (bertab seperti VS Code). UI extension mendaftarkan komponennya dengan `registerPanel`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PanelDecl {
    pub id: String,
    pub title: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Contributes {
    #[serde(default)]
    pub router_actions: Vec<RouterActionDecl>,
    #[serde(default)]
    pub panels: Vec<PanelDecl>,
    #[serde(default)]
    pub sessions: Vec<SessionDecl>,
    #[serde(default)]
    pub routes: Vec<RouteDecl>,
    #[serde(default)]
    pub workers: Vec<WorkerDecl>,
    #[serde(default)]
    pub widgets: Vec<WidgetDecl>,
    #[serde(default)]
    pub pages: Vec<PageDecl>,
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
        if let Some(l) = &self.license {
            l.validate()?;
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
        if (!self.contributes.widgets.is_empty() || !self.contributes.pages.is_empty()) && self.ui.is_none() {
            return Err("Widget dan halaman butuh ui.entry".into());
        }
        if let Some(b) = &self.backend {
            if b.entry != "backend.wasm" {
                return Err("backend.entry harus backend.wasm di akar paket".into());
            }
        }
        if !self.contributes.router_actions.is_empty() || !self.contributes.panels.is_empty() {
            if self.ui.is_none() {
                return Err("Aksi router dan panel butuh ui.entry".into());
            }
        }
        if self.contributes.router_actions.len() > 10 || self.contributes.panels.len() > 5 {
            return Err("Terlalu banyak aksi router atau panel".into());
        }
        let mut seen = std::collections::HashSet::new();
        for a in &self.contributes.router_actions {
            if !valid_id(&a.id) || a.id.contains('.') || a.title.trim().is_empty() || a.title.len() > 60 || !seen.insert(a.id.clone()) {
                return Err(format!("Aksi router tidak valid: {}", a.id));
            }
            if let Some(x) = &a.action {
                if !self.actions.iter().any(|d| d.id == *x) {
                    return Err(format!("Aksi router {}: aksi {x} tidak dideklarasikan", a.id));
                }
            }
        }
        let mut seen = std::collections::HashSet::new();
        for p in &self.contributes.panels {
            if !valid_id(&p.id) || p.id.contains('.') || p.title.trim().is_empty() || p.title.len() > 60 || !seen.insert(p.id.clone()) {
                return Err(format!("Panel tidak valid: {}", p.id));
            }
        }
        if self.contributes.sessions.len() > 5 {
            return Err("Terlalu banyak sesi".into());
        }
        for s in &self.contributes.sessions {
            if !valid_id(&s.id) || s.id.contains('.') || s.kind != "router-ssh" {
                return Err(format!("Sesi tidak valid: {}", s.id));
            }
            let decl = self.actions.iter().find(|a| a.id == s.action).ok_or_else(|| format!("Sesi {}: aksi {} tidak dideklarasikan", s.id, s.action))?;
            if decl.level != "update" {
                return Err(format!("Sesi {}: aksi {} harus berlevel update", s.id, s.action));
            }
            // Terminal = hak penuh akun router: izin harus dideklarasikan sensitif supaya mulai mati.
            if !self.permissions.iter().any(|p| p.id == "feature.router-ssh" && p.sensitive) {
                return Err(format!("Sesi {}: butuh izin feature.router-ssh bertanda sensitive", s.id));
            }
            if self.ui.is_none() {
                return Err("Sesi butuh ui.entry".into());
            }
        }
        if (!self.contributes.routes.is_empty() || !self.contributes.workers.is_empty()) && self.backend.is_none() {
            return Err("Rute dan worker butuh backend".into());
        }
        if self.contributes.routes.len() > 50 || self.contributes.workers.len() > 10 {
            return Err("Terlalu banyak rute atau worker".into());
        }
        let mut seen = std::collections::HashSet::new();
        for r in &self.contributes.routes {
            let path_ok = r.path.starts_with('/') && r.path.len() <= 80 && !r.path.contains("//") && r.path.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '-' | '_' | '/'));
            if !valid_id(&r.id) || r.id.contains('.') || !path_ok || !matches!(r.method.as_str(), "GET" | "POST" | "PUT" | "DELETE") || !matches!(r.level.as_str(), "read" | "update") {
                return Err(format!("Rute tidak valid: {}", r.id));
            }
            if r.method != "GET" && r.level != "update" {
                return Err(format!("Rute {} mengubah data: level harus update", r.id));
            }
            if let Some(a) = &r.action {
                let decl = self.actions.iter().find(|d| d.id == *a).ok_or_else(|| format!("Rute {}: aksi {a} tidak dideklarasikan", r.id))?;
                if decl.level == "update" && r.level != "update" {
                    return Err(format!("Rute {}: aksi {a} butuh level update", r.id));
                }
            }
            if !seen.insert((r.method.clone(), r.path.clone())) || !seen.insert((r.id.clone(), String::new())) {
                return Err(format!("Rute ganda: {}", r.id));
            }
        }
        let mut seen = std::collections::HashSet::new();
        for w in &self.contributes.workers {
            if !valid_id(&w.id) || w.id.contains('.') || !(10..=86_400).contains(&w.every_seconds) || !seen.insert(w.id.clone()) {
                return Err(format!("Worker tidak valid: {}", w.id));
            }
        }
        if self.contributes.pages.len() > 20 || self.actions.len() > 40 {
            return Err("Terlalu banyak halaman atau aksi".into());
        }
        let mut seen = std::collections::HashSet::new();
        for p in &self.contributes.pages {
            if !valid_id(&p.id) || p.id.contains('.') || p.title.trim().is_empty() || p.title.len() > 60 || !matches!(p.kind.as_str(), "dashboard" | "page") {
                return Err("Halaman tidak valid".into());
            }
            if !seen.insert(p.id.clone()) {
                return Err("Id halaman ganda".into());
            }
        }
        if self.contributes.pages.iter().filter(|p| p.kind == "dashboard").count() > 1 {
            return Err("Hanya satu halaman dashboard".into());
        }
        let mut seen = std::collections::HashSet::new();
        for a in &self.actions {
            if !valid_id(&a.id) || a.label.trim().is_empty() || a.label.len() > 60 || a.description.len() > 200 || !matches!(a.level.as_str(), "read" | "update") {
                return Err("Aksi tidak valid".into());
            }
            if !seen.insert(a.id.clone()) {
                return Err("Id aksi ganda".into());
            }
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
