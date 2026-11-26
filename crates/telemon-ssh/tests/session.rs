use std::sync::{Arc, Mutex};
use std::time::Duration;

use russh::keys::{Algorithm, HashAlg, PrivateKey};
use russh::server::{self, Auth, Msg, Server as _, Session};
use russh::{Channel, ChannelId, Pty};
use telemon_ssh::{SshError, Target, host_key_fingerprint, open};
use tokio::net::TcpListener;

/// Server SSH tiruan: akun `api`/`rahasia`; menjawab prompt, menggemakan masukan, dan mencatat ukuran terminal.
#[derive(Clone)]
struct Fake {
    log: Arc<Mutex<Vec<String>>>,
}

impl server::Server for Fake {
    type Handler = Self;
    fn new_client(&mut self, _: Option<std::net::SocketAddr>) -> Self {
        self.clone()
    }
}

impl server::Handler for Fake {
    type Error = russh::Error;

    async fn auth_password(&mut self, user: &str, password: &str) -> Result<Auth, Self::Error> {
        Ok(if user == "api" && password == "rahasia" { Auth::Accept } else { Auth::reject() })
    }

    async fn channel_open_session(&mut self, _channel: Channel<Msg>, reply: server::ChannelOpenHandle, _s: &mut Session) -> Result<(), Self::Error> {
        reply.accept().await;
        Ok(())
    }

    async fn pty_request(&mut self, channel: ChannelId, term: &str, cols: u32, rows: u32, _pw: u32, _ph: u32, _modes: &[(Pty, u32)], session: &mut Session) -> Result<(), Self::Error> {
        self.log.lock().unwrap().push(format!("pty {term} {cols}x{rows}"));
        session.channel_success(channel)?;
        Ok(())
    }

    async fn shell_request(&mut self, channel: ChannelId, session: &mut Session) -> Result<(), Self::Error> {
        session.channel_success(channel)?;
        session.data(channel, b"[api@router] > ".to_vec())?;
        Ok(())
    }

    async fn window_change_request(&mut self, _c: ChannelId, cols: u32, rows: u32, _pw: u32, _ph: u32, _s: &mut Session) -> Result<(), Self::Error> {
        self.log.lock().unwrap().push(format!("resize {cols}x{rows}"));
        Ok(())
    }

    async fn data(&mut self, channel: ChannelId, data: &[u8], session: &mut Session) -> Result<(), Self::Error> {
        if data == b"quit\r" {
            session.eof(channel)?;
            session.close(channel)?;
            return Ok(());
        }
        let mut out = b"echo:".to_vec();
        out.extend_from_slice(data);
        session.data(channel, out)?;
        Ok(())
    }
}

/// Mulai server; kembalikan port, sidik jari kunci host, dan log.
async fn start() -> (u16, String, Arc<Mutex<Vec<String>>>) {
    let key = PrivateKey::random(&mut rand::rng(), Algorithm::Ed25519).unwrap();
    let fp = key.public_key().fingerprint(HashAlg::Sha256).to_string();
    let config = Arc::new(server::Config { keys: vec![key], auth_rejection_time: Duration::from_millis(1), auth_rejection_time_initial: Some(Duration::from_millis(0)), ..Default::default() });
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let log = Arc::new(Mutex::new(Vec::new()));
    let mut srv = Fake { log: log.clone() };
    tokio::spawn(async move {
        let _ = srv.run_on_socket(config, &listener).await;
    });
    (port, fp, log)
}

fn target(port: u16, pw: &str) -> Target {
    Target { host: "127.0.0.1".into(), port, username: "api".into(), password: pw.into() }
}

async fn read_until(r: &mut telemon_ssh::Reader, needle: &str) -> String {
    let mut s = String::new();
    while !s.contains(needle) {
        let chunk = tokio::time::timeout(Duration::from_secs(5), r.read()).await.expect("waktu habis menunggu keluaran").expect("sesi berakhir");
        s.push_str(&String::from_utf8_lossy(&chunk));
    }
    s
}

#[tokio::test]
async fn fingerprint_can_be_read_without_logging_in_and_matches_the_key() {
    let (port, fp, _) = start().await;
    assert_eq!(host_key_fingerprint("127.0.0.1", port).await.unwrap(), fp);
    assert!(fp.starts_with("SHA256:"));
    assert!(matches!(host_key_fingerprint("127.0.0.1", 1).await, Err(SshError::Connect(_))));
}

#[tokio::test]
async fn interactive_session_streams_both_ways_and_resizes() {
    let (port, fp, log) = start().await;
    let mut s = open(&target(port, "rahasia"), &fp, 100, 30).await.unwrap();
    assert!(read_until(&mut s.reader, "[api@router] > ").await.ends_with("> "));
    s.writer.write(b"/ip address print").await.unwrap();
    assert!(read_until(&mut s.reader, "echo:/ip address print").await.contains("echo:"));
    s.writer.resize(120, 40).await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    {
        let l = log.lock().unwrap();
        assert!(l.contains(&"pty xterm-256color 100x30".to_string()), "{l:?}");
        assert!(l.contains(&"resize 120x40".to_string()), "{l:?}");
    }
    // Server menutup sesi: pembaca mendapat None.
    s.writer.write(b"quit\r").await.unwrap();
    let end = tokio::time::timeout(Duration::from_secs(5), async { while s.reader.read().await.is_some() {} }).await;
    assert!(end.is_ok(), "sesi harus berakhir saat server menutup");
}

#[tokio::test]
async fn host_key_must_match_the_pin_and_wrong_password_is_refused() {
    let (port, fp, _) = start().await;
    // Pin salah: ditolak sebelum mengirim kata sandi apa pun.
    let wrong = "SHA256:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
    match open(&target(port, "rahasia"), wrong, 80, 24).await {
        Err(SshError::HostKeyMismatch { expected, got }) => assert_eq!((expected.as_str(), got.as_str()), (wrong, fp.as_str())),
        other => panic!("diharapkan HostKeyMismatch, dapat {:?}", other.err()),
    }
    // Pin benar tetapi kata sandi salah: Auth.
    assert!(matches!(open(&target(port, "salah"), &fp, 80, 24).await.err(), Some(SshError::Auth)));
    // Huruf besar kecil pada pin tidak menjadi masalah untuk pembandingan heksa/base64? (base64 peka huruf: tetap cocok persis)
    assert!(open(&target(port, "rahasia"), &fp, 80, 24).await.is_ok());
    // Kata sandi tidak muncul di Debug.
    let dbg = format!("{:?}", target(port, "rahasia"));
    assert!(!dbg.contains("rahasia") && dbg.contains("***"));
}
