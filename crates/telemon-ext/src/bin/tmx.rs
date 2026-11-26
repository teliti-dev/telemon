//! Alat build paket extension (ADR 0028). Dipakai repo extension, bukan aplikasi.
//!
//! - `tmx keygen`                       cetak kunci privat (heksa) ke stdout; simpan di tempat aman
//! - `tmx pubkey <KUNCI_PRIVAT_HEKSA>`  cetak kunci publik (heksa)
//! - `tmx pack <FOLDER> -o <OUT.tmx> [--key-env NAMA]`  kemas isi folder (manifest.json, ui/...);
//!   dengan `--key-env`, kunci privat dibaca dari variabel lingkungan itu (bukan argumen) dan paket ditandatangani.

use std::path::Path;

fn walk(base: &Path, dir: &Path, out: &mut Vec<(String, Vec<u8>)>) -> Result<(), String> {
    let mut entries: Vec<_> = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?.collect::<Result<_, _>>().map_err(|e| e.to_string())?;
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let p = e.path();
        let ft = e.file_type().map_err(|e| e.to_string())?;
        if ft.is_symlink() {
            return Err(format!("tautan simbolik tidak boleh: {}", p.display()));
        }
        if ft.is_dir() {
            walk(base, &p, out)?;
        } else {
            let rel = p.strip_prefix(base).map_err(|e| e.to_string())?.to_string_lossy().replace('\\', "/");
            out.push((rel, std::fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))?));
        }
    }
    Ok(())
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("keygen") => {
            let mut k = [0u8; 32];
            getrandom::fill(&mut k).map_err(|e| e.to_string())?;
            println!("{}", k.iter().map(|b| format!("{b:02x}")).collect::<String>());
            Ok(())
        }
        Some("pubkey") => {
            let sec = args.get(1).ok_or("pakai: tmx pubkey <kunci-privat-heksa>")?;
            println!("{}", telemon_ext::public_key_hex(sec).ok_or("kunci privat harus 64 karakter heksa")?);
            Ok(())
        }
        Some("pack") => {
            let dir = args.get(1).ok_or("pakai: tmx pack <folder> -o <out.tmx> [--key-env NAMA]")?;
            let mut out = None;
            let mut key_env = None;
            let mut it = args[2..].iter();
            while let Some(a) = it.next() {
                match a.as_str() {
                    "-o" => out = it.next().cloned(),
                    "--key-env" => key_env = it.next().cloned(),
                    other => return Err(format!("argumen tidak dikenal: {other}")),
                }
            }
            let out = out.ok_or("-o <out.tmx> wajib")?;
            let key = match key_env {
                Some(n) => Some(std::env::var(&n).map_err(|_| format!("variabel {n} kosong"))?),
                None => None,
            };
            let mut files = Vec::new();
            walk(Path::new(dir), Path::new(dir), &mut files)?;
            let bytes = telemon_ext::pack(&files, key.as_deref())?;
            // Periksa hasilnya dengan pembaca yang sama sebelum menulis.
            let keys = telemon_ext::TrustedKeys { official: key.as_deref().and_then(telemon_ext::public_key_hex).into_iter().collect(), publishers: vec![], allow_unsigned: key.is_none() };
            telemon_ext::Package::open(&bytes, &keys, &telemon_ext::Limits::default()).map_err(|e| format!("paket tidak valid: {e}"))?;
            std::fs::write(&out, bytes).map_err(|e| format!("{out}: {e}"))?;
            eprintln!("ditulis: {out} ({} berkas{})", files.len(), if key.is_some() { ", bertanda tangan" } else { ", TANPA tanda tangan" });
            Ok(())
        }
        _ => Err("perintah: keygen | pubkey <kunci> | pack <folder> -o <out.tmx> [--key-env NAMA]".into()),
    }
}

fn main() {
    if let Err(e) = run() {
        eprintln!("tmx: {e}");
        std::process::exit(1);
    }
}
