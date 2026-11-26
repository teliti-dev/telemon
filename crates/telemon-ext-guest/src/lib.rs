//! SDK penulis backend extension Telemon (ADR 0028), lisensi permisif agar extension tidak terikat AGPL.
//!
//! ```ignore
//! use telemon_ext_guest::{entry, Request, Response, host};
//! fn handle(req: Request) -> Response {
//!     match req.route.as_deref() {
//!         Some("hello") => Response::ok(serde_json::json!({ "hello": "world" })),
//!         _ => Response::error(404, "not_found", "unknown route"),
//!     }
//! }
//! entry!(handle);
//! ```
//! Kompilasi: `cargo build --release --target wasm32-unknown-unknown` (crate bertipe `cdylib`, tanpa WASI).
//! Modul hanya bisa memakai operasi host di `host`; yang butuh izin dijawab `permission_denied` bila admin
//! mematikannya.

use serde_json::{Value, json};

/// Permintaan yang diterima backend: `kind` = `http` (rute) atau `worker` (tugas terjadwal).
#[derive(Debug, Clone)]
pub struct Request {
    pub kind: String,
    /// Id rute (kind `http`).
    pub route: Option<String>,
    pub method: Option<String>,
    pub path: Option<String>,
    /// Parameter query sebagai objek `{nama: teks}`.
    pub query: Value,
    pub body: Value,
    /// Pengguna yang memanggil (`id`, `username`, `displayName`, `roleId`); `null` untuk worker dan desktop.
    pub user: Value,
    /// Id worker (kind `worker`).
    pub worker: Option<String>,
    /// Waktu server (ms sejak epoch) untuk worker.
    pub now: i64,
}

impl Request {
    pub fn query_str(&self, key: &str) -> Option<&str> {
        self.query.get(key).and_then(Value::as_str)
    }

    fn from_json(v: &Value) -> Self {
        let s = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
        Self {
            kind: s("kind").unwrap_or_default(),
            route: s("route"),
            method: s("method"),
            path: s("path"),
            query: v.get("query").cloned().unwrap_or(Value::Null),
            body: v.get("body").cloned().unwrap_or(Value::Null),
            user: v.get("user").cloned().unwrap_or(Value::Null),
            worker: s("worker"),
            now: v.get("now").and_then(Value::as_i64).unwrap_or(0),
        }
    }
}

/// Jawaban: `status` 2xx atau 4xx dan `body` JSON.
#[derive(Debug, Clone)]
pub struct Response {
    pub status: u16,
    pub body: Value,
}

impl Response {
    pub fn ok(body: Value) -> Self {
        Self { status: 200, body }
    }

