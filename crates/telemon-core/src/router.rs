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
    /// Ditolak oleh Telemon sebelum sampai ke router (validasi atau di luar daftar perintah yang diizinkan).
    Rejected(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Connect(m) => write!(f, "gagal terhubung: {m}"),
            Error::Trap(m) => write!(f, "router menolak: {m}"),
            Error::Fatal(m) => write!(f, "koneksi terputus: {m}"),
            Error::Rejected(m) => write!(f, "ditolak: {m}"),
        }
    }
}

impl std::error::Error for Error {}

/// Akses RouterOS. Membaca lewat `print`/`find`; menulis lewat `run` yang hanya boleh dipanggil
/// dari modul `write` (daftar perintah diizinkan, ADR 0004).
pub trait RouterApi: Sync {
    /// Menjalankan `<path>/print` dan mengumpulkan semua baris.
    fn print(&self, path: &str) -> impl std::future::Future<Output = Result<Vec<Row>, Error>> + Send;

    /// Baris dengan `key == value` (bawaan: baca semua lalu saring; klien nyata memakai query router).
    fn find(&self, path: &str, key: &str, value: &str) -> impl std::future::Future<Output = Result<Vec<Row>, Error>> + Send {
        async move { Ok(self.print(path).await?.into_iter().filter(|r| r.get(key).is_some_and(|v| v == value)).collect()) }
    }

    /// Menjalankan perintah apa adanya dengan atribut `=k=v`. Hanya untuk modul `write`.
    fn run(&self, _cmd: &str, _attrs: &[(String, String)]) -> impl std::future::Future<Output = Result<Vec<Row>, Error>> + Send {
        async { Err(Error::Rejected("klien ini tidak mendukung penulisan".into())) }
    }
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

/// Cara menyambung ke router.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TlsMode {
    /// API biasa (port 8728), tanpa enkripsi. Hanya untuk jaringan tepercaya.
    Plain,
    /// api-ssl (port 8729) dengan sertifikat yang di-pin lewat sidik jarinya.
    #[cfg(feature = "tls")]
    Pinned(crate::tls::Fingerprint),
}

impl MikrotikClient {
    pub async fn connect_with(addr: &str, username: &str, password: &str, mode: TlsMode) -> Result<Self, Error> {
        match mode {
            TlsMode::Plain => Self::connect(addr, username, password).await,
            #[cfg(feature = "tls")]
            TlsMode::Pinned(pin) => {
                let device = MikrotikDevice::builder(addr)
                    .credentials(username, Some(password))
                    .tls_config(crate::tls::pinned_config(pin)?, crate::tls::server_name())
                    .connect()
                    .await
                    .map_err(|e| Error::Connect(e.to_string()))?;
                Ok(Self { device })
            }
        }
    }

    pub async fn connect(addr: &str, username: &str, password: &str) -> Result<Self, Error> {
        let device = MikrotikDevice::connect(addr, username, Some(password))
            .await
            .map_err(|e| Error::Connect(e.to_string()))?;
        Ok(Self { device })
    }
}

async fn collect(mut rx: tokio::sync::mpsc::UnboundedReceiver<Event>) -> Result<Vec<Row>, Error> {
    let mut rows = Vec::new();
    while let Some(ev) = rx.recv().await {
        match ev {
            Event::Reply { response, .. } => rows.push(response.attributes.into_iter().map(|(k, v)| (k, v.unwrap_or_default())).collect()),
            Event::Done { .. } | Event::Empty { .. } => return Ok(rows),
            Event::Trap { response, .. } => return Err(Error::Trap(response.message)),
            Event::Fatal { reason } => return Err(Error::Fatal(reason)),
        }
    }
    Err(Error::Fatal("koneksi ditutup sebelum perintah selesai".into()))
}

impl RouterApi for MikrotikClient {
    async fn print(&self, path: &str) -> Result<Vec<Row>, Error> {
        let cmd = CommandBuilder::new().command(&format!("{path}/print")).build();
        collect(self.device.send_command(cmd).await.map_err(|e| Error::Fatal(e.to_string()))?).await
    }

