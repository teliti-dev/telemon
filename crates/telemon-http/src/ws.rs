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
use crate::guard::{COOKIE, client_info, cookie_value, origin_ok};
use crate::state::AppState;

const RECHECK: Duration = Duration::from_secs(30);

pub async fn handler<C: Connector>(State(st): State<Arc<AppState<C>>>, ConnectInfo(peer): ConnectInfo<SocketAddr>, headers: HeaderMap, ws: WebSocketUpgrade) -> Result<Response, ApiError> {
    if !origin_ok(&headers) {
        return Err(ApiError::forbidden("Asal permintaan tidak diizinkan"));
    }
    let token = cookie_value(&headers, COOKIE).ok_or_else(ApiError::unauthenticated)?;
    let auth = st.auth.clone();
    let t = token.clone();
    let mut session = tokio::task::spawn_blocking(move || auth.authenticate(&t)).await.map_err(ApiError::internal)??;
    session.via = client_info(&headers, peer).via;
    st.auth.authorize(Some(&session), Resource::Event, Level::Read)?;
    Ok(ws.on_upgrade(move |socket| stream(st, socket, token)))
}

async fn stream<C: Connector>(st: Arc<AppState<C>>, mut socket: WebSocket, token: String) {
    let mut rx = st.telemon.subscribe();
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
            _ = tick.tick() => {
                let auth = st.auth.clone();
                let t = token.clone();
                let ok = tokio::task::spawn_blocking(move || auth.authenticate(&t)).await.ok().and_then(Result::ok).is_some_and(|s| s.permissions.get(Resource::Event) >= Level::Read);
                if !ok {
                    let _ = socket.send(Message::Close(None)).await;
                    return;
                }
            }
        }
    }
}
