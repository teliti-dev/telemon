//! Klien SSH untuk terminal router (ADR 0029). Hanya sesi PTY interaktif; tanpa penerusan port, SFTP, atau agen.
//!
//! - **Kunci host disematkan** (seperti sidik jari TLS router, ADR 0018): `host_key_fingerprint` membaca kunci tanpa
//!   masuk, admin memeriksa sidik jarinya, lalu `open` menolak kunci yang berbeda (`HostKeyMismatch`).
//! - Autentikasi kata sandi saja: akun router yang sama dengan API (RouterOS memakai satu daftar pengguna; akun perlu
//!   kebijakan `ssh`). Kata sandi tidak pernah masuk log atau pesan galat.
//! - `Session` memisahkan baca dan tulis supaya jembatan WebSocket bisa mengalirkan dua arah sekaligus.

use std::sync::Arc;
use std::time::Duration;

use russh::client::{self, Handle};
use russh::keys::{HashAlg, PublicKeyOrCertificate};
use russh::{ChannelMsg, ChannelReadHalf, ChannelWriteHalf, Disconnect};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SshError {
    /// Gagal tersambung atau handshake (pesan tanpa rahasia).
    Connect(String),
    /// Akun atau kata sandi ditolak router, atau akun tanpa kebijakan `ssh`.
    Auth,
    /// Kunci host belum pernah disematkan (hanya dari `open` tanpa pin yang cocok): sidik jari yang ditemui.
    HostKeyMismatch { expected: String, got: String },
    Channel(String),
}

impl std::fmt::Display for SshError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Connect(m) => write!(f, "Tidak bisa tersambung lewat SSH: {m}"),
            Self::Auth => write!(f, "SSH menolak akun router (periksa kata sandi dan kebijakan ssh pada akun)"),
            Self::HostKeyMismatch { expected, got } => write!(f, "Kunci host router berubah (disematkan {expected}, router mengirim {got}); sambungan dibatalkan"),
            Self::Channel(m) => write!(f, "Sesi SSH gagal: {m}"),
        }
    }
}

impl std::error::Error for SshError {}

#[derive(Clone)]
pub struct Target {
    pub host: String,
    pub port: u16,
    pub username: String,
    /// Tidak ikut `Debug`.
    pub password: String,
}

impl std::fmt::Debug for Target {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Target").field("host", &self.host).field("port", &self.port).field("username", &self.username).field("password", &"***").finish()
    }
}

/// Mencatat sidik jari kunci host yang ditawarkan server dan menolak bila tidak sama dengan pin (bila ada pin).
struct Pin {
    expected: Option<String>,
    seen: Arc<std::sync::Mutex<Option<String>>>,
}

impl client::Handler for Pin {
    type Error = russh::Error;

    async fn check_server_key(&mut self, key: &PublicKeyOrCertificate) -> Result<bool, Self::Error> {
        let fp = match key {
            PublicKeyOrCertificate::PublicKey { key, .. } => key.fingerprint(HashAlg::Sha256).to_string(),
            _ => return Ok(false), // sertifikat host tidak didukung
        };
        *self.seen.lock().expect("lock") = Some(fp.clone());
        Ok(self.expected.as_ref().is_none_or(|e| e.eq_ignore_ascii_case(&fp)))
    }
}

fn config() -> Arc<client::Config> {
    Arc::new(client::Config { inactivity_timeout: Some(Duration::from_secs(600)), keepalive_interval: Some(Duration::from_secs(30)), keepalive_max: 3, ..Default::default() })
}

