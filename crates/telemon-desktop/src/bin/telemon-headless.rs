//! Menjalankan peluncur desktop **tanpa Tauri** (untuk uji dan mesin tanpa layar): mencetak alamat masuk,
//! lalu berjalan sampai Ctrl+C. Data di folder `--data` (bawaan `./telemon-data`), rahasia di berkas
//! `secrets.json` di folder itu (izin 0600).
//!
//! Pakai: `telemon-headless [--data DIR] [--static ../../dist] [--port 47831]`

use std::path::PathBuf;
use std::sync::Arc;

use telemon_desktop::secrets::FileSecrets;
use telemon_desktop::{Desktop, DesktopConfig, PREFERRED_PORT, TunnelChoice};

fn arg(name: &str) -> Option<String> {
    let a: Vec<String> = std::env::args().collect();
    a.iter().position(|x| x == name).and_then(|i| a.get(i + 1)).cloned()
}

#[tokio::main]
async fn main() {
    let dir = PathBuf::from(arg("--data").unwrap_or_else(|| "telemon-data".into()));
    let secrets = FileSecrets::open(dir.join("secrets.json")).unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(1)
    });
    let cfg = DesktopConfig {
        db_path: dir.join("telemon.db"),
        static_dir: arg("--static").map(PathBuf::from),
        secrets: Arc::new(secrets),
        port: arg("--port").and_then(|p| p.parse().ok()).unwrap_or(PREFERRED_PORT),
        tunnel: if arg("--no-tunnel").is_some() { TunnelChoice::Off } else { TunnelChoice::Official },
    };
    let d = Desktop::start(cfg).await.unwrap_or_else(|e| {
        eprintln!("gagal memulai: {e}");
        std::process::exit(1)
    });
    println!("ENTRY {}", d.entry_url());
    let _ = tokio::signal::ctrl_c().await;
    d.shutdown().await;
}
