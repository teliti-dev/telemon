//! Data bandwidth untuk widget Ringkasan (ADR 0026): daftar antarmuka dan deret waktu laju.
//!
//! Hanya membaca, izin Router Lihat. Antarmuka virtual `@pppoe` = jumlah semua sesi PPPoE satu router;
//! hanya itu yang boleh dijumlah lintas router (antarmuka fisik akan terhitung ganda).

use axum::extract::{Query, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{Value, json};
use telemon_core::Connector;
use telemon_core::auth::{Level, Resource};
use telemon_core::traffic::PPPOE_AGGREGATE;

use crate::error::{ApiError, ApiResult};
use crate::guard::Caller;
use crate::state::AppState;

type St<C> = std::sync::Arc<AppState<C>>;

/// Jumlah titik maksimum per deret; lebar ember menyesuaikan rentang.
const MAX_POINTS: i64 = 240;
const MAX_IFACE_LEN: usize = 64;

pub fn routes<C: Connector>() -> Router<St<C>> {
    Router::new().route("/api/traffic/interfaces", get(interfaces::<C>)).route("/api/traffic/series", get(series::<C>))
}

#[derive(Deserialize)]
struct IfacesQuery {
    router: String,
}

#[derive(Deserialize)]
struct SeriesQuery {
    /// Id router, atau `all` (total semua router, hanya untuk `@pppoe`).
    router: String,
    iface: String,
    range: String,
}

fn range_ms(r: &str) -> Option<i64> {
    const H: i64 = 3_600_000;
    Some(match r {
        "1h" => H,
        "6h" => 6 * H,
        "24h" => 24 * H,
        "7d" => 168 * H,
        "30d" => 720 * H,
        _ => return None,
    })
}

fn settings<C: Connector>(st: &AppState<C>) -> Result<telemon_core::Settings, ApiError> {
    st.telemon.with_store(|s| s.get_settings()).map_err(ApiError::internal)
}

async fn interfaces<C: Connector>(State(st): State<St<C>>, c: Caller, Query(q): Query<IfacesQuery>) -> ApiResult<Json<Value>> {
    st.auth.authorize(Some(&c.session), Resource::Router, Level::Read).map_err(ApiError::from)?;
    let router = st.telemon.with_store(|s| s.get_router(&q.router)).map_err(ApiError::internal)?;
    if router.is_none() {
        return Err(ApiError::not_found());
    }
    let rows = st.telemon.with_store(|s| s.list_traffic_ifaces(&q.router)).map_err(ApiError::internal)?;
    let cfg = settings(&st)?;
    let list: Vec<Value> = rows.iter().map(|i| json!({ "name": i.name, "kind": i.kind, "running": i.running, "lastSeen": i.last_seen })).collect();
    Ok(Json(json!({ "interfaces": list, "enabled": cfg.bandwidth.enabled, "intervalSeconds": cfg.bandwidth.interval_seconds })))
}

async fn series<C: Connector>(State(st): State<St<C>>, c: Caller, Query(q): Query<SeriesQuery>) -> ApiResult<Json<Value>> {
    st.auth.authorize(Some(&c.session), Resource::Router, Level::Read).map_err(ApiError::from)?;
    let span = range_ms(&q.range).ok_or_else(|| ApiError::bad("Rentang harus 1h, 6h, 24h, 7d, atau 30d"))?;
    if q.iface.is_empty() || q.iface.len() > MAX_IFACE_LEN {
        return Err(ApiError::bad("Nama antarmuka tidak valid"));
    }
    let all = q.router == "all";
    if all && q.iface != PPPOE_AGGREGATE {
        return Err(ApiError::bad("Total semua router hanya untuk antarmuka @pppoe"));
    }
    if !all && st.telemon.with_store(|s| s.get_router(&q.router)).map_err(ApiError::internal)?.is_none() {
        return Err(ApiError::not_found());
    }
    let cfg = settings(&st)?;
    let interval_ms = i64::from(cfg.bandwidth.interval_seconds) * 1000;
    // Ember tidak lebih sempit dari interval pengambilan sampel, dan titiknya tidak melebihi MAX_POINTS.
    let bucket = interval_ms.max((span + MAX_POINTS - 1) / MAX_POINTS);
    let to = st.now();
    let from = to - span;
    let router = (!all).then_some(q.router.as_str());
    let points = st.telemon.with_store(|s| s.traffic_series(router, &q.iface, from, to, bucket)).map_err(ApiError::internal)?;
    let points: Vec<Value> = points
        .iter()
        .map(|p| {
            let mut v = json!({ "at": p.at, "rxBps": p.rx_bps.round() as i64, "txBps": p.tx_bps.round() as i64 });
            if let Some(s) = p.sessions {
                v["sessions"] = json!((s * 10.0).round() / 10.0);
            }
            v
        })
        .collect();
    Ok(Json(json!({ "points": points, "from": from, "to": to, "bucketSeconds": bucket / 1000, "intervalSeconds": cfg.bandwidth.interval_seconds, "enabled": cfg.bandwidth.enabled })))
}