    pub fn error(status: u16, code: &str, message: &str) -> Self {
        Self { status, body: json!({ "error": { "code": code, "message": message } }) }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostError {
    pub code: String,
    pub message: String,
}

impl std::fmt::Display for HostError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

#[cfg(target_arch = "wasm32")]
#[link(wasm_import_module = "telemon")]
unsafe extern "C" {
    #[link_name = "call"]
    fn host_call(op_ptr: *const u8, op_len: i32, in_ptr: *const u8, in_len: i32, out_ptr: *mut u8, out_cap: i32) -> i32;
}

/// Di luar WASM (tes unit penulis extension) tidak ada host: setiap panggilan gagal dengan `no_host`.
#[cfg(not(target_arch = "wasm32"))]
unsafe fn host_call(_: *const u8, _: i32, _: *const u8, _: i32, _: *mut u8, _: i32) -> i32 {
    0
}

/// Operasi host mentah. Masukan dan jawaban JSON; jawaban `{"error":{...}}` menjadi `Err`.
pub fn call(op: &str, input: &Value) -> Result<Value, HostError> {
    let input = input.to_string();
    let mut cap = 64 * 1024usize;
    for _ in 0..2 {
        let mut out = vec![0u8; cap];
        // SAFETY: penunjuk dan panjang berasal dari irisan/Vec yang hidup selama panggilan.
        let n = unsafe { host_call(op.as_ptr(), op.len() as i32, input.as_ptr(), input.len() as i32, out.as_mut_ptr(), cap as i32) };
        if n < 0 {
            cap = (-n) as usize + 16;
            continue;
        }
        if n == 0 {
            return Err(HostError { code: "no_host".into(), message: "Host tidak menjawab".into() });
        }
        out.truncate(n as usize);
        let v: Value = serde_json::from_slice(&out).map_err(|_| HostError { code: "bad_response".into(), message: "Jawaban host bukan JSON".into() })?;
        if let Some(e) = v.get("error") {
            return Err(HostError { code: e.get("code").and_then(Value::as_str).unwrap_or("error").into(), message: e.get("message").and_then(Value::as_str).unwrap_or("").into() });
        }
        return Ok(v.get("ok").cloned().unwrap_or(Value::Null));
    }
    Err(HostError { code: "too_large".into(), message: "Jawaban host terlalu besar".into() })
}

/// Operasi host bertipe.
pub mod host {
    use super::{HostError, call};
    use serde_json::{Value, json};

    pub fn time_now() -> Result<i64, HostError> {
        Ok(call("time.now", &json!({}))?.as_i64().unwrap_or(0))
    }

    pub fn log(message: &str) {
        let _ = call("log", &json!({ "message": message }));
    }

    pub fn kv_get(key: &str) -> Result<Option<String>, HostError> {
        Ok(call("kv.get", &json!({ "key": key }))?.as_str().map(str::to_string))
    }

    pub fn kv_set(key: &str, value: &str) -> Result<(), HostError> {
        call("kv.set", &json!({ "key": key, "value": value })).map(|_| ())
    }

    pub fn kv_delete(key: &str) -> Result<bool, HostError> {
        Ok(call("kv.delete", &json!({ "key": key }))?.as_bool().unwrap_or(false))
    }

    pub fn kv_list(prefix: &str, limit: usize) -> Result<Vec<String>, HostError> {
        Ok(call("kv.list", &json!({ "prefix": prefix, "limit": limit }))?.as_array().map(|a| a.iter().filter_map(|k| k.as_str().map(str::to_string)).collect()).unwrap_or_default())
    }

    /// `SELECT` pada tabel extension ini; baris sebagai array nilai berurutan sesuai `columns`.
    pub fn db_query(sql: &str, params: &[Value], limit: usize) -> Result<Value, HostError> {
        call("db.query", &json!({ "sql": sql, "params": params, "limit": limit }))
    }

    /// `INSERT`/`UPDATE`/`DELETE` pada tabel extension ini; mengembalikan jumlah baris terdampak.
    pub fn db_exec(sql: &str, params: &[Value]) -> Result<u64, HostError> {
        Ok(call("db.exec", &json!({ "sql": sql, "params": params }))?.get("changes").and_then(Value::as_u64).unwrap_or(0))
    }

    /// Butuh izin `data.routers`.
    pub fn routers_list() -> Result<Value, HostError> {
        call("routers.list", &json!({}))
    }

    /// Butuh izin `data.routers`; hanya jalur baca yang diizinkan host (`/system/resource`, `/interface`, dst.).
    pub fn router_print(router: &str, path: &str) -> Result<Value, HostError> {
        call("router.print", &json!({ "router": router, "path": path }))
    }

    /// Butuh izin `data.events`.
    pub fn events_list(input: &Value) -> Result<Value, HostError> {
        call("events.list", input)
    }
}

#[doc(hidden)]
pub fn alloc(n: i32) -> i32 {
    let mut v: Vec<u8> = Vec::with_capacity(n.max(0) as usize);
    let p = v.as_mut_ptr();
    std::mem::forget(v);
    p as i32
}

#[doc(hidden)]
pub fn handle(ptr: i32, len: i32, f: fn(Request) -> Response) -> i64 {
    // SAFETY: host menulis `len` byte ke `ptr` yang dialokasikan oleh `alloc` sebelum memanggil kita.
    let bytes = unsafe { std::slice::from_raw_parts(ptr as *const u8, len.max(0) as usize) };
    let req = serde_json::from_slice::<Value>(bytes).map(|v| Request::from_json(&v));
    let resp = match req {
        Ok(r) => f(r),
        Err(_) => Response::error(400, "bad_request", "Permintaan bukan JSON"),
    };
    let out = json!({ "status": resp.status, "body": resp.body }).to_string().into_bytes();
    let (p, l) = (out.as_ptr() as usize as i64, out.len() as i64);
    std::mem::forget(out);
    (p << 32) | l
}

/// Pasang titik masuk: mengekspor `telemon_alloc` dan `telemon_handle` untuk fungsi `fn(Request) -> Response`.
#[macro_export]
macro_rules! entry {
    ($handler:path) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn telemon_alloc(n: i32) -> i32 {
            $crate::alloc(n)
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn telemon_handle(ptr: i32, len: i32) -> i64 {
            $crate::handle(ptr, len, $handler)
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_parses_http_and_worker_shapes() {
        let r = Request::from_json(&json!({ "kind": "http", "route": "status", "method": "GET", "path": "/status", "query": { "router": "r1" }, "body": null, "user": { "username": "a" } }));
        assert_eq!((r.kind.as_str(), r.route.as_deref(), r.query_str("router")), ("http", Some("status"), Some("r1")));
        let w = Request::from_json(&json!({ "kind": "worker", "worker": "sample", "now": 5 }));
        assert_eq!((w.worker.as_deref(), w.now, w.route), (Some("sample"), 5, None));
    }

    #[test]
    fn host_calls_fail_cleanly_outside_wasm() {
        assert_eq!(host::kv_get("a").unwrap_err().code, "no_host");
    }
}
