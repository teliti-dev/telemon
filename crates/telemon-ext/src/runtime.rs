//! Runtime WASM untuk backend extension (ADR 0028). Interpreter murni Rust (`wasmi`): tanpa JIT, tanpa C,
//! sehingga biner tetap kecil (ADR 0010) dan sandbox-nya sederhana. Modul tidak punya akses apa pun selain satu
//! impor, `telemon.call`, yang dijawab oleh `Host` (di server: dicek izin dan kuota di sana).
//!
//! ABI v1 (JSON di kedua arah, tanpa WASI):
//! - ekspor modul: `memory`, `telemon_alloc(size: i32) -> i32`, `telemon_handle(ptr: i32, len: i32) -> i64`
//!   (jawaban = `ptr << 32 | len` di memori modul);
//! - impor: `telemon.call(op_ptr, op_len, in_ptr, in_len, out_ptr, out_cap) -> i32`: menjalankan operasi host
//!   `op` dengan masukan JSON, menulis jawaban JSON ke `out`; mengembalikan panjangnya, atau `-(panjang)` bila
//!   `out_cap` kurang (modul mengulang dengan penyangga lebih besar).
//!
//! Batas per pemanggilan: bahan bakar (CPU), memori, ukuran permintaan dan jawaban, dan jumlah panggilan host.

use wasmi::{Caller, Config, Engine, Linker, Module, Store, StoreLimits, StoreLimitsBuilder};

/// Operasi host: masukan dan jawaban JSON. Jawaban galat memakai `{"error":{"code","message"}}`.
pub trait Host: Send {
    fn call(&mut self, op: &str, input: &[u8]) -> Vec<u8>;
}

#[derive(Debug, Clone)]
pub struct RunLimits {
    /// Bahan bakar wasmi (kira-kira satu per instruksi dasar).
    pub fuel: u64,
    pub memory_bytes: usize,
    pub max_request: usize,
    pub max_response: usize,
    pub max_host_calls: u32,
}

impl Default for RunLimits {
    fn default() -> Self {
        Self { fuel: 200_000_000, memory_bytes: 64 << 20, max_request: 256 << 10, max_response: 1 << 20, max_host_calls: 500 }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum RuntimeError {
    /// Modul tidak valid atau memakai impor/fitur yang tidak ada.
    Invalid(String),
    /// Ekspor wajib hilang atau bertanda tangan salah.
    NoExport(&'static str),
    OutOfFuel,
    Trap(String),
    TooLarge(&'static str),
    BadResponse,
}

impl std::fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Invalid(m) => write!(f, "Modul backend tidak valid: {m}"),
            Self::NoExport(n) => write!(f, "Modul backend tidak punya ekspor {n} yang benar"),
            Self::OutOfFuel => write!(f, "Backend melewati batas waktu CPU"),
            Self::Trap(m) => write!(f, "Backend berhenti dengan galat: {m}"),
            Self::TooLarge(what) => write!(f, "{what} terlalu besar"),
            Self::BadResponse => write!(f, "Jawaban backend tidak valid"),
        }
    }
}

impl std::error::Error for RuntimeError {}

struct State {
    host: Box<dyn Host>,
    limits: StoreLimits,
    calls: u32,
    max_calls: u32,
    max_response: usize,
}

/// Modul yang sudah dikompilasi dan divalidasi; murah dipakai berulang (satu instans baru per panggilan).
pub struct Sandbox {
    engine: Engine,
    module: Module,
}

impl Sandbox {
    pub fn new(wasm: &[u8]) -> Result<Self, RuntimeError> {
        let mut cfg = Config::default();
        cfg.consume_fuel(true);
        let engine = Engine::new(&cfg);
        let module = Module::new(&engine, wasm).map_err(|e| RuntimeError::Invalid(e.to_string()))?;
        let sb = Self { engine, module };
        // Impor yang tidak dikenal ditolak sejak awal (bukan baru saat dipanggil).
        for imp in sb.module.imports() {
            if imp.module() != "telemon" || imp.name() != "call" {
                return Err(RuntimeError::Invalid(format!("impor tidak diizinkan: {}.{}", imp.module(), imp.name())));
            }
        }
        for need in ["memory", "telemon_alloc", "telemon_handle"] {
            if sb.module.get_export(need).is_none() {
                return Err(RuntimeError::NoExport(match need {
                    "memory" => "memory",
                    "telemon_alloc" => "telemon_alloc",
                    _ => "telemon_handle",
                }));
            }
        }
        Ok(sb)
    }

