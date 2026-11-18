//! Antarmuka baca RouterOS dan implementasi `mikrotik-rs`.

use std::fmt;

use mikrotik_rs::{CommandBuilder, Event, MikrotikDevice};

use crate::model::{PppActive, PppProfile, PppSecret};

pub use crate::model::Row;

#[derive(Debug)]
pub enum Error {
    /// Gagal terhubung atau login.
    Connect(String),
    /// Router menjawab `!trap` (kesalahan perintah, hak akses kurang, dst).
    Trap(String),
    /// Koneksi putus atau router mengirim `!fatal`.
    Fatal(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Connect(m) => write!(f, "gagal terhubung: {m}"),
            Error::Trap(m) => write!(f, "router menolak: {m}"),
            Error::Fatal(m) => write!(f, "koneksi terputus: {m}"),
        }
    }
}

impl std::error::Error for Error {}

/// Baca-saja untuk saat ini. Penulisan (ADR 0004) menyusul dengan tinjauan perubahan.
pub trait RouterApi: Sync {
    /// Menjalankan `<path>/print` dan mengumpulkan semua baris.
    fn print(&self, path: &str) -> impl std::future::Future<Output = Result<Vec<Row>, Error>> + Send;
}

pub async fn ppp_active(api: &impl RouterApi) -> Result<Vec<PppActive>, Error> {
    Ok(api.print("/ppp/active").await?.iter().map(PppActive::from_row).collect())
}

pub async fn ppp_secrets(api: &impl RouterApi) -> Result<Vec<PppSecret>, Error> {
    Ok(api.print("/ppp/secret").await?.iter().map(PppSecret::from_row).collect())
}

pub async fn ppp_profiles(api: &impl RouterApi) -> Result<Vec<PppProfile>, Error> {
    Ok(api.print("/ppp/profile").await?.iter().map(PppProfile::from_row).collect())
}

/// Klien `mikrotik-rs` (API plaintext port 8728; TLS 8729 menyusul lewat fitur `tls`).
#[derive(Clone)]
pub struct MikrotikClient {
    device: MikrotikDevice,
}

impl MikrotikClient {
    pub async fn connect(addr: &str, username: &str, password: &str) -> Result<Self, Error> {
        let device = MikrotikDevice::connect(addr, username, Some(password))
            .await
            .map_err(|e| Error::Connect(e.to_string()))?;
        Ok(Self { device })
    }
}

