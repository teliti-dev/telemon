//! Kompatibilitas (ADR 0028): versi Telemon dan antar extension. Murni: tidak menyentuh penyimpanan.

use serde::Serialize;

use crate::manifest::Manifest;
use crate::version::{Range, Version};

/// Satu masalah yang menghalangi pemasangan atau perubahan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Problem {
    /// Versi Telemon di luar rentang yang dinyatakan paket.
    TelemonIncompatible { needs: String, have: String },
    /// Extension yang dibutuhkan belum terpasang.
    MissingDependency { id: String, needs: String },
    /// Terpasang tetapi versinya di luar rentang.
    IncompatibleDependency { id: String, needs: String, have: String },
    /// Memperbarui/menghapus ini akan melanggar extension lain yang bergantung padanya.
    BreaksDependent { id: String, needs: String },
    /// Rantai dependensi melingkar.
    Cycle { id: String },
}

/// Status satu syarat untuk tampilan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DepStatus {
    pub id: String,
    pub needs: String,
    pub optional: bool,
    /// `ok`, `missing`, atau `incompatible`.
    pub state: &'static str,
    pub installed: Option<String>,
}

fn host_ok(m: &Manifest, host: &str) -> Option<Problem> {
    let range = m.telemon.as_deref()?;
    let r = Range::parse(range).ok()?;
    let have = Version::parse(host)?;
    (!r.matches(have)).then(|| Problem::TelemonIncompatible { needs: range.to_string(), have: have.to_string() })
}

/// `false` bila paket menyatakan rentang `telemon` dan versi host di luarnya (dipakai saat memuat: Telemon
/// diperbarui ke versi yang tidak lagi didukung extension terpasang).
pub fn host_compatible(m: &Manifest, host: &str) -> bool {
    host_ok(m, host).is_none()
}

/// Status tiap syarat `m.requires` terhadap extension terpasang `(id, versi)`.
pub fn dep_status(m: &Manifest, installed: &[(String, String)]) -> Vec<DepStatus> {
    m.requires
        .iter()
        .map(|r| {
            let have = installed.iter().find(|(id, _)| *id == r.id).map(|(_, v)| v.clone());
            let state = match (&have, Range::parse(&r.version)) {
                (None, _) => "missing",
                (Some(v), Ok(range)) => {
                    if Version::parse(v).is_some_and(|v| range.matches(v)) { "ok" } else { "incompatible" }
                }
                (Some(_), Err(_)) => "incompatible",
            };
            DepStatus { id: r.id.clone(), needs: r.version.clone(), optional: r.optional, state, installed: have }
        })
        .collect()
}

/// Masalah bila `m` dipasang (atau diperbarui) pada keadaan sekarang. `installed` = semua extension
/// terpasang selain `m` sendiri; `others` = manifest mereka (untuk memeriksa yang bergantung pada `m`).
pub fn check_install(m: &Manifest, host: &str, installed: &[(String, String)], others: &[Manifest]) -> Vec<Problem> {
    let mut out = Vec::new();
    out.extend(host_ok(m, host));
    for d in dep_status(m, installed) {
        if d.optional && d.state == "missing" {
            continue;
        }
        match d.state {
            "missing" => out.push(Problem::MissingDependency { id: d.id, needs: d.needs }),
            "incompatible" => out.push(Problem::IncompatibleDependency { id: d.id, needs: d.needs, have: d.installed.unwrap_or_default() }),
            _ => {}
        }
    }
    // Yang sudah terpasang dan bergantung pada `m`: versi baru tidak boleh melanggar rentangnya.
    let new_v = Version::parse(&m.version);
    for o in others {
        for r in o.requires.iter().filter(|r| r.id == m.id) {
            let ok = Range::parse(&r.version).ok().zip(new_v).is_some_and(|(rg, v)| rg.matches(v));
            if !ok {
                out.push(Problem::BreaksDependent { id: o.id.clone(), needs: r.version.clone() });
            }
        }
    }
    // Lingkaran langsung (A butuh B, B butuh A) dan lingkaran melalui yang terpasang.
    if has_cycle(m, others) {
        out.push(Problem::Cycle { id: m.id.clone() });
    }
    out
}

/// Extension yang wajib bergantung pada `id` (tanpa yang opsional).
pub fn dependents_of<'a>(id: &str, all: &'a [Manifest]) -> Vec<&'a Manifest> {
    all.iter().filter(|o| o.id != id && o.requires.iter().any(|r| r.id == id && !r.optional)).collect()
}

fn has_cycle(root: &Manifest, others: &[Manifest]) -> bool {
    fn walk(id: &str, root: &str, others: &[Manifest], seen: &mut Vec<String>) -> bool {
        if seen.iter().any(|s| s == id) {
            return false;
        }
        seen.push(id.to_string());
        let Some(m) = others.iter().find(|o| o.id == id) else { return false };
        m.requires.iter().filter(|r| !r.optional).any(|r| r.id == root || walk(&r.id, root, others, seen))
    }
    let mut seen = Vec::new();
    root.requires.iter().filter(|r| !r.optional).any(|r| r.id == root.id || walk(&r.id, &root.id, others, &mut seen))
}

/// Urutan pasang yang memasang dependensi lebih dulu. `lookup(id)` mengembalikan manifest yang tersedia
/// (dari toko atau paket yang diunggah bersama) untuk id yang belum terpasang. Hasil: id yang harus
/// dipasang, dependensi dulu, `root` terakhir. Gagal bila ada yang tidak tersedia atau melingkar.
pub fn install_order(root: &Manifest, installed: &[(String, String)], lookup: &dyn Fn(&str) -> Option<Manifest>) -> Result<Vec<String>, Problem> {
    fn visit(m: &Manifest, installed: &[(String, String)], lookup: &dyn Fn(&str) -> Option<Manifest>, stack: &mut Vec<String>, out: &mut Vec<String>) -> Result<(), Problem> {
        if stack.contains(&m.id) {
            return Err(Problem::Cycle { id: m.id.clone() });
        }
        stack.push(m.id.clone());
        for r in m.requires.iter().filter(|r| !r.optional) {
            let satisfied = installed.iter().find(|(id, _)| *id == r.id).is_some_and(|(_, v)| Range::parse(&r.version).ok().zip(Version::parse(v)).is_some_and(|(rg, v)| rg.matches(v)));
            if satisfied || out.contains(&r.id) {
                continue;
            }
            if installed.iter().any(|(id, _)| *id == r.id) {
                let have = installed.iter().find(|(id, _)| *id == r.id).map(|(_, v)| v.clone()).unwrap_or_default();
                return Err(Problem::IncompatibleDependency { id: r.id.clone(), needs: r.version.clone(), have });
            }
            let dep = lookup(&r.id).ok_or_else(|| Problem::MissingDependency { id: r.id.clone(), needs: r.version.clone() })?;
            if !Range::parse(&r.version).ok().zip(Version::parse(&dep.version)).is_some_and(|(rg, v)| rg.matches(v)) {
                return Err(Problem::IncompatibleDependency { id: r.id.clone(), needs: r.version.clone(), have: dep.version });
            }
            visit(&dep, installed, lookup, stack, out)?;
        }
        stack.pop();
        if !out.contains(&m.id) {
            out.push(m.id.clone());
        }
        Ok(())
    }
    let mut out = Vec::new();
    visit(root, installed, lookup, &mut Vec::new(), &mut out)?;
    Ok(out)
}
