//! Katalog toko extension official (ADR 0028): `catalog.json` + `catalog.json.sig` (Ed25519 oleh kunci official).
//!
//! Katalog hanya memuat extension official. Tiap entri membawa manifest lengkap (untuk memeriksa kompatibilitas
//! dan dependensi tanpa mengunduh paket), nama berkas, SHA-256, dan ukuran. Berkas diunduh relatif terhadap
//! alamat katalog dan diperiksa ulang oleh `Package::open` (tanda tangan paket) setelah SHA-256 cocok.

use serde::{Deserialize, Serialize};

use crate::manifest::Manifest;
use crate::package::{sha256_hex, verify_bytes};
use crate::resolve::{Problem, host_compatible, install_order};

pub const CATALOG_VERSION: u32 = 1;
const MAX_ENTRIES: usize = 500;
const MAX_PACKAGE: u64 = 20 << 20;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Entry {
    pub manifest: Manifest,
    /// Nama berkas paket, relatif terhadap alamat katalog (tanpa folder).
    pub file: String,
    pub sha256: String,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Catalog {
    pub version: u32,
    pub generated: String,
    pub extensions: Vec<Entry>,
}

fn safe_file(f: &str) -> bool {
    !f.is_empty() && f.len() <= 120 && f.ends_with(".tmx") && f.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
}

/// Verifikasi tanda tangan terhadap salah satu kunci official, lalu urai dan periksa isinya.
pub fn open_catalog(bytes: &[u8], sig_hex: &str, official: &[String]) -> Result<Catalog, String> {
    if !official.iter().any(|k| verify_bytes(k, bytes, sig_hex)) {
        return Err("Tanda tangan katalog tidak cocok dengan kunci official".into());
    }
    let c: Catalog = serde_json::from_slice(bytes).map_err(|e| format!("Katalog tidak valid: {e}"))?;
    if c.version != CATALOG_VERSION {
        return Err(format!("Versi katalog {} tidak didukung", c.version));
    }
    if c.extensions.len() > MAX_ENTRIES {
        return Err("Katalog terlalu besar".into());
    }
    let mut seen = std::collections::HashSet::new();
    for e in &c.extensions {
        e.manifest.validate().map_err(|m| format!("{}: {m}", e.manifest.id))?;
        if !safe_file(&e.file) || e.sha256.len() != 64 || !e.sha256.chars().all(|c| c.is_ascii_hexdigit()) || e.size == 0 || e.size > MAX_PACKAGE {
            return Err(format!("Entri katalog tidak valid: {}", e.manifest.id));
        }
        if !seen.insert(e.manifest.id.clone()) {
            return Err(format!("Id ganda di katalog: {}", e.manifest.id));
        }
    }
    Ok(c)
}

impl Catalog {
    pub fn find(&self, id: &str) -> Option<&Entry> {
        self.extensions.iter().find(|e| e.manifest.id == id)
    }

    /// Entri yang harus dipasang (dependensi dulu, `id` terakhir) untuk Telemon `host` dengan extension terpasang
    /// `installed` (id, versi). Dependensi yang terpasang dan cocok dilewati; yang terpasang tetapi tak cocok atau
    /// tak ada di katalog menggagalkan rencana dengan alasannya (tidak ada pembaruan diam-diam).
    pub fn plan(&self, id: &str, host: &str, installed: &[(String, String)]) -> Result<Vec<&Entry>, Problem> {
        let root = self.find(id).ok_or_else(|| Problem::MissingDependency { id: id.to_string(), needs: "*".into() })?;
        if !host_compatible(&root.manifest, host) {
            return Err(Problem::TelemonIncompatible { needs: root.manifest.telemon.clone().unwrap_or_default(), have: host.to_string() });
        }
        let lookup = |dep: &str| self.find(dep).filter(|e| host_compatible(&e.manifest, host)).map(|e| e.manifest.clone());
        let order = install_order(&root.manifest, installed, &lookup)?;
        Ok(order.iter().filter_map(|i| self.find(i)).collect())
    }
}

impl Entry {
    /// Periksa bytes unduhan terhadap ukuran dan SHA-256 di katalog.
    pub fn check_download(&self, bytes: &[u8]) -> Result<(), String> {
        if bytes.len() as u64 != self.size {
            return Err(format!("Ukuran unduhan {} tidak sama dengan katalog ({})", bytes.len(), self.size));
        }
        if sha256_hex(bytes) != self.sha256.to_ascii_lowercase() {
            return Err("SHA-256 unduhan tidak cocok dengan katalog".into());
        }
        Ok(())
    }
}
