//! WebSocket event live (ADR 0003). Cookie sesi dan Origin diperiksa sebelum upgrade; izin Event
//! dibaca ulang berkala sehingga sesi yang berakhir atau dicabut memutus aliran.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{ConnectInfo, State};
use axum::http::HeaderMap;
use axum::response::Response;
use serde_json::json;
use telemon_core::auth::{Level, Resource};
use telemon_core::Connector;
use tokio::sync::broadcast::error::RecvError;

use crate::dto::EventDto;
use crate::error::ApiError;
use crate::guard::{Surface, origin_ok, resolve_caller};
use crate::state::AppState;

const RECHECK: Duration = Duration::from_secs(30);

pub async fn handler<C: Connector>(
    State(st): State<Arc<AppState<C>>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    axum::Extension(surface): axum::Extension<Surface>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Result<Response, ApiError> {
    if !origin_ok(&headers) {
        return Err(ApiError::forbidden("Asal permintaan tidak diizinkan"));
    }
    let caller = resolve_caller(&st, &surface, &headers, peer).await?;
    st.auth.authorize(Some(&caller.session), Resource::Event, Level::Read)?;
    Ok(ws.on_upgrade(move |socket| stream(st, socket, surface, headers, peer)))
}

async fn stream<C: Connector>(st: Arc<AppState<C>>, mut socket: WebSocket, surface: Surface, headers: HeaderMap, peer: SocketAddr) {
    let mut rx = st.telemon.subscribe();
    let mut epoch = st.lan_epoch.subscribe();
    let remote = matches!(surface, Surface::Remote);
    let mut tick = tokio::time::interval(RECHECK);
    tick.tick().await;
    loop {
        tokio::select! {
            ev = rx.recv() => match ev {
                Ok(e) => {
                    let dto = EventDto::from_event(e.id.to_string(), &e.router_id, &e.event);
                    let msg = json!({ "kind": "event", "event": dto }).to_string();
                    if socket.send(Message::Text(msg.into())).await.is_err() {
                        return;
                    }
                }
                // Penerima tertinggal: klien memuat ulang snapshot.
                Err(RecvError::Lagged(_)) => {
                    if socket.send(Message::Text(json!({ "kind": "resync" }).to_string().into())).await.is_err() {
                        return;
                    }
                }
                Err(RecvError::Closed) => return,
            },
            msg = socket.recv() => match msg {
                Some(Ok(Message::Close(_))) | None | Some(Err(_)) => return,
                Some(Ok(_)) => {}
            },
            // Server LAN dimatikan atau diganti: sambungan dari sana ikut ditutup.
            _ = epoch.changed(), if remote => {
                let _ = socket.send(Message::Close(None)).await;
                return;
            }
            _ = tick.tick() => {
                let ok = resolve_caller(&st, &surface, &headers, peer).await.is_ok_and(|c| c.session.permissions.get(Resource::Event) >= Level::Read);
                if !ok {
                    let _ = socket.send(Message::Close(None)).await;
                    return;
                }
            }
        }
    }
}
