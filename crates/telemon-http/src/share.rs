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
/// Waktu maksimum menyalakan tunnel (unduh `cloudflared` pertama kali termasuk).
const TUNNEL_START_TIMEOUT: Duration = Duration::from_secs(180);
const ALIVE_EVERY: Duration = Duration::from_secs(10);

type BoxFut<'a, T> = std::pin::Pin<Box<dyn std::future::Future<Output = T> + Send + 'a>>;

/// Penyedia tunnel publik (ADR 0003): menyalakan Cloudflare Quick Tunnel ke port lokal dan mengembalikan URL
/// publiknya. Disediakan aplikasi desktop; server HTTP tidak tahu cara kerjanya.
pub trait Tunnel: Send + Sync {
    fn start(&self, local_port: u16) -> BoxFut<'_, Result<String, String>>;
    fn stop(&self) -> BoxFut<'_, ()>;
    /// Tunnel masih berjalan? Diperiksa berkala; bila mati, status Publik menjadi galat (bukan "Aktif" dengan URL mati).
    fn alive(&self) -> BoxFut<'_, bool>;
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum PublikStatus {
    #[default]
    Off,
    Connecting,
    On,
    Error,
}

#[derive(Default)]
pub struct Publik {
    pub status: PublikStatus,
    pub url: Option<String>,
    pub expires_at: Option<i64>,
    pub duration_hours: u32,
    pub error: Option<String>,
    timer: Option<JoinHandle<()>>,
}

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
    /// Pasang penyedia tunnel (aplikasi desktop). Tanpa penyedia, akses Publik menjawab 501.
    pub fn set_tunnel(&self, t: Arc<dyn Tunnel>) {
        *self.tunnel.lock().expect("lock") = Some(t);
    }

    /// Nyalakan atau matikan akses Publik. Syarat: server LAN menyala (tunnel mengarah ke sana), penyedia
    /// tunnel ada. Selama aktif, 2FA wajib untuk semua akun (ADR 0005); mati otomatis setelah `hours` jam.
    pub async fn set_public(self: &Arc<Self>, enabled: bool, hours: u32) -> Result<(), ApiError> {
        if !enabled {
            self.stop_public().await;
            return Ok(());
        }
        if !matches!(hours, 1 | 4 | 8 | 24) {
            return Err(ApiError::bad("Durasi harus 1, 4, 8, atau 24 jam"));
        }
        let tunnel = self.tunnel.lock().expect("lock").clone().ok_or_else(|| ApiError::new(axum::http::StatusCode::NOT_IMPLEMENTED, "not_implemented", "Akses publik belum tersedia di aplikasi ini"))?;
        let port = {
            let lan = self.lan.lock().await;
            lan.running.as_ref().map(|r| r.addr.port())
        }
        .ok_or_else(|| ApiError::new(axum::http::StatusCode::CONFLICT, "lan_off", "Nyalakan Serve ke browser dulu"))?;
        {
            let mut p = self.publik.lock().await;
            if matches!(p.status, PublikStatus::On | PublikStatus::Connecting) {
                return Err(ApiError::new(axum::http::StatusCode::CONFLICT, "already_on", "Akses publik sudah aktif atau sedang dinyalakan"));
            }
            *p = Publik { status: PublikStatus::Connecting, duration_hours: hours, ..Default::default() };
        }
        let started = tokio::time::timeout(TUNNEL_START_TIMEOUT, tunnel.start(port)).await.unwrap_or_else(|_| Err("waktu habis menyalakan tunnel".into()));
        match started {
            Ok(url) => {
                let expires = self.now() + i64::from(hours) * 3_600_000;
                let st = self.clone();
                let watched = tunnel.clone();
                let timer = tokio::spawn(async move {
                    let deadline = tokio::time::Instant::now() + Duration::from_secs(u64::from(hours) * 3600);
                    let mut tick = tokio::time::interval(ALIVE_EVERY);
                    tick.tick().await;
                    loop {
                        tokio::select! {
                            // Dipanggil dari tugas ini sendiri: jangan membatalkan diri sendiri di tengah menghentikan tunnel.
                            () = tokio::time::sleep_until(deadline) => return st.stop_public_inner(false).await,
                            _ = tick.tick() => {
                                if !watched.alive().await {
                                    return st.fail_public("Tunnel terputus (proses cloudflared berhenti)").await;
                                }
                            }
                        }
                    }
                });
                let mut p = self.publik.lock().await;
                *p = Publik { status: PublikStatus::On, url: Some(url), expires_at: Some(expires), duration_hours: hours, error: None, timer: Some(timer) };
                drop(p);
                // 2FA wajib untuk semua selama publik aktif.
                let (r2, _) = self.share_flags();
                self.set_share(r2, true);
                Ok(())
            }
            Err(e) => {
                tunnel.stop().await;
                *self.publik.lock().await = Publik { status: PublikStatus::Error, duration_hours: hours, error: Some(e.clone()), ..Default::default() };
                Err(ApiError::new(axum::http::StatusCode::BAD_GATEWAY, "tunnel_failed", format!("Tunnel gagal: {e}")))
            }
        }
    }

