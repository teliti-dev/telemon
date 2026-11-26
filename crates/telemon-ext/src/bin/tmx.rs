//! Alat build paket extension (ADR 0028). Dipakai repo extension, bukan aplikasi.
//!
//! - `tmx keygen`                       cetak kunci privat (heksa) ke stdout; simpan di tempat aman
//! - `tmx pubkey <KUNCI_PRIVAT_HEKSA>`  cetak kunci publik (heksa)
//! - `tmx sign <BERKAS> --key-env NAMA`  cetak tanda tangan Ed25519 (heksa) atas isi berkas (katalog toko)
//! - `tmx verify <BERKAS> <TANDA_TANGAN_HEKSA> <KUNCI_PUBLIK_HEKSA>`  periksa tanda tangan; keluar 0 bila cocok
//! - `tmx catalog <FOLDER_PAKET> -o <catalog.json> --key-env NAMA`  buat katalog dari semua `*.tmx` di folder
//!   (tiap paket diperiksa dan harus bertanda tangan kunci itu) dan tulis `<catalog.json>.sig`
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

fn key_from_env(args: &[String]) -> Result<Option<String>, String> {
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if a == "--key-env" {
            let n = it.next().ok_or("--key-env butuh nama variabel")?;
            return std::env::var(n).map(Some).map_err(|_| format!("variabel {n} kosong"));
        }
    }
    Ok(None)
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
        Some("sign") => {
            let file = args.get(1).ok_or("pakai: tmx sign <berkas> --key-env NAMA")?;
            let key = key_from_env(&args[2..])?.ok_or("--key-env wajib")?;
            println!("{}", telemon_ext::sign_bytes(&key, &std::fs::read(file).map_err(|e| format!("{file}: {e}"))?)?);
            Ok(())
        }
        Some("verify") => {
            let (file, sig, key) = (args.get(1), args.get(2), args.get(3));
            let (Some(file), Some(sig), Some(key)) = (file, sig, key) else { return Err("pakai: tmx verify <berkas> <tanda-tangan> <kunci-publik>".into()) };
            if telemon_ext::verify_bytes(key, &std::fs::read(file).map_err(|e| format!("{file}: {e}"))?, sig) {
                eprintln!("cocok");
                Ok(())
            } else {
                Err("tanda tangan TIDAK cocok".into())
            }
        }
        Some("catalog") => {
            let dir = args.get(1).ok_or("pakai: tmx catalog <folder-paket> -o <catalog.json> --key-env NAMA")?;
            let mut out = None;
            let mut rest = Vec::new();
            let mut it = args[2..].iter();
            while let Some(a) = it.next() {
                if a == "-o" {
                    out = it.next().cloned();
                } else {
                    rest.push(a.clone());
                }
            }
            let out = out.ok_or("-o <catalog.json> wajib")?;
            let key = key_from_env(&rest)?.ok_or("--key-env wajib")?;
            let pubkey = telemon_ext::public_key_hex(&key).ok_or("kunci privat tidak valid")?;
            let keys = telemon_ext::TrustedKeys { official: vec![pubkey.clone()], publishers: vec![], allow_unsigned: false };
            let mut names: Vec<_> = std::fs::read_dir(dir).map_err(|e| format!("{dir}: {e}"))?.filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().to_string()).filter(|n| n.ends_with(".tmx")).collect();
            names.sort();
            let mut entries = Vec::new();
            for n in names {
                let bytes = std::fs::read(Path::new(dir).join(&n)).map_err(|e| format!("{n}: {e}"))?;
                let pkg = telemon_ext::Package::open(&bytes, &keys, &telemon_ext::Limits::default()).map_err(|e| format!("{n}: {e}"))?;
                if pkg.tier != telemon_ext::Tier::Official {
                    return Err(format!("{n}: bukan official"));
                }
                entries.push(telemon_ext::CatalogEntry { manifest: pkg.manifest, file: n, sha256: telemon_ext::sha256_hex(&bytes), size: bytes.len() as u64 });
            }
            let generated = std::env::var("TMX_GENERATED").unwrap_or_else(|_| "unknown".into());
            let cat = telemon_ext::Catalog { version: telemon_ext::CATALOG_VERSION, generated, extensions: entries };
            let json = serde_json::to_vec_pretty(&cat).map_err(|e| e.to_string())?;
            // Periksa dengan pembaca yang sama dengan aplikasi sebelum menulis.
            let sig = telemon_ext::sign_bytes(&key, &json)?;
            telemon_ext::open_catalog(&json, &sig, &[pubkey]).map_err(|e| format!("katalog tidak valid: {e}"))?;
            std::fs::write(&out, &json).map_err(|e| format!("{out}: {e}"))?;
            std::fs::write(format!("{out}.sig"), format!("{sig}\n")).map_err(|e| e.to_string())?;
            eprintln!("ditulis: {out} ({} extension) dan {out}.sig", cat.extensions.len());
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
        _ => Err("perintah: keygen | pubkey <kunci> | pack <folder> -o <out.tmx> [--key-env NAMA] | catalog <folder> -o <catalog.json> --key-env NAMA | sign <berkas> --key-env NAMA | verify <berkas> <ttd> <kunci>".into()),
    }
}

fn main() {
    if let Err(e) = run() {
        eprintln!("tmx: {e}");
        std::process::exit(1);
    }
}
