//! Berkas `.tmx`: zip berisi `manifest.json`, `ui/*`, dan `signature.json`.
//!
//! Tanda tangan menutup **seluruh isi**: ringkasan SHA-256 dari daftar (jalur, SHA-256 isi) terurut,
//! semua berkas kecuali `signature.json`. Diperiksa di setiap pemuatan, bukan hanya saat pasang.

use std::io::{Cursor, Read, Write};

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::manifest::Manifest;

const SIG_FILE: &str = "signature.json";
const MANIFEST_FILE: &str = "manifest.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    Official,
    Public,
}

#[derive(Debug, Clone)]
pub struct Limits {
    pub max_package: usize,
    pub max_files: usize,
    pub max_total: u64,
    pub max_file: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self { max_package: 20 << 20, max_files: 300, max_total: 50 << 20, max_file: 10 << 20 }
    }
}

/// Kunci yang dipercaya (heksa 64 karakter). `official` = kunci Teliti; `publishers` = penerbit yang
/// dipercaya admin. `allow_unsigned` hanya untuk pengembangan (ADR 0028): paket tanpa tanda tangan
/// valid diterima sebagai tingkat Public dan ditandai tidak bertanda tangan.
#[derive(Debug, Clone, Default)]
pub struct TrustedKeys {
    pub official: Vec<String>,
    pub publishers: Vec<String>,
    pub allow_unsigned: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub enum PackageError {
    TooLarge,
    NotZip,
    Unsafe(String),
    Limits(String),
    NoManifest,
    Manifest(String),
    BadSignature,
    Untrusted,
}

impl std::fmt::Display for PackageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooLarge => write!(f, "Paket terlalu besar"),
            Self::NotZip => write!(f, "Bukan berkas .tmx yang valid"),
            Self::Unsafe(p) => write!(f, "Jalur berkas tidak aman: {p}"),
            Self::Limits(m) => write!(f, "{m}"),
            Self::NoManifest => write!(f, "manifest.json tidak ada"),
            Self::Manifest(m) => write!(f, "Manifest tidak valid: {m}"),
            Self::BadSignature => write!(f, "Tanda tangan tidak cocok dengan isi paket"),
            Self::Untrusted => write!(f, "Paket tidak bertanda tangan kunci yang dipercaya"),
        }
    }
}

impl std::error::Error for PackageError {}

/// Paket yang sudah diperiksa.
#[derive(Debug)]
pub struct Package {
    pub manifest: Manifest,
    pub tier: Tier,
    /// Kunci penanda tangan (heksa) bila ada tanda tangan valid.
    pub key_id: Option<String>,
    pub signed: bool,
    /// Semua berkas selain `signature.json`, jalur → isi.
    pub files: Vec<(String, Vec<u8>)>,
}

#[derive(Serialize, Deserialize)]
struct SigFile {
    #[serde(rename = "keyId")]
    key_id: String,
    sig: String,
}

/// Jalur relatif aman: tanpa `..`, tanpa awalan `/`, tanpa `\`, tanpa NUL, tanpa segmen kosong.
pub(crate) fn safe_path(p: &str) -> bool {
    !p.is_empty()
        && p.len() <= 200
        && !p.starts_with('/')
        && !p.contains('\\')
        && !p.contains('\0')
        && !p.contains(':')
        && p.split('/').all(|s| !s.is_empty() && s != "." && s != "..")
}

