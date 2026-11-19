use std::path::PathBuf;
use std::sync::Arc;

use telemon_core::store::NewRouter;
use telemon_core::SecretStore;
use telemon_desktop::secrets::FileSecrets;
use telemon_desktop::{Desktop, DesktopConfig, StartError};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("telemon-desktop-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn cfg(dir: &PathBuf, port: u16) -> DesktopConfig {
    DesktopConfig { db_path: dir.join("data").join("telemon.db"), static_dir: None, secrets: Arc::new(FileSecrets::open(dir.join("secrets.json")).unwrap()), port }
}

/// GET mentah ke server loopback; mengembalikan seluruh respons (status, header, badan).
async fn get(port: u16, path: &str, cookie: Option<&str>) -> String {
    let mut s = tokio::net::TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    let ck = cookie.map(|c| format!("Cookie: {c}\r\n")).unwrap_or_default();
    s.write_all(format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n{ck}Connection: close\r\n\r\n").as_bytes()).await.unwrap();
    let mut out = String::new();
    s.read_to_string(&mut out).await.unwrap();
    out
}

fn status(r: &str) -> &str {
    r.lines().next().unwrap_or("")
}

#[tokio::test]
async fn meluncurkan_server_loopback_dengan_token() {
    let dir = tmp("start");
    let d = Desktop::start(cfg(&dir, 0)).await.unwrap();
    assert!(d.addr.ip().is_loopback(), "server desktop hanya boleh di loopback");
    let port = d.addr.port();
    assert_eq!(d.token.len(), 64);

    // Tanpa token: tidak ada data.
    assert!(status(&get(port, "/api/snapshot", None).await).contains("401"));
    // Token salah ditolak; token benar menukar dengan cookie dan mengalihkan ke UI.
    assert!(status(&get(port, "/desktop-entry?t=salah", None).await).contains("403"));
    let entry = get(port, &d.entry_url().replace(&format!("http://127.0.0.1:{port}"), ""), None).await;
    assert!(status(&entry).contains("303"), "{entry}");
    let cookie_line = entry.lines().find(|l| l.to_lowercase().starts_with("set-cookie:")).expect("cookie dipasang");
    assert!(cookie_line.contains("HttpOnly") && cookie_line.contains("SameSite=Strict"));
    let cookie = cookie_line.split_once(':').unwrap().1.split(';').next().unwrap().trim().to_string();

    let health = get(port, "/api/health", Some(&cookie)).await;
    assert!(health.contains("\"desktop\":true"), "{health}");
    let snap = get(port, "/api/snapshot", Some(&cookie)).await;
    assert!(status(&snap).contains("200") && snap.contains("\"routers\":[]") && snap.contains("\"share\""), "{snap}");
    d.shutdown().await;
    assert!(tokio::net::TcpStream::connect(("127.0.0.1", port)).await.is_err(), "port ditutup setelah shutdown");
}

#[tokio::test]
async fn data_dan_kunci_bertahan_setelah_restart() {
    let dir = tmp("restart");
    let d = Desktop::start(cfg(&dir, 0)).await.unwrap();
    let r = d
        .telemon
        .add_router(&NewRouter { name: "pusat".into(), host: "127.0.0.1".into(), port: 1, tls_pin: None, username: "api".into(), can_write: true }, "pw-router")
        .unwrap();
    d.telemon.with_store(|s| s.set_user_password(&format!("{}:budi", r.id), "rahasia123", 1)).unwrap();
    d.shutdown().await;

    // Kunci dan kata sandi router ada di berkas rahasia; basis data tidak memuatnya.
    let secrets = std::fs::read_to_string(dir.join("secrets.json")).unwrap();
    assert!(secrets.contains("database-key") && secrets.contains("pw-router"));
    let db = std::fs::read(dir.join("data").join("telemon.db")).unwrap();
    assert!(!db.windows(9).any(|w| w == b"pw-router") && !db.windows(10).any(|w| w == b"rahasia123"), "rahasia tidak boleh polos di basis data");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(std::fs::metadata(dir.join("secrets.json")).unwrap().permissions().mode() & 0o777, 0o600);
    }

    let d = Desktop::start(cfg(&dir, 0)).await.unwrap();
    let routers = d.telemon.with_store(|s| s.list_routers()).unwrap();
    assert_eq!(routers.len(), 1);
    assert_eq!(d.telemon.with_store(|s| s.user_password(&format!("{}:budi", r.id))).unwrap().as_deref(), Some("rahasia123"), "kunci sama setelah restart");
    // Pemantau router tersimpan dijalankan otomatis: router tak terjangkau menjadi offline dengan pesan.
    let mut offline = false;
    for _ in 0..100 {
        if d.telemon.with_store(|s| s.get_router(&r.id)).unwrap().unwrap().status == telemon_core::RouterStatus::Offline {
            offline = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    assert!(offline, "pemantau harus berjalan otomatis setelah peluncuran");
    d.shutdown().await;
}

#[tokio::test]
async fn kunci_hilang_tidak_diganti_diam_diam() {
    let dir = tmp("lostkey");
    Desktop::start(cfg(&dir, 0)).await.unwrap().shutdown().await;
    // Berkas rahasia hilang tetapi basis data masih ada: harus gagal jelas, bukan membuat kunci baru.
    std::fs::remove_file(dir.join("secrets.json")).unwrap();
    match Desktop::start(cfg(&dir, 0)).await {
        Err(StartError::Secret(m)) => assert!(m.contains("kunci tidak cocok"), "{m}"),
        Err(e) => panic!("jenis galat salah: {e}"),
        Ok(_) => panic!("seharusnya gagal"),
    }
}

/// Penyimpanan rahasia yang gagal membaca sesaat (mis. keyring terkunci) tetapi menyimpan benar.
struct Flaky {
    inner: FileSecrets,
    blind: std::sync::atomic::AtomicBool,
}

impl SecretStore for Flaky {
    fn get(&self, key: &str) -> Option<String> {
        if self.blind.load(std::sync::atomic::Ordering::SeqCst) { None } else { self.inner.get(key) }
    }
    fn set(&self, key: &str, value: &str) -> Result<(), String> {
        self.inner.set(key, value)
    }
    fn delete(&self, key: &str) {
        self.inner.delete(key)
    }
}

#[tokio::test]
async fn kegagalan_baca_sesaat_tidak_menimpa_kunci_lama() {
    let dir = tmp("flaky");
    let mk = |blind: bool| DesktopConfig {
        db_path: dir.join("data").join("telemon.db"),
        static_dir: None,
        secrets: Arc::new(Flaky { inner: FileSecrets::open(dir.join("secrets.json")).unwrap(), blind: std::sync::atomic::AtomicBool::new(blind) }),
        port: 0,
    };
    Desktop::start(mk(false)).await.unwrap().shutdown().await;
    let before = std::fs::read_to_string(dir.join("secrets.json")).unwrap();
    // Keyring "terkunci": get gagal. Peluncuran ditolak dan kunci di penyimpanan tetap utuh.
    assert!(matches!(Desktop::start(mk(true)).await, Err(StartError::Secret(_))));
    assert_eq!(std::fs::read_to_string(dir.join("secrets.json")).unwrap(), before, "kunci lama tidak boleh tertimpa");
    // Setelah pulih, semuanya normal dengan kunci yang sama.
    Desktop::start(mk(false)).await.unwrap().shutdown().await;
}

#[tokio::test]
async fn port_pilihan_terpakai_beralih_ke_acak() {
    let dir = tmp("port");
    let blocker = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let busy = blocker.local_addr().unwrap().port();
    let d = Desktop::start(cfg(&dir, busy)).await.unwrap();
    assert_ne!(d.addr.port(), busy);
    d.shutdown().await;
}

#[test]
fn file_secrets_dasar() {
    let dir = tmp("fs");
    let p = dir.join("s.json");
    let s = FileSecrets::open(p.clone()).unwrap();
    assert_eq!(s.get("a"), None);
    s.set("a", "1").unwrap();
    s.set("b", "2").unwrap();
    s.delete("a");
    let again = FileSecrets::open(p.clone()).unwrap();
    assert_eq!((again.get("a"), again.get("b").as_deref()), (None, Some("2")));
    assert!(!p.with_extension("tmp").exists(), "tidak ada berkas sementara tersisa");
    std::fs::write(&p, "{rusak").unwrap();
    assert!(FileSecrets::open(p).is_err(), "berkas rusak tidak boleh diam-diam dianggap kosong");
}
