#![cfg(unix)]
//! Uji sidecar Cloudflare: unduhan terverifikasi (server HTTP lokal) dan pengelolaan proses (skrip pengganti).

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use sha2::{Digest, Sha256};
use telemon_desktop::cloudflared::{Cloudflared, Program, Source};
use telemon_http::Tunnel;

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("telemon-cf-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn sha(b: &[u8]) -> String {
    Sha256::digest(b).iter().map(|x| format!("{x:02x}")).collect()
}

/// Server HTTP sederhana yang menyajikan satu berkas dan menghitung permintaan.
fn serve(body: Vec<u8>) -> (String, Arc<AtomicUsize>) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/cloudflared", l.local_addr().unwrap());
    let hits = Arc::new(AtomicUsize::new(0));
    let h = hits.clone();
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            let mut s = s;
            let mut buf = [0u8; 2048];
            let _ = s.read(&mut buf);
            h.fetch_add(1, Ordering::SeqCst);
            let _ = s.write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()).as_bytes());
            let _ = s.write_all(&body);
        }
    });
    (url, hits)
}

const SCRIPT: &[u8] = b"#!/bin/sh\necho 'INF Requesting new quick Tunnel' >&2\necho 'INF |  https://acak-uji-123.trycloudflare.com  |' >&2\nexec sleep 60\n";

fn official(dir: &PathBuf, url: &str, sha256: &str, max: u64) -> Cloudflared {
    Cloudflared::new(dir.clone(), Program::Downloaded(Source { url: url.into(), sha256: sha256.into(), max_size: max, file_name: "cloudflared-uji".into() }))
}

#[tokio::test]
async fn unduhan_terverifikasi_dipasang_dan_dipakai_ulang() {
    let dir = tmp("dl");
    let (url, hits) = serve(SCRIPT.to_vec());
    let cf = official(&dir, &url, &sha(SCRIPT), 1 << 20);
    let (path, args) = cf.ensure().await.unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), SCRIPT);
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o755);
    }
    assert!(args.contains(&"tunnel".to_string()) && args.contains(&"--no-autoupdate".to_string()) && args.contains(&"http://127.0.0.1:{PORT}".to_string()));
    assert_eq!(hits.load(Ordering::SeqCst), 1);
    assert!(!path.with_extension("part").exists());
    // Sudah ada dan utuh: tidak diunduh lagi.
    cf.ensure().await.unwrap();
    assert_eq!(hits.load(Ordering::SeqCst), 1);
    // Berkas diubah (rusak/diganti): ditolak lalu diunduh ulang, bukan dijalankan.
    std::fs::write(&path, b"#!/bin/sh\necho jahat\n").unwrap();
    cf.ensure().await.unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), SCRIPT);
    assert_eq!(hits.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn checksum_salah_dan_ukuran_berlebih_ditolak_tanpa_sisa() {
    let dir = tmp("bad");
    let (url, _) = serve(SCRIPT.to_vec());
    let cf = official(&dir, &url, &sha(b"bukan ini"), 1 << 20);
    let e = cf.ensure().await.unwrap_err();
    assert!(e.contains("checksum tidak cocok"), "{e}");
    assert!(std::fs::read_dir(&dir).unwrap().next().is_none(), "tidak boleh ada berkas tersisa (termasuk .part)");

    let cf = official(&dir, &url, &sha(SCRIPT), 10);
    let e = cf.ensure().await.unwrap_err();
    assert!(e.contains("lebih besar"), "{e}");
    assert!(std::fs::read_dir(&dir).unwrap().next().is_none());

    // Server tidak terjangkau: galat jelas.
    let cf = official(&dir, "http://127.0.0.1:1/x", &sha(SCRIPT), 1 << 20);
    assert!(cf.ensure().await.unwrap_err().contains("mengunduh"));
}

fn sh(script: &str) -> Cloudflared {
    Cloudflared::new(tmp("sh"), Program::Custom("/bin/sh".into(), vec!["-c".into(), script.into(), "sh".into(), "{PORT}".into()])).with_url_timeout(Duration::from_secs(2))
}

#[tokio::test]
async fn menjalankan_membaca_url_dan_menghentikan() {
    // `$1` = port lokal (placeholder {PORT} diganti).
    let cf = sh("echo \"INF |  https://port-$1.trycloudflare.com  |\" >&2; exec sleep 60");
    assert!(!cf.alive().await, "belum jalan");
    let url = cf.start(4321).await.unwrap();
    assert_eq!(url, "https://port-4321.trycloudflare.com");
    assert!(cf.alive().await);
    cf.stop().await;
    assert!(!cf.alive().await);
    // Dapat dinyalakan lagi.
    assert!(cf.start(5555).await.unwrap().contains("5555"));
    cf.stop().await;
}

#[tokio::test]
async fn proses_mati_sebelum_url_atau_tanpa_url_gagal_jelas() {
    // Mati sebelum membuka tunnel; pesan galat memuat keluaran terakhir (bukan URL api.trycloudflare.com).
    let cf = sh("echo 'ERR failed to request quick Tunnel: Post \"https://api.trycloudflare.com/tunnel\": dial tcp: lookup api.trycloudflare.com' >&2; exit 1");
    let e = cf.start(1).await.unwrap_err();
    assert!(e.contains("berhenti sebelum membuka tunnel") && e.contains("lookup api.trycloudflare.com"), "{e}");
    assert!(!cf.alive().await);
    // Hidup tetapi tidak pernah mencetak URL: waktu habis, proses dibunuh.
    let cf = sh("echo 'INF sedang menunggu' >&2; exec sleep 60");
    let e = cf.start(1).await.unwrap_err();
    assert!(e.contains("tidak terbuka dalam") && e.contains("sedang menunggu"), "{e}");
    assert!(!cf.alive().await);
    // Program tidak ada.
    let cf = Cloudflared::new(tmp("none"), Program::Custom("/tidak/ada/cloudflared".into(), vec![]));
    assert!(cf.start(1).await.unwrap_err().contains("menjalankan"));
}

#[tokio::test]
async fn proses_yang_mati_setelah_start_terdeteksi() {
    let cf = sh("echo 'INF https://sebentar-saja.trycloudflare.com' >&2; sleep 1");
    cf.start(1).await.unwrap();
    assert!(cf.alive().await);
    tokio::time::sleep(Duration::from_millis(1500)).await;
    assert!(!cf.alive().await, "proses berhenti sendiri harus terdeteksi");
}
