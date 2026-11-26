use std::sync::{Arc, Mutex};

use telemon_ext::*;

/// Alokator bump + `handle` yang memanggil sebuah kode WAT.
fn module(handle_body: &str, extra: &str) -> Vec<u8> {
    wat::parse_str(format!(
        r#"(module
  (import "telemon" "call" (func $call (param i32 i32 i32 i32 i32 i32) (result i32)))
  (memory (export "memory") 2)
  (global $bump (mut i32) (i32.const 65536))
  (data (i32.const 0) "kv.get")
  (data (i32.const 16) "{{\"key\":\"a\"}}")
  (func (export "telemon_alloc") (param $n i32) (result i32)
    (local $p i32)
    (local.set $p (global.get $bump))
    (global.set $bump (i32.add (global.get $bump) (local.get $n)))
    (local.get $p))
  {extra}
  (func (export "telemon_handle") (param $ptr i32) (param $len i32) (result i64)
    {handle_body}))"#
    ))
    .unwrap()
}

const ECHO: &str = "(i64.or (i64.shl (i64.extend_i32_u (local.get $ptr)) (i64.const 32)) (i64.extend_i32_u (local.get $len)))";

struct Rec(Arc<Mutex<Vec<(String, String)>>>, Vec<u8>);

impl Host for Rec {
    fn call(&mut self, op: &str, input: &[u8]) -> Vec<u8> {
        self.0.lock().unwrap().push((op.to_string(), String::from_utf8_lossy(input).to_string()));
        self.1.clone()
    }
}

fn rec(reply: &str) -> (Box<dyn Host>, Arc<Mutex<Vec<(String, String)>>>) {
    let log = Arc::new(Mutex::new(Vec::new()));
    (Box::new(Rec(log.clone(), reply.as_bytes().to_vec())), log)
}