fn digest(files: &[(String, Vec<u8>)]) -> [u8; 32] {
    let mut rows: Vec<(&str, [u8; 32])> = files.iter().filter(|(p, _)| p != SIG_FILE).map(|(p, b)| (p.as_str(), Sha256::digest(b).into())).collect();
    rows.sort_by(|a, b| a.0.cmp(b.0));
    let mut h = Sha256::new();
    for (p, d) in rows {
        h.update(p.as_bytes());
        h.update([0]);
        h.update(d);
    }
    h.finalize().into()
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn unhex<const N: usize>(s: &str) -> Option<[u8; N]> {
    if s.len() != N * 2 || !s.is_ascii() {
        return None;
    }
    let mut out = [0u8; N];
    for (i, o) in out.iter_mut().enumerate() {
        *o = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(out)
}

/// Kunci publik (heksa) dari kunci privat 32 byte (heksa).
pub fn public_key_hex(secret_hex: &str) -> Option<String> {
    let sk = SigningKey::from_bytes(&unhex::<32>(secret_hex)?);
    Some(hex(sk.verifying_key().as_bytes()))
}

impl Package {
    pub fn open(bytes: &[u8], keys: &TrustedKeys, limits: &Limits) -> Result<Self, PackageError> {
        if bytes.len() > limits.max_package {
            return Err(PackageError::TooLarge);
        }
        let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|_| PackageError::NotZip)?;
        if zip.len() > limits.max_files {
            return Err(PackageError::Limits(format!("Maksimal {} berkas dalam paket", limits.max_files)));
        }
        let mut files: Vec<(String, Vec<u8>)> = Vec::new();
        let mut sig: Option<SigFile> = None;
        let mut total = 0u64;
        for i in 0..zip.len() {
            let f = zip.by_index(i).map_err(|_| PackageError::NotZip)?;
            if f.is_dir() {
                continue;
            }
            let name = f.name().to_string();
            if !safe_path(&name) {
                return Err(PackageError::Unsafe(name));
            }
            // Ukuran yang diklaim dicek dulu, lalu pembacaan dibatasi (zip bomb).
            if f.size() > limits.max_file {
                return Err(PackageError::Limits(format!("Berkas {name} terlalu besar")));
            }
            total += f.size();
            if total > limits.max_total {
                return Err(PackageError::Limits("Isi paket terlalu besar setelah diekstrak".into()));
            }
            let mut buf = Vec::new();
            f.take(limits.max_file + 1).read_to_end(&mut buf).map_err(|_| PackageError::NotZip)?;
            if buf.len() as u64 > limits.max_file {
                return Err(PackageError::Limits(format!("Berkas {name} terlalu besar")));
            }
            if files.iter().any(|(p, _)| *p == name) || (name == SIG_FILE && sig.is_some()) {
                return Err(PackageError::Unsafe(format!("{name} (ganda)")));
            }
            if name == SIG_FILE {
                sig = serde_json::from_slice(&buf).ok();
                if sig.is_none() {
                    return Err(PackageError::BadSignature);
                }
            } else {
                files.push((name, buf));
            }
        }
        let raw = files.iter().find(|(p, _)| p == MANIFEST_FILE).map(|(_, b)| b.clone()).ok_or(PackageError::NoManifest)?;
        let manifest: Manifest = serde_json::from_slice(&raw).map_err(|e| PackageError::Manifest(e.to_string()))?;
        manifest.validate().map_err(PackageError::Manifest)?;
        if let Some(ui) = &manifest.ui {
            if !files.iter().any(|(p, _)| *p == ui.entry) {
                return Err(PackageError::Manifest(format!("{} tidak ada di paket", ui.entry)));
            }
        }

        let (tier, key_id, signed) = match sig {
            Some(s) => {
                let pk = unhex::<32>(&s.key_id).and_then(|b| VerifyingKey::from_bytes(&b).ok()).ok_or(PackageError::BadSignature)?;
                let sg = unhex::<64>(&s.sig).map(|b| Signature::from_bytes(&b)).ok_or(PackageError::BadSignature)?;
                pk.verify(&digest(&files), &sg).map_err(|_| PackageError::BadSignature)?;
                if keys.official.iter().any(|k| *k == s.key_id) {
                    (Tier::Official, Some(s.key_id), true)
                } else if keys.publishers.iter().any(|k| *k == s.key_id) {
                    (Tier::Public, Some(s.key_id), true)
                } else if keys.allow_unsigned {
                    (Tier::Public, Some(s.key_id), false)
                } else {
                    return Err(PackageError::Untrusted);
                }
            }
            None if keys.allow_unsigned => (Tier::Public, None, false),
            None => return Err(PackageError::Untrusted),
        };
        Ok(Self { manifest, tier, key_id, signed, files })
    }

    pub fn file(&self, path: &str) -> Option<&[u8]> {
        self.files.iter().find(|(p, _)| p == path).map(|(_, b)| b.as_slice())
    }
}

/// Buat paket dari berkas (alat build dan tes). `secret_hex` = kunci privat 32 byte heksa; `None` = tanpa tanda tangan.
pub fn pack(files: &[(String, Vec<u8>)], secret_hex: Option<&str>) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    {
        let mut z = zip::ZipWriter::new(Cursor::new(&mut out));
        let opt = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        for (p, b) in files {
            if !safe_path(p) || p == SIG_FILE {
                return Err(format!("Jalur tidak boleh: {p}"));
            }
            z.start_file(p, opt).map_err(|e| e.to_string())?;
            z.write_all(b).map_err(|e| e.to_string())?;
        }
        if let Some(sec) = secret_hex {
            let sk = SigningKey::from_bytes(&unhex::<32>(sec).ok_or("Kunci privat harus 64 karakter heksa")?);
            let sig = sk.sign(&digest(files));
            let body = serde_json::to_vec(&SigFile { key_id: hex(sk.verifying_key().as_bytes()), sig: hex(&sig.to_bytes()) }).map_err(|e| e.to_string())?;
            z.start_file(SIG_FILE, opt).map_err(|e| e.to_string())?;
            z.write_all(&body).map_err(|e| e.to_string())?;
        }
        z.finish().map_err(|e| e.to_string())?;
    }
    Ok(out)
}
