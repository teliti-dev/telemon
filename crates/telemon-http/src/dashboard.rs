//! Tata letak widget Ringkasan (satu untuk semua akun). Server hanya menyimpan dan memeriksa bentuknya;
//! jenis widget dikelola UI, jadi jenis yang tak dikenal diterima (UI mengabaikannya).
//!
//! - Baca: izin Ringkasan Lihat. Simpan atau kembalikan ke bawaan: izin Ringkasan Ubah.
//! - `layout: null` = hapus tata letak tersimpan, UI memakai susunan bawaan.

use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{Value, json};
use telemon_core::Connector;
use telemon_core::auth::{Level, Resource};

use crate::error::{ApiError, ApiResult};
use crate::guard::Caller;
use crate::state::AppState;

type St<C> = std::sync::Arc<AppState<C>>;

const KEY: &str = "dashboard.layout";
const MAX_WIDGETS: usize = 60;
const MAX_CONFIG_BYTES: usize = 2048;

pub fn routes<C: Connector>() -> Router<St<C>> {
    Router::new().route("/api/dashboard", get(get_layout::<C>).put(put_layout::<C>))
}

#[derive(Deserialize)]
struct PutReq {
    layout: Option<Value>,
}

async fn get_layout<C: Connector>(State(st): State<St<C>>, c: Caller) -> ApiResult<Json<Value>> {
    st.auth.authorize(Some(&c.session), Resource::Ringkasan, Level::Read).map_err(ApiError::from)?;
    let raw = st.telemon.with_store(|s| s.kv_get(KEY)).map_err(ApiError::internal)?;
    // Isi yang rusak dianggap tidak ada: UI kembali ke susunan bawaan.
    let layout = raw.and_then(|r| serde_json::from_str::<Value>(&r).ok()).filter(|v| validate(v).is_ok());
    Ok(Json(json!({ "layout": layout })))
}

async fn put_layout<C: Connector>(State(st): State<St<C>>, c: Caller, Json(req): Json<PutReq>) -> ApiResult<Json<Value>> {
    st.auth.authorize(Some(&c.session), Resource::Ringkasan, Level::Update).map_err(ApiError::from)?;
    match &req.layout {
        None => st.telemon.with_store(|s| s.kv_del(KEY)).map_err(ApiError::internal)?,
        Some(v) => {
            validate(v).map_err(ApiError::bad)?;
            let text = v.to_string();
            st.telemon.with_store(|s| s.kv_set(KEY, &text)).map_err(ApiError::internal)?;
        }
    }
    Ok(Json(json!({ "layout": req.layout })))
}

fn ident(s: &str, max: usize, allow_upper: bool) -> bool {
    !s.is_empty() && s.len() <= max && s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_' || (allow_upper && c.is_ascii_uppercase()))
}

/// Jenis bawaan: huruf kecil, angka, `-`, `_` (maks 40). Jenis dari extension (ADR 0028):
/// `ext:<id>:<jenis>` dengan `.` dan `:` tambahan (maks 140). Server tidak tahu extension mana yang ada;
/// UI mengabaikan jenis yang tidak dikenalnya.
fn widget_type_ok(t: &str) -> bool {
    if let Some(rest) = t.strip_prefix("ext:") {
        return t.len() <= 140 && rest.matches(':').count() == 1 && rest.split(':').all(|p| !p.is_empty()) && rest.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '-' | '_' | '.' | ':'));
    }
    ident(t, 40, false)
}

fn int(v: &Value, key: &str, min: i64, max: i64) -> Result<i64, String> {
    match v.get(key).and_then(Value::as_i64) {
        Some(n) if (min..=max).contains(&n) => Ok(n),
        _ => Err(format!("Kolom {key} harus bilangan bulat {min}-{max}")),
    }
}

/// Bentuk: `{ "version": 1, "widgets": [ { id, type, x, y, w, h, config? } ] }`.
pub(crate) fn validate(v: &Value) -> Result<(), String> {
    if v.get("version").and_then(Value::as_i64) != Some(1) {
        return Err("Versi tata letak tidak dikenal".into());
    }
    let widgets = v.get("widgets").and_then(Value::as_array).ok_or("Daftar widget tidak ada")?;
    if widgets.len() > MAX_WIDGETS {
        return Err(format!("Maksimal {MAX_WIDGETS} widget"));
    }
    let mut ids = std::collections::HashSet::new();
    for w in widgets {
        let id = w.get("id").and_then(Value::as_str).unwrap_or("");
        if !ident(id, 40, true) {
            return Err("Id widget tidak valid".into());
        }
        if !ids.insert(id.to_string()) {
            return Err("Id widget ganda".into());
        }
        if !widget_type_ok(w.get("type").and_then(Value::as_str).unwrap_or("")) {
            return Err("Jenis widget tidak valid".into());
        }
        int(w, "x", 0, 200)?;
        int(w, "y", 0, 10_000)?;
        int(w, "w", 1, 40)?;
        int(w, "h", 1, 2000)?;
        match w.get("config") {
            None | Some(Value::Null) => {}
            Some(c @ Value::Object(_)) => {
                if c.to_string().len() > MAX_CONFIG_BYTES {
                    return Err("Pengaturan widget terlalu besar".into());
                }
            }
            Some(_) => return Err("Pengaturan widget harus berupa objek".into()),
        }
    }
    Ok(())
}