    /// Tunnel mati sendiri: tampilkan galat dan cabut status publik (2FA tidak lagi dipaksa).
    async fn fail_public(self: &Arc<Self>, msg: &str) {
        let tunnel = self.tunnel.lock().expect("lock").clone();
        {
            let mut p = self.publik.lock().await;
            let hours = p.duration_hours;
            *p = Publik { status: PublikStatus::Error, duration_hours: hours, error: Some(msg.into()), ..Default::default() };
        }
        if let Some(t) = tunnel {
            t.stop().await;
        }
        let (r2, _) = self.share_flags();
        self.set_share(r2, false);
    }

    pub async fn stop_public(self: &Arc<Self>) {
        self.stop_public_inner(true).await;
    }

    async fn stop_public_inner(self: &Arc<Self>, abort_timer: bool) {
        let tunnel = self.tunnel.lock().expect("lock").clone();
        let was = {
            let mut p = self.publik.lock().await;
            if let Some(t) = p.timer.take()
                && abort_timer
            {
                t.abort();
            }
            let was = p.status;
            let hours = p.duration_hours;
            *p = Publik { duration_hours: hours, ..Default::default() };
            was
        };
        if matches!(was, PublikStatus::On | PublikStatus::Connecting)
            && let Some(t) = tunnel
        {
            t.stop().await;
        }
        let (r2, _) = self.share_flags();
        self.set_share(r2, false);
        // Sesi yang dibuat lewat publik tidak diputus; akses baru lewat tunnel sudah tertutup.
    }

    /// Nyalakan atau matikan server LAN (permukaan Remote: login akun wajib, ADR 0005).
    pub async fn set_lan(self: &Arc<Self>, enabled: bool, bind: &str, port: u16) -> Result<(), ApiError> {
        if !matches!(bind, "lan" | "localhost") {
            return Err(ApiError::bad("Bind harus lan atau localhost"));
        }
        if enabled && !(1024..=65535).contains(&port) {
            return Err(ApiError::bad("Port harus 1024-65535"));
        }
        if !enabled || self.lan.lock().await.running.is_some() {
            // Mematikan atau mengganti server LAN memutus tunnel yang mengarah ke sana.
            self.stop_public().await;
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
        let (require_2fa, _) = self.share_flags();
        let pub_state = {
            let p = self.publik.lock().await;
            let status = match p.status {
                PublikStatus::Off => "off",
                PublikStatus::Connecting => "connecting",
                PublikStatus::On => "on",
                PublikStatus::Error => "error",
            };
            json!({ "status": status, "url": if owner { p.url.clone() } else { None }, "durationHours": if p.duration_hours == 0 { 8 } else { p.duration_hours }, "expiresAt": p.expires_at, "error": p.error })
        };
        let settings = self.telemon.with_store(|s| s.get_settings()).unwrap_or_default();
        let lan = self.lan.lock().await;
        let enabled = lan.running.is_some();
        let url = lan.running.as_ref().map(|r| {
            let host = if lan.bind == "lan" { local_ip() } else { IpAddr::V4(Ipv4Addr::LOCALHOST) };
            format!("http://{}", SocketAddr::new(host, r.addr.port()))
        });
        json!({
            "lan": { "enabled": enabled, "bind": lan.bind, "port": lan.port, "url": if owner { url } else { None } },
            "publik": pub_state,
            "auth": { "require2fa": require_2fa, "allowPublicWrite": settings.remote.allow_public_write },
            "features": { "ai": self.ai_ready(), "aiMask": settings.ai.mask_sensitive },
        })
    }
}