impl RouterApi for MikrotikClient {
    async fn print(&self, path: &str) -> Result<Vec<Row>, Error> {
        let cmd = CommandBuilder::new().command(&format!("{path}/print")).build();
        let mut rx = self.device.send_command(cmd).await.map_err(|e| Error::Fatal(e.to_string()))?;
        let mut rows = Vec::new();
        while let Some(ev) = rx.recv().await {
            match ev {
                Event::Reply { response, .. } => rows.push(
                    response.attributes.into_iter().map(|(k, v)| (k, v.unwrap_or_default())).collect(),
                ),
                Event::Done { .. } | Event::Empty { .. } => return Ok(rows),
                Event::Trap { response, .. } => return Err(Error::Trap(response.message)),
                Event::Fatal { reason } => return Err(Error::Fatal(reason)),
            }
        }
        Err(Error::Fatal("koneksi ditutup sebelum perintah selesai".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::{TcpListener, TcpStream};

    // ---- Router tiruan: protokol kalimat RouterOS (panjang-kata + kata, diakhiri kata kosong) ----

    fn enc(w: &str) -> Vec<u8> {
        let n = w.len();
        let mut out = if n < 0x80 {
            vec![n as u8]
        } else if n < 0x4000 {
            vec![((n >> 8) as u8) | 0x80, n as u8]
        } else {
            panic!("kata terlalu panjang untuk tiruan")
        };
        out.extend_from_slice(w.as_bytes());
        out
    }

    fn sentence(words: &[&str]) -> Vec<u8> {
        let mut out: Vec<u8> = words.iter().flat_map(|w| enc(w)).collect();
        out.push(0);
        out
    }

    async fn read_len(s: &mut TcpStream) -> Option<usize> {
        let mut b = [0u8; 1];
        s.read_exact(&mut b).await.ok()?;
        let b0 = b[0] as usize;
        if b0 < 0x80 {
            return Some(b0);
        }
        let mut b1 = [0u8; 1];
        s.read_exact(&mut b1).await.ok()?;
        Some(((b0 & 0x3f) << 8) | b1[0] as usize)
    }

    async fn read_sentence(s: &mut TcpStream) -> Option<Vec<String>> {
        let mut words = Vec::new();
        loop {
            let n = read_len(s).await?;
            if n == 0 {
                return Some(words);
            }
            let mut buf = vec![0u8; n];
            s.read_exact(&mut buf).await.ok()?;
            words.push(String::from_utf8(buf).ok()?);
        }
    }

    /// Router tiruan: terima login apa saja, jawab print PPP dengan data tetap.
    async fn fake_router(secret_pw: &'static str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        tokio::spawn(async move {
            loop {
                let (mut s, _) = listener.accept().await.unwrap();
                tokio::spawn(async move {
                    while let Some(words) = read_sentence(&mut s).await {
                        let tag = words.iter().find(|w| w.starts_with(".tag=")).cloned().unwrap_or_default();
                        let cmd = words.first().cloned().unwrap_or_default();
                        let mut out = Vec::new();
                        match cmd.as_str() {
                            "/login" => out.extend(sentence(&["!done", &tag])),
                            "/ppp/active/print" => {
                                out.extend(sentence(&["!re", "=.id=*1", "=name=budi", "=service=pppoe", "=address=100.64.1.2", "=caller-id=AA:BB:CC:DD:EE:FF", "=uptime=1h2m", &tag]));
                                out.extend(sentence(&["!done", &tag]));
                            }
                            "/ppp/secret/print" => {
                                let pw = format!("=password={secret_pw}");
                                out.extend(sentence(&["!re", "=.id=*2", "=name=budi", "=profile=10M", "=disabled=false", &pw, &tag]));
                                out.extend(sentence(&["!re", "=.id=*3", "=name=sari", "=profile=20M", "=disabled=true", &tag]));
                                out.extend(sentence(&["!done", &tag]));
                            }
                            "/ppp/profile/print" => {
                                out.extend(sentence(&["!re", "=.id=*4", "=name=10M", "=rate-limit=10M/10M", &tag]));
                                out.extend(sentence(&["!done", &tag]));
                            }
                            _ => out.extend(sentence(&["!trap", "=message=no such command", &tag])),
                        }
                        if s.write_all(&out).await.is_err() {
                            return;
                        }
                    }
                });
            }
        });
        addr
    }

    #[tokio::test]
    async fn membaca_ppp_dari_router_tiruan() {
        let addr = fake_router("rahasia123").await;
        let c = MikrotikClient::connect(&addr, "api", "x").await.expect("terhubung");

        let active = ppp_active(&c).await.unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].name, "budi");
        assert_eq!(active[0].uptime_secs, Some(3_720));

        let secrets = ppp_secrets(&c).await.unwrap();
        assert_eq!(secrets.len(), 2);
        assert!(!secrets[0].disabled && secrets[1].disabled);
        assert_eq!(secrets[0].password.as_deref(), Some("rahasia123"));
        assert_eq!(secrets[1].password, None);

        let profiles = ppp_profiles(&c).await.unwrap();
        assert_eq!(profiles[0].rate_limit.as_deref(), Some("10M/10M"));
    }

    #[tokio::test]
    async fn trap_menjadi_error() {
        let addr = fake_router("x").await;
        let c = MikrotikClient::connect(&addr, "api", "x").await.unwrap();
        match c.print("/tidak/ada").await {
            Err(Error::Trap(m)) => assert!(m.contains("no such command")),
            other => panic!("harusnya Trap, dapat {other:?}"),
        }
    }

    #[tokio::test]
    async fn koneksi_ditolak_menjadi_error() {
        // Port 1 di localhost hampir pasti tertutup.
        match MikrotikClient::connect("127.0.0.1:1", "a", "b").await {
            Err(Error::Connect(_)) => {}
            Err(e) => panic!("jenis error salah: {e:?}"),
            Ok(_) => panic!("seharusnya gagal"),
        }
    }
}