async fn handshake(host: &str, port: u16, expected: Option<String>) -> (Result<Handle<Pin>, SshError>, Option<String>) {
    let seen = Arc::new(std::sync::Mutex::new(None));
    let h = Pin { expected: expected.clone(), seen: seen.clone() };
    let r = tokio::time::timeout(CONNECT_TIMEOUT, client::connect(config(), (host, port), h)).await;
    let got = seen.lock().expect("lock").clone();
    let r = match r {
        Err(_) => Err(SshError::Connect("waktu habis".into())),
        Ok(Ok(handle)) => Ok(handle),
        Ok(Err(e)) => match (&expected, &got) {
            (Some(exp), Some(g)) if !exp.eq_ignore_ascii_case(g) => Err(SshError::HostKeyMismatch { expected: exp.clone(), got: g.clone() }),
            _ => Err(SshError::Connect(e.to_string())),
        },
    };
    (r, got)
}

/// Sidik jari SHA-256 (`SHA256:...`) kunci host, tanpa autentikasi. Dipakai admin untuk memeriksa sebelum menyematkan.
pub async fn host_key_fingerprint(host: &str, port: u16) -> Result<String, SshError> {
    let (r, got) = handshake(host, port, None).await;
    if let Ok(h) = r {
        let _ = h.disconnect(Disconnect::ByApplication, "", "en").await;
    }
    got.ok_or_else(|| SshError::Connect("router tidak mengirim kunci host".into()))
}

/// Sesi PTY terbuka: tulis ke router lewat `writer`, baca keluarannya lewat `reader`.
pub struct Session {
    pub reader: Reader,
    pub writer: Writer,
}

pub struct Reader {
    half: ChannelReadHalf,
    _handle: Arc<Handle<Pin>>,
}

pub struct Writer {
    half: ChannelWriteHalf<client::Msg>,
    handle: Arc<Handle<Pin>>,
}

impl Reader {
    /// Keluaran berikutnya (stdout dan stderr digabung), atau `None` bila sesi berakhir.
    pub async fn read(&mut self) -> Option<Vec<u8>> {
        loop {
            match self.half.wait().await? {
                ChannelMsg::Data { data } => return Some(data.to_vec()),
                ChannelMsg::ExtendedData { data, .. } => return Some(data.to_vec()),
                ChannelMsg::Eof | ChannelMsg::Close | ChannelMsg::ExitStatus { .. } | ChannelMsg::ExitSignal { .. } => return None,
                _ => {}
            }
        }
    }
}

impl Writer {
    pub async fn write(&self, data: &[u8]) -> Result<(), SshError> {
        self.half.data_bytes(data.to_vec()).await.map_err(|e| SshError::Channel(e.to_string()))
    }

    pub async fn resize(&self, cols: u32, rows: u32) -> Result<(), SshError> {
        self.half.window_change(cols, rows, 0, 0).await.map_err(|e| SshError::Channel(e.to_string()))
    }

    pub async fn close(&self) {
        let _ = self.half.eof().await;
        let _ = self.half.close().await;
        let _ = self.handle.disconnect(Disconnect::ByApplication, "", "en").await;
    }
}

/// Buka sesi shell interaktif. `pin` = sidik jari kunci host yang sudah diperiksa admin (wajib).
pub async fn open(target: &Target, pin: &str, cols: u32, rows: u32) -> Result<Session, SshError> {
    let (r, _) = handshake(&target.host, target.port, Some(pin.to_string())).await;
    let mut handle = r?;
    let auth = handle.authenticate_password(target.username.clone(), target.password.clone()).await.map_err(|e| SshError::Connect(e.to_string()))?;
    if !auth.success() {
        return Err(SshError::Auth);
    }
    let channel = handle.channel_open_session().await.map_err(|e| SshError::Channel(e.to_string()))?;
    channel.request_pty(true, "xterm-256color", cols, rows, 0, 0, &[]).await.map_err(|e| SshError::Channel(e.to_string()))?;
    channel.request_shell(true).await.map_err(|e| SshError::Channel(e.to_string()))?;
    let (read, write) = channel.split();
    let handle = Arc::new(handle);
    Ok(Session { reader: Reader { half: read, _handle: handle.clone() }, writer: Writer { half: write, handle } })
}
