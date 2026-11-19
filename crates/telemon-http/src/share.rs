//! Berbagi (ADR 0002/0003): server LAN yang bisa dinyalakan dari aplikasi desktop, dan status berbagi
//! untuk UI. Akses Publik (Cloudflare Quick Tunnel) belum ada.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};
use telemon_core::Connector;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

use crate::error::ApiError;
use crate::state::AppState;

pub const DEFAULT_LAN_PORT: u16 = 7878;

pub struct Running {
    task: JoinHandle<()>,
    stop: oneshot::Sender<()>,
    pub addr: SocketAddr,
}

pub struct Lan {
    pub bind: String,
    pub port: u16,
    pub running: Option<Running>,
}

impl Default for Lan {
    fn default() -> Self {
        Self { bind: "lan".into(), port: DEFAULT_LAN_PORT, running: None }
    }
}

/// Alamat IP komputer di jaringan (untuk URL dan QR). Tidak mengirim paket apa pun.
fn local_ip() -> IpAddr {
    std::net::UdpSocket::bind("0.0.0.0:0")
        .and_then(|s| s.connect("192.0.2.1:9").map(|()| s))
        .and_then(|s| s.local_addr())
        .map_or(IpAddr::V4(Ipv4Addr::LOCALHOST), |a| a.ip())
}

impl<C: Connector> AppState<C> {
    /// Nyalakan atau matikan server LAN (permukaan Remote: login akun wajib, ADR 0005).
    pub async fn set_lan(self: &Arc<Self>, enabled: bool, bind: &str, port: u16) -> Result<(), ApiError> {
        if !matches!(bind, "lan" | "localhost") {
            return Err(ApiError::bad("Bind harus lan atau localhost"));
        }
        if enabled && !(1024..=65535).contains(&port) {
            return Err(ApiError::bad("Port harus 1024-65535"));
        }
        let mut lan = self.lan.lock().await;
        if let Some(r) = lan.running.take() {
            let _ = r.stop.send(());
            // Koneksi yang masih terbuka (WebSocket) tidak menunda pematian: dipaksa setelah jeda singkat.
            let mut task = r.task;
            if tokio::time::timeout(Duration::from_secs(2), &mut task).await.is_err() {
                task.abort();
            }
            self.lan_epoch.send_modify(|e| *e += 1);
        }
        lan.bind = bind.into();
        lan.port = port;
        if !enabled {
            return Ok(());
        }
        let ip = if bind == "lan" { IpAddr::V4(Ipv4Addr::UNSPECIFIED) } else { IpAddr::V4(Ipv4Addr::LOCALHOST) };
        let listener = tokio::net::TcpListener::bind(SocketAddr::new(ip, port)).await.map_err(|e| ApiError::new(axum::http::StatusCode::CONFLICT, "port_in_use", format!("Port {port} tidak bisa dipakai: {e}")))?;
        let addr = listener.local_addr().map_err(ApiError::internal)?;
        let (stop, rx) = oneshot::channel();
        let st = self.clone();
        let task = tokio::spawn(async move {
            let _ = crate::serve_on(st, listener, async {
                let _ = rx.await;
            })
            .await;
        });
        lan.running = Some(Running { task, stop, addr });
        Ok(())
    }

    pub async fn lan_running(&self) -> bool {
        self.lan.lock().await.running.is_some()
    }

    pub async fn shutdown_lan(self: &Arc<Self>) {
        let (bind, port) = {
            let l = self.lan.lock().await;
            (l.bind.clone(), l.port)
        };
        let _ = self.set_lan(false, &bind, port).await;
    }

    /// Status berbagi untuk UI. `owner` (aplikasi desktop) melihat alamat server; peramban jarak jauh tidak.
    pub async fn share_json(&self, owner: bool) -> Value {
        let (require_2fa, public_on) = self.share_flags();
        let settings = self.telemon.with_store(|s| s.get_settings()).unwrap_or_default();
        let lan = self.lan.lock().await;
        let enabled = lan.running.is_some();
        let url = lan.running.as_ref().map(|r| {
            let host = if lan.bind == "lan" { local_ip() } else { IpAddr::V4(Ipv4Addr::LOCALHOST) };
            format!("http://{}", SocketAddr::new(host, r.addr.port()))
        });
        json!({
            "lan": { "enabled": enabled, "bind": lan.bind, "port": lan.port, "url": if owner { url } else { None } },
            "publik": { "status": if public_on { "on" } else { "off" }, "durationHours": 8 },
            "auth": { "require2fa": require_2fa, "allowPublicWrite": settings.remote.allow_public_write },
            "features": { "ai": false, "aiMask": settings.ai.mask_sensitive },
        })
    }
}
