//! Server SSH tiruan untuk mencoba terminal tanpa router asli: `cargo run -p telemon-ssh --example fake_router -- 2222`.
//! Akun `api` / `rahasia`. Menjawab prompt bergaya RouterOS dan menggemakan masukan. Hanya untuk pengembangan.
use std::sync::Arc;

use russh::keys::{Algorithm, HashAlg, PrivateKey};
use russh::server::{self, Auth, Msg, Server as _, Session};
use russh::{Channel, ChannelId, Pty};

#[derive(Clone)]
struct Fake;

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
    async fn channel_open_session(&mut self, _c: Channel<Msg>, reply: server::ChannelOpenHandle, _s: &mut Session) -> Result<(), Self::Error> {
        reply.accept().await;
        Ok(())
    }
    async fn pty_request(&mut self, channel: ChannelId, _t: &str, _c: u32, _r: u32, _pw: u32, _ph: u32, _m: &[(Pty, u32)], session: &mut Session) -> Result<(), Self::Error> {
        session.channel_success(channel)?;
        Ok(())
    }
    async fn shell_request(&mut self, channel: ChannelId, session: &mut Session) -> Result<(), Self::Error> {
        session.channel_success(channel)?;
        session.data(channel, b"\r\n  MikroTik RouterOS (fake) - demo shell\r\n\r\n[api@fake] > ".to_vec())?;
        Ok(())
    }
    async fn data(&mut self, channel: ChannelId, data: &[u8], session: &mut Session) -> Result<(), Self::Error> {
        let mut out = Vec::new();
        for &b in data {
            match b {
                b'\r' => out.extend_from_slice(b"\r\n[api@fake] > "),
                0x7f => out.extend_from_slice(b"\x08 \x08"),
                _ => out.push(b),
            }
        }
        session.data(channel, out)?;
        Ok(())
    }
}

#[tokio::main]
async fn main() {
    let port: u16 = std::env::args().nth(1).and_then(|p| p.parse().ok()).unwrap_or(2222);
    let key = PrivateKey::random(&mut rand::rng(), Algorithm::Ed25519).unwrap();
    println!("host key: {}", key.public_key().fingerprint(HashAlg::Sha256));
    let config = Arc::new(server::Config { keys: vec![key], auth_rejection_time: std::time::Duration::from_millis(50), ..Default::default() });
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port)).await.unwrap();
    println!("listening on 127.0.0.1:{port} (api / rahasia)");
    let mut srv = Fake;
    srv.run_on_socket(config, &listener).await.unwrap();
}