    /// Satu pemanggilan: `request` (JSON) → jawaban (JSON). Instans baru tiap kali, jadi tidak ada keadaan yang
    /// bocor antar pemanggilan; keadaan tahan lama lewat operasi host (`kv.*`, `db.*`).
    pub fn invoke(&self, host: Box<dyn Host>, request: &[u8], limits: &RunLimits) -> Result<Vec<u8>, RuntimeError> {
        if request.len() > limits.max_request {
            return Err(RuntimeError::TooLarge("Permintaan"));
        }
        let st = State { host, limits: StoreLimitsBuilder::new().memory_size(limits.memory_bytes).instances(1).build(), calls: 0, max_calls: limits.max_host_calls, max_response: limits.max_response };
        let mut store = Store::new(&self.engine, st);
        store.limiter(|s| &mut s.limits);
        store.set_fuel(limits.fuel).map_err(|e| RuntimeError::Trap(e.to_string()))?;
        let mut linker = <Linker<State>>::new(&self.engine);
        linker
            .func_wrap("telemon", "call", |mut caller: Caller<'_, State>, op_ptr: i32, op_len: i32, in_ptr: i32, in_len: i32, out_ptr: i32, out_cap: i32| -> i32 {
                host_call(&mut caller, op_ptr, op_len, in_ptr, in_len, out_ptr, out_cap)
            })
            .map_err(|e| RuntimeError::Invalid(e.to_string()))?;
        let instance = linker.instantiate_and_start(&mut store, &self.module).map_err(|e| classify(&e.to_string()))?;
        let memory = instance.get_memory(&store, "memory").ok_or(RuntimeError::NoExport("memory"))?;
        let alloc = instance.get_typed_func::<i32, i32>(&store, "telemon_alloc").map_err(|_| RuntimeError::NoExport("telemon_alloc"))?;
        let handle = instance.get_typed_func::<(i32, i32), i64>(&store, "telemon_handle").map_err(|_| RuntimeError::NoExport("telemon_handle"))?;
        let ptr = alloc.call(&mut store, request.len() as i32).map_err(|e| classify(&e.to_string()))?;
        memory.write(&mut store, ptr as u32 as usize, request).map_err(|_| RuntimeError::Trap("alokasi di luar memori".into()))?;
        let packed = handle.call(&mut store, (ptr, request.len() as i32)).map_err(|e| classify(&e.to_string()))? as u64;
        let (rp, rl) = ((packed >> 32) as usize, (packed & 0xffff_ffff) as usize);
        if rl > limits.max_response {
            return Err(RuntimeError::TooLarge("Jawaban"));
        }
        let mut out = vec![0u8; rl];
        memory.read(&store, rp, &mut out).map_err(|_| RuntimeError::BadResponse)?;
        Ok(out)
    }
}

fn classify(msg: &str) -> RuntimeError {
    if msg.to_ascii_lowercase().contains("fuel") { RuntimeError::OutOfFuel } else { RuntimeError::Trap(msg.to_string()) }
}

fn host_call(caller: &mut Caller<'_, State>, op_ptr: i32, op_len: i32, in_ptr: i32, in_len: i32, out_ptr: i32, out_cap: i32) -> i32 {
    let err = |code: &str, message: &str| format!(r#"{{"error":{{"code":"{code}","message":"{message}"}}}}"#).into_bytes();
    let Some(memory) = caller.get_export("memory").and_then(|e| e.into_memory()) else { return 0 };
    let read = |caller: &Caller<'_, State>, ptr: i32, len: i32, max: usize| -> Option<Vec<u8>> {
        let (ptr, len) = (ptr as u32 as usize, len as u32 as usize);
        if len > max {
            return None;
        }
        let mut buf = vec![0u8; len];
        memory.read(caller, ptr, &mut buf).ok()?;
        Some(buf)
    };
    caller.data_mut().calls += 1;
    let reply = if caller.data().calls > caller.data().max_calls {
        err("quota", "Terlalu banyak panggilan host dalam satu eksekusi")
    } else {
        match (read(caller, op_ptr, op_len, 64), read(caller, in_ptr, in_len, 256 << 10)) {
            (Some(op), Some(input)) => match String::from_utf8(op) {
                Ok(op) => {
                    let mut r = caller.data_mut().host.call(&op, &input);
                    if r.len() > caller.data().max_response {
                        r = err("too_large", "Jawaban host terlalu besar");
                    }
                    r
                }
                Err(_) => err("bad_request", "Nama operasi bukan UTF-8"),
            },
            _ => err("bad_request", "Penunjuk atau ukuran masukan tidak valid"),
        }
    };
    if reply.len() > out_cap.max(0) as usize {
        return -(reply.len() as i32);
    }
    if memory.write(&mut *caller, out_ptr as u32 as usize, &reply).is_err() {
        return 0;
    }
    reply.len() as i32
}