    async fn find(&self, path: &str, key: &str, value: &str) -> Result<Vec<Row>, Error> {
        let cmd = CommandBuilder::new().command(&format!("{path}/print")).query_equal(key, value).build();
        collect(self.device.send_command(cmd).await.map_err(|e| Error::Fatal(e.to_string()))?).await
    }

    async fn run(&self, cmd: &str, attrs: &[(String, String)]) -> Result<Vec<Row>, Error> {
        let mut b = CommandBuilder::new().command(cmd);
        for (k, v) in attrs {
            b = b.attribute(k, Some(v));
        }
        collect(self.device.send_command(b.build()).await.map_err(|e| Error::Fatal(e.to_string()))?).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
    use tokio::net::TcpListener;

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

    async fn read_len<S: AsyncRead + Unpin>(s: &mut S) -> Option<usize> {
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

    async fn read_sentence<S: AsyncRead + Unpin>(s: &mut S) -> Option<Vec<String>> {
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
        fake_router_log(secret_pw, Default::default()).await
    }

    type WordLog = std::sync::Arc<std::sync::Mutex<Vec<Vec<String>>>>;

    /// Seperti `fake_router`, dan mencatat setiap kalimat yang diterima.
    #[cfg(feature = "tls")]
    type Acceptor = tokio_rustls::TlsAcceptor;
    #[cfg(not(feature = "tls"))]
    type Acceptor = ();

    async fn fake_router_log(secret_pw: &'static str, log: WordLog) -> String {
        fake_router_with(secret_pw, log, None).await
    }

    /// Satu sambungan: balas kalimat RouterOS dengan data tetap (stream apa pun: TCP atau TLS).
    async fn serve<S: AsyncRead + AsyncWrite + Unpin>(mut s: S, secret_pw: &'static str, log: WordLog) {
        while let Some(words) = read_sentence(&mut s).await {
                        log.lock().unwrap().push(words.clone());
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
                            "/ppp/secret/set" => out.extend(sentence(&["!done", &tag])),
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
    }

    async fn fake_router_with(secret_pw: &'static str, log: WordLog, acceptor: Option<Acceptor>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        tokio::spawn(async move {
            loop {
                let (tcp, _) = listener.accept().await.unwrap();
                let log = log.clone();
                #[cfg(feature = "tls")]
                let acceptor = acceptor.clone();
                tokio::spawn(async move {
                    #[cfg(feature = "tls")]
                    if let Some(acc) = acceptor {
                        if let Ok(tls) = acc.accept(tcp).await {
                            serve(tls, secret_pw, log).await;
                        }
                        return;
                    }
                    serve(tcp, secret_pw, log).await;
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
    async fn run_dan_find_mengirim_atribut_dan_query() {
        let log: WordLog = Default::default();
        let addr = fake_router_log("x", log.clone()).await;
        let c = MikrotikClient::connect(&addr, "api", "x").await.unwrap();
        c.run("/ppp/secret/set", &[(".id".into(), "*1".into()), ("profile".into(), "20M".into())]).await.unwrap();
        c.find("/ppp/secret", "name", "budi").await.unwrap();
        let log = log.lock().unwrap();
        let set = log.iter().find(|w| w[0] == "/ppp/secret/set").expect("set terkirim");
        assert!(set.contains(&"=.id=*1".to_string()) && set.contains(&"=profile=20M".to_string()), "{set:?}");
        let print = log.iter().find(|w| w[0] == "/ppp/secret/print" && w.iter().any(|x| x.starts_with('?'))).expect("print berquery");
        assert!(print.contains(&"?name=budi".to_string()), "{print:?}");
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

    // ---------------- TLS (api-ssl) ----------------

    #[cfg(feature = "tls")]
    mod tls_tests {
        use super::*;
        use crate::tls::{Fingerprint, fetch_fingerprint};
        use rustls::pki_types::{PrivateKeyDer, PrivatePkcs8KeyDer};
        use std::sync::Arc;
        use std::time::Duration;

        /// Router tiruan ber-TLS dengan sertifikat self-signed baru; mengembalikan alamat dan sidik jarinya.
        async fn tls_router() -> (String, Fingerprint) {
            let ck = rcgen::generate_simple_self_signed(vec!["mikrotik".to_string()]).unwrap();
            let der = ck.cert.der().clone();
            let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(ck.signing_key.serialize_der()));
            let provider = Arc::new(rustls::crypto::ring::default_provider());
            let cfg = rustls::ServerConfig::builder_with_provider(provider)
                .with_safe_default_protocol_versions()
                .unwrap()
                .with_no_client_auth()
                .with_single_cert(vec![der.clone()], key)
                .unwrap();
            let addr = fake_router_with("rahasia123", Default::default(), Some(tokio_rustls::TlsAcceptor::from(Arc::new(cfg)))).await;
            (addr, Fingerprint::of_cert(&der))
        }

        const WAIT: Duration = Duration::from_secs(5);

        #[tokio::test]
        async fn pin_cocok_membaca_data_lewat_tls() {
            let (addr, fp) = tls_router().await;
            let c = MikrotikClient::connect_with(&addr, "api", "x", TlsMode::Pinned(fp)).await.expect("terhubung lewat TLS");
            let secrets = ppp_secrets(&c).await.unwrap();
            assert_eq!(secrets[0].password.as_deref(), Some("rahasia123"));
            assert_eq!(ppp_active(&c).await.unwrap()[0].name, "budi");
        }

        #[tokio::test]
        async fn pin_salah_ditolak() {
            let (addr, mut fp) = tls_router().await;
            fp.0[0] ^= 0xff;
            match tokio::time::timeout(WAIT, MikrotikClient::connect_with(&addr, "api", "x", TlsMode::Pinned(fp))).await.expect("tidak menggantung") {
                Err(Error::Connect(m)) => assert!(m.contains("sidik jari sertifikat router tidak cocok"), "{m}"),
                Err(e) => panic!("jenis error salah: {e:?}"),
                Ok(_) => panic!("sertifikat tak dipercaya harus ditolak"),
            }
        }

        #[tokio::test]
        async fn percaya_saat_pertama_lalu_pin() {
            let (addr, real) = tls_router().await;
            let seen = fetch_fingerprint(&addr, WAIT).await.expect("sidik jari terbaca");
            assert_eq!(seen, real);
            assert_eq!(Fingerprint::parse(&seen.to_hex()), Some(seen));
            MikrotikClient::connect_with(&addr, "api", "x", TlsMode::Pinned(seen)).await.expect("pin dari fetch diterima");
            // Router lain (sertifikat berbeda) di alamat yang sama tidak diterima dengan pin lama.
            let (other, _) = tls_router().await;
            assert!(MikrotikClient::connect_with(&other, "api", "x", TlsMode::Pinned(seen)).await.is_err());
        }

        #[tokio::test]
        async fn salah_protokol_gagal_bukan_menggantung() {
            // Klien TLS ke server API biasa, dan klien biasa ke server TLS.
            let plain = fake_router("x").await;
            let (tls_addr, fp) = tls_router().await;
            let a = tokio::time::timeout(WAIT, MikrotikClient::connect_with(&plain, "a", "b", TlsMode::Pinned(fp))).await;
            assert!(matches!(a, Ok(Err(_))), "TLS ke server biasa harus gagal cepat");
            assert!(fetch_fingerprint(&plain, Duration::from_millis(500)).await.is_err());
            let b = tokio::time::timeout(WAIT, MikrotikClient::connect(&tls_addr, "a", "b")).await;
            assert!(matches!(b, Ok(Err(_))) || b.is_err(), "klien biasa ke server TLS tidak boleh berhasil");
        }

        #[tokio::test]
        async fn fetch_ke_port_tertutup_gagal() {
            assert!(matches!(fetch_fingerprint("127.0.0.1:1", Duration::from_secs(1)).await, Err(Error::Connect(_))));
        }
    }
}
