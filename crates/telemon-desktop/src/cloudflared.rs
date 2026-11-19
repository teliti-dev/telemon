//! Cloudflare Quick Tunnel (ADR 0003, ADR 0022): menjalankan `cloudflared` resmi sebagai proses anak yang
//! mengarah ke server LAN, dan membaca URL `*.trycloudflare.com` dari keluarannya.
//!
//! Biner **diunduh otomatis saat pertama kali dibutuhkan** (pengguna tidak perlu memasang apa pun) dari rilis
//! resmi Cloudflare di GitHub, dengan versi dan SHA-256 yang dikunci di kode ini: berkas yang checksum-nya tidak
//! cocok ditolak dan dihapus. Platform tanpa pin tidak didukung (galat jelas).

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Mutex;
use std::time::Duration;

use sha2::{Digest, Sha256};
use telemon_http::Tunnel;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};

pub const VERSION: &str = "2026.11.1";
const RELEASE_BASE: &str = "https://github.com/cloudflare/cloudflared/releases/download";
/// Batas waktu sampai URL tunnel muncul di keluaran (setelah biner siap).
const URL_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Clone, Debug)]
pub struct Pin {
    pub asset: &'static str,
    pub sha256: &'static str,
    pub size: u64,
}

/// Berkas rilis resmi yang dikunci. Nilai diambil dari `digest` aset rilis di GitHub (bukan dari berkas unduhan).
pub fn pin_for(os: &str, arch: &str) -> Option<Pin> {
    Some(match (os, arch) {
        ("linux", "x86_64") => Pin { asset: "cloudflared-linux-amd64", sha256: "97bb3b02d23a5980fc9c15a1a270e692da1923f103f920d5b16d4931d5ab55e7", size: 40_346_301 },
        ("linux", "aarch64") => Pin { asset: "cloudflared-linux-arm64", sha256: "b155cec3e48e489cfb8e5c602769fb8c771083018d8881d68b4d18a791567c5a", size: 37_818_397 },
        ("windows", "x86_64") => Pin { asset: "cloudflared-windows-amd64.exe", sha256: "341deda3db5ec498a0482254d6c38c0b7cf01a30c37bb5dd95d2216d08955e35", size: 55_483_112 },
        _ => return None,
    })
}

/// Ambil URL tunnel dari satu baris log. `api.trycloudflare.com` (muncul di pesan galat permintaan) bukan tunnel.
pub fn parse_tunnel_url(line: &str) -> Option<String> {
    let start = line.find("https://")?;
    let rest = &line[start + "https://".len()..];
    let host_len = rest.find(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '.')).unwrap_or(rest.len());
    let host = &rest[..host_len];
    let sub = host.strip_suffix(".trycloudflare.com")?;
    if sub.is_empty() || sub == "api" || sub.contains('.') {
        return None;
    }
    Some(format!("https://{host}"))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn file_sha256(path: &Path) -> std::io::Result<String> {
    let mut f = std::fs::File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(hex(h.finalize().as_slice()))
}

/// Unduh `url` ke `dest` dengan batas ukuran, verifikasi SHA-256, lalu pasang atomik. Sinkron (jalankan di thread blocking).
pub fn download_verified(url: &str, dest: &Path, sha256: &str, max_size: u64) -> Result<(), String> {
    let part = dest.with_extension("part");
    let result = (|| {
        let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(900))).build().into();
        let mut resp = agent.get(url).call().map_err(|e| format!("mengunduh {url}: {e}"))?;
        let mut reader = resp.body_mut().as_reader();
        let mut out = std::fs::File::create(&part).map_err(|e| e.to_string())?;
        let mut hasher = Sha256::new();
        let (mut total, mut buf) = (0u64, [0u8; 64 * 1024]);
        loop {
            let n = reader.read(&mut buf).map_err(|e| format!("membaca unduhan: {e}"))?;
            if n == 0 {
                break;
            }
            total += n as u64;
            if total > max_size {
                return Err(format!("unduhan lebih besar dari {max_size} byte; dibatalkan"));
            }
            hasher.update(&buf[..n]);
            std::io::Write::write_all(&mut out, &buf[..n]).map_err(|e| e.to_string())?;
        }
        out.sync_all().map_err(|e| e.to_string())?;
        let got = hex(hasher.finalize().as_slice());
        if !got.eq_ignore_ascii_case(sha256) {
            return Err(format!("checksum tidak cocok (didapat {got}); berkas ditolak"));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&part, std::fs::Permissions::from_mode(0o755)).map_err(|e| e.to_string())?;
        }
        std::fs::rename(&part, dest).map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&part);
    }
    result
}