#[test]
fn echo_roundtrip_and_isolation_between_calls() {
    let sb = Sandbox::new(&module(ECHO, "")).unwrap();
    let (h, _) = rec("{}");
    assert_eq!(sb.invoke(h, br#"{"kind":"http"}"#, &RunLimits::default()).unwrap(), br#"{"kind":"http"}"#);
    // Instans baru tiap panggilan: alokator bump mulai lagi, jawabannya tetap benar.
    let (h, _) = rec("{}");
    assert_eq!(sb.invoke(h, b"{\"n\":2}", &RunLimits::default()).unwrap(), b"{\"n\":2}");
}

#[test]
fn host_call_reaches_the_host_with_op_and_input() {
    let body = "(i64.or (i64.shl (i64.const 1024) (i64.const 32)) (i64.extend_i32_u (call $call (i32.const 0) (i32.const 6) (i32.const 16) (i32.const 11) (i32.const 1024) (i32.const 1024))))";
    let sb = Sandbox::new(&module(body, "")).unwrap();
    let (h, log) = rec(r#"{"ok":{"value":"x"}}"#);
    let out = sb.invoke(h, b"{}", &RunLimits::default()).unwrap();
    assert_eq!(out, br#"{"ok":{"value":"x"}}"#);
    assert_eq!(log.lock().unwrap().as_slice(), &[("kv.get".to_string(), r#"{"key":"a"}"#.to_string())]);
}

#[test]
fn small_out_buffer_returns_negative_needed_size() {
    // Penyangga 4 byte: modul mendapat -(panjang jawaban) dan mengembalikannya sebagai panjang (negatif -> 0xffff...).
    let body = "(i64.extend_i32_s (call $call (i32.const 0) (i32.const 6) (i32.const 16) (i32.const 11) (i32.const 1024) (i32.const 4)))";
    let sb = Sandbox::new(&module(body, "")).unwrap();
    let (h, _) = rec("0123456789");
    let r = sb.invoke(h, b"{}", &RunLimits::default());
    assert!(matches!(r, Err(RuntimeError::TooLarge(_))), "panjang jawaban yang diklaim melebihi batas: {r:?}");
}

#[test]
fn infinite_loop_runs_out_of_fuel() {
    let sb = Sandbox::new(&module("(loop $l (br $l)) (i64.const 0)", "")).unwrap();
    let (h, _) = rec("{}");
    let r = sb.invoke(h, b"{}", &RunLimits { fuel: 100_000, ..Default::default() });
    assert_eq!(r, Err(RuntimeError::OutOfFuel));
}

#[test]
fn memory_growth_is_capped() {
    // Minta 100 halaman (6,4 MB) dengan batas 1 MB: memory.grow gagal (-1); jawaban = hasil itu.
    let body = "(i64.extend_i32_s (memory.grow (i32.const 100)))";
    let sb = Sandbox::new(&module(body, "")).unwrap();
    let (h, _) = rec("{}");
    // Modul memulai dengan 2 halaman (128 KiB), jadi batas 1 MiB masih cukup untuk instansiasi.
    let r = sb.invoke(h, b"{}", &RunLimits { memory_bytes: 1 << 20, ..Default::default() });
    // -1 dipakai sebagai (ptr<<32|len): len terlalu besar, ditolak sebagai Jawaban terlalu besar.
    assert!(matches!(r, Err(RuntimeError::TooLarge(_))), "{r:?}");
    // Tanpa batas ketat, grow berhasil (hasil = jumlah halaman lama = 2), jadi len=2 dan ptr=0.
    let (h, _) = rec("{}");
    assert!(sb.invoke(h, b"{}", &RunLimits::default()).is_ok());
}

#[test]
fn only_the_telemon_call_import_is_allowed() {
    let wasm = wat::parse_str(r#"(module (import "wasi_snapshot_preview1" "fd_write" (func (param i32 i32 i32 i32) (result i32))) (memory (export "memory") 1) (func (export "telemon_alloc") (param i32) (result i32) (i32.const 0)) (func (export "telemon_handle") (param i32 i32) (result i64) (i64.const 0)))"#).unwrap();
    assert!(matches!(Sandbox::new(&wasm), Err(RuntimeError::Invalid(_))), "WASI dan impor lain ditolak");
    let wasm = wat::parse_str(r#"(module (import "telemon" "secret" (func)) (memory (export "memory") 1) (func (export "telemon_alloc") (param i32) (result i32) (i32.const 0)) (func (export "telemon_handle") (param i32 i32) (result i64) (i64.const 0)))"#).unwrap();
    assert!(matches!(Sandbox::new(&wasm), Err(RuntimeError::Invalid(_))));
}

#[test]
fn required_exports_and_valid_wasm() {
    assert!(matches!(Sandbox::new(b"bukan wasm"), Err(RuntimeError::Invalid(_))));
    let no_handle = wat::parse_str(r#"(module (memory (export "memory") 1) (func (export "telemon_alloc") (param i32) (result i32) (i32.const 0)))"#).unwrap();
    assert_eq!(Sandbox::new(&no_handle).err(), Some(RuntimeError::NoExport("telemon_handle")));
    let no_mem = wat::parse_str(r#"(module (func (export "telemon_alloc") (param i32) (result i32) (i32.const 0)) (func (export "telemon_handle") (param i32 i32) (result i64) (i64.const 0)))"#).unwrap();
    assert_eq!(Sandbox::new(&no_mem).err(), Some(RuntimeError::NoExport("memory")));
}

#[test]
fn request_and_response_sizes_are_limited() {
    let sb = Sandbox::new(&module(ECHO, "")).unwrap();
    let (h, _) = rec("{}");
    let big = vec![b'x'; 2000];
    assert_eq!(sb.invoke(h, &big, &RunLimits { max_request: 1000, ..Default::default() }), Err(RuntimeError::TooLarge("Permintaan")));
    // Jawaban (echo 2000 byte) lebih besar dari batas jawaban 1000.
    let (h, _) = rec("{}");
    assert_eq!(sb.invoke(h, &big, &RunLimits { max_response: 1000, ..Default::default() }), Err(RuntimeError::TooLarge("Jawaban")));
}

#[test]
fn host_call_quota_is_enforced_by_the_runtime() {
    // Modul memanggil host 5 kali berturut-turut dan mengembalikan jawaban terakhir; batas 3 panggilan.
    let call = "(drop (call $call (i32.const 0) (i32.const 6) (i32.const 16) (i32.const 11) (i32.const 1024) (i32.const 1024)))";
    let body = format!("{call}{call}{call}{call}(i64.or (i64.shl (i64.const 1024) (i64.const 32)) (i64.extend_i32_u (call $call (i32.const 0) (i32.const 6) (i32.const 16) (i32.const 11) (i32.const 1024) (i32.const 1024))))");
    let sb = Sandbox::new(&module(&body, "")).unwrap();
    let (h, log) = rec(r#"{"ok":1}"#);
    let out = sb.invoke(h, b"{}", &RunLimits { max_host_calls: 3, ..Default::default() }).unwrap();
    assert!(String::from_utf8_lossy(&out).contains("quota"), "{}", String::from_utf8_lossy(&out));
    assert_eq!(log.lock().unwrap().len(), 3, "panggilan ke-4 dan seterusnya tidak sampai ke host");
}