/// Sumber biner: dapat diganti di uji.
#[derive(Clone)]
pub struct Source {
    pub url: String,
    pub sha256: String,
    pub max_size: u64,
    pub file_name: String,
}

/// Perintah yang dijalankan: bawaan memakai biner unduhan; uji menggantinya dengan skrip.
#[derive(Clone)]
pub enum Program {
    Downloaded(Source),
    /// (program, argumen; `{PORT}` diganti port lokal)
    Custom(PathBuf, Vec<String>),
}

pub struct Cloudflared {
    dir: PathBuf,
    program: Program,
    child: tokio::sync::Mutex<Option<Child>>,
    last_error: Mutex<Option<String>>,
    url_timeout: Duration,
}

impl Cloudflared {
    /// Bawaan: unduh `cloudflared` resmi untuk platform ini ke `bin_dir`. `None` bila platform tidak didukung.
    pub fn official(bin_dir: PathBuf) -> Option<Self> {
        let pin = pin_for(std::env::consts::OS, std::env::consts::ARCH)?;
        let ext = if cfg!(windows) { ".exe" } else { "" };
        let source = Source { url: format!("{RELEASE_BASE}/{VERSION}/{}", pin.asset), sha256: pin.sha256.into(), max_size: pin.size + 1_048_576, file_name: format!("cloudflared-{VERSION}{ext}") };
        Some(Self::new(bin_dir, Program::Downloaded(source)))
    }

    pub fn new(dir: PathBuf, program: Program) -> Self {
        Self { dir, program, child: tokio::sync::Mutex::new(None), last_error: Mutex::new(None), url_timeout: URL_TIMEOUT }
    }

    pub fn with_url_timeout(mut self, d: Duration) -> Self {
        self.url_timeout = d;
        self
    }

    /// Pastikan biner ada dan utuh (checksum cocok); unduh bila belum atau rusak.
    pub async fn ensure(&self) -> Result<(PathBuf, Vec<String>), String> {
        match &self.program {
            Program::Custom(p, args) => Ok((p.clone(), args.clone())),
            Program::Downloaded(src) => {
                let (dir, src) = (self.dir.clone(), src.clone());
                let path = tokio::task::spawn_blocking(move || -> Result<PathBuf, String> {
                    std::fs::create_dir_all(&dir).map_err(|e| format!("membuat {}: {e}", dir.display()))?;
                    let dest = dir.join(&src.file_name);
                    if dest.exists() {
                        if file_sha256(&dest).is_ok_and(|h| h.eq_ignore_ascii_case(&src.sha256)) {
                            return Ok(dest);
                        }
                        // Rusak atau diubah: jangan dijalankan.
                        let _ = std::fs::remove_file(&dest);
                    }
                    download_verified(&src.url, &dest, &src.sha256, src.max_size)?;
                    Ok(dest)
                })
                .await
                .map_err(|e| e.to_string())??;
                Ok((path, vec!["tunnel".into(), "--no-autoupdate".into(), "--url".into(), "http://127.0.0.1:{PORT}".into()]))
            }
        }
    }

    async fn start_inner(&self, port: u16) -> Result<String, String> {
        self.stop_inner().await;
        let (bin, args) = self.ensure().await?;
        let args: Vec<String> = args.iter().map(|a| a.replace("{PORT}", &port.to_string())).collect();
        let mut cmd = Command::new(&bin);
        cmd.args(&args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::piped()).kill_on_drop(true);
        #[cfg(windows)]
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW: tanpa jendela konsol
        let mut child = cmd.spawn().map_err(|e| format!("menjalankan {}: {e}", bin.display()))?;
        let stderr = child.stderr.take().ok_or("tanpa keluaran dari cloudflared")?;
        let mut lines = BufReader::new(stderr).lines();
        let mut tail: Vec<String> = Vec::new();
        let found = tokio::time::timeout(self.url_timeout, async {
            while let Ok(Some(line)) = lines.next_line().await {
                if let Some(url) = parse_tunnel_url(&line) {
                    return Some(url);
                }
                tail.push(line);
                if tail.len() > 6 {
                    tail.remove(0);
                }
            }
            None
        })
        .await;
        match found {
            Ok(Some(url)) => {
                // Terus kuras keluaran supaya pipa tidak penuh dan proses tidak macet.
                tokio::spawn(async move { while let Ok(Some(_)) = lines.next_line().await {} });
                *self.child.lock().await = Some(child);
                *self.last_error.lock().expect("lock") = None;
                Ok(url)
            }
            Ok(None) => {
                let _ = child.kill().await;
                Err(format!("cloudflared berhenti sebelum membuka tunnel: {}", tail.join(" | ")))
            }
            Err(_) => {
                let _ = child.kill().await;
                Err(format!("tunnel tidak terbuka dalam {} detik: {}", self.url_timeout.as_secs().max(1), tail.join(" | ")))
            }
        }
    }

    async fn stop_inner(&self) {
        if let Some(mut c) = self.child.lock().await.take() {
            let _ = c.kill().await;
        }
    }
}

impl Tunnel for Cloudflared {
    fn start(&self, local_port: u16) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String, String>> + Send + '_>> {
        Box::pin(async move {
            let r = self.start_inner(local_port).await;
            if let Err(e) = &r {
                *self.last_error.lock().expect("lock") = Some(e.clone());
            }
            r
        })
    }

    fn stop(&self) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + '_>> {
        Box::pin(self.stop_inner())
    }

    fn alive(&self) -> std::pin::Pin<Box<dyn std::future::Future<Output = bool> + Send + '_>> {
        Box::pin(async move {
            match self.child.lock().await.as_mut() {
                Some(c) => matches!(c.try_wait(), Ok(None)),
                None => false,
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_url() {
        let l = "2026-11-19T02:00:00Z INF |  https://tebak-kata-acak.trycloudflare.com                                |";
        assert_eq!(parse_tunnel_url(l).as_deref(), Some("https://tebak-kata-acak.trycloudflare.com"));
        assert_eq!(parse_tunnel_url("https://a1-b2.trycloudflare.com/path").as_deref(), Some("https://a1-b2.trycloudflare.com"));
        // Pesan galat memuat api.trycloudflare.com: bukan tunnel.
        assert_eq!(parse_tunnel_url("ERR failed to request quick Tunnel: Post \"https://api.trycloudflare.com/tunnel\": dial tcp"), None);
        assert_eq!(parse_tunnel_url("https://example.com"), None);
        assert_eq!(parse_tunnel_url("https://evil.com/x.trycloudflare.com"), None);
        assert_eq!(parse_tunnel_url("https://sub.dom.trycloudflare.com"), None);
        assert_eq!(parse_tunnel_url("https://.trycloudflare.com"), None);
        assert_eq!(parse_tunnel_url("tanpa url"), None);
    }

    #[test]
    fn pin_platform() {
        for (os, arch) in [("linux", "x86_64"), ("linux", "aarch64"), ("windows", "x86_64")] {
            let p = pin_for(os, arch).expect("didukung");
            assert_eq!(p.sha256.len(), 64);
            assert!(p.size > 10_000_000);
        }
        assert!(pin_for("macos", "aarch64").is_none() && pin_for("windows", "aarch64").is_none());
    }
}
