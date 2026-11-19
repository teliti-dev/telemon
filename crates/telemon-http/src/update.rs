//! Pembaruan aplikasi desktop (ADR 0023). Pemeriksaan dan pemasangan dikerjakan penyedia (`Updater`, di aplikasi
//! Tauri) dan dipanggil lewat API ini, bukan lewat IPC webview: webview memuat UI dari origin loopback, dan
//! membuka IPC Tauri untuk origin itu akan membiarkan halaman apa pun di origin tersebut memanggil perintah aplikasi.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};
use telemon_core::Connector;
use tokio::task::JoinHandle;

use crate::error::ApiError;
use crate::state::AppState;

type BoxFut<'a, T> = std::pin::Pin<Box<dyn std::future::Future<Output = T> + Send + 'a>>;

/// Banyak cadangan basis data (sebelum pasang update) yang disimpan.
pub const KEEP_BACKUPS: usize = 3;
/// Selang pemeriksaan otomatis dan tundaan pemeriksaan pertama setelah aplikasi dibuka.
const CHECK_EVERY: Duration = Duration::from_secs(24 * 3600);
const FIRST_CHECK_AFTER: Duration = Duration::from_secs(20);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdateInfo {
    pub version: String,
    pub notes: Option<String>,
    pub date: Option<String>,
}

/// Penyedia pembaruan. `channel`: `stable` (hanya rilis non-pra-rilis) atau `rc` (termasuk pra-rilis).
pub trait Updater: Send + Sync {
    fn current_version(&self) -> String;
    /// Aplikasi ini bisa memasang sendiri? (Linux hanya untuk AppImage; paket `.deb` harus dipasang manual.)
    fn can_install(&self) -> bool {
        true
    }
    fn check(&self, channel: &str) -> BoxFut<'_, Result<Option<UpdateInfo>, String>>;
    /// Unduh, verifikasi tanda tangan, dan pasang pembaruan yang ditemukan pemeriksaan terakhir. Aplikasi
    /// dapat menutup atau memulai ulang sendiri setelahnya.
    fn install(&self) -> BoxFut<'_, Result<(), String>>;
}

#[derive(Default)]
pub struct UpdateState {
    updater: Mutex<Option<Arc<dyn Updater>>>,
    status: Mutex<Status>,
    task: Mutex<Option<JoinHandle<()>>>,
    /// Hanya satu pemasangan pada satu waktu.
    installing: tokio::sync::Mutex<()>,
}

#[derive(Default, Clone)]
struct Status {
    checked_at: Option<i64>,
    available: Option<UpdateInfo>,
    error: Option<String>,
}

fn safe_name(s: &str) -> String {
    s.chars().map(|c| if c.is_ascii_alphanumeric() || matches!(c, '.' | '-') { c } else { '_' }).collect()
}

/// Hapus cadangan lama, sisakan `keep` terbaru (urut nama = urut waktu).
pub fn prune_backups(dir: &Path, keep: usize) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut files: Vec<PathBuf> = rd.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "db") && p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("telemon-"))).collect();
    files.sort();
    if files.len() > keep {
        for old in &files[..files.len() - keep] {
            let _ = std::fs::remove_file(old);
        }
    }
}

impl<C: Connector> AppState<C> {
    /// Pasang penyedia pembaruan (aplikasi desktop). Tanpa penyedia, `/api/update` melaporkan "tidak didukung".
    pub fn set_updater(&self, u: Arc<dyn Updater>) {
        *self.update.updater.lock().expect("lock") = Some(u);
    }

    fn updater(&self) -> Option<Arc<dyn Updater>> {
        self.update.updater.lock().expect("lock").clone()
    }

    pub fn update_channel(&self) -> String {
        match self.telemon.with_store(|s| s.kv_get("update.channel")) {
            Ok(Some(c)) if c == "rc" => "rc".into(),
            _ => "stable".into(),
        }
    }

    /// Periksa sekarang dan simpan hasilnya.
    pub async fn check_update(&self) -> Result<Option<UpdateInfo>, ApiError> {
        let u = self.updater().ok_or_else(not_supported)?;
        let channel = self.update_channel();
        let result = u.check(&channel).await;
        let mut st = self.update.status.lock().expect("lock");
        st.checked_at = Some(self.now());
        match &result {
            Ok(a) => {
                st.available = a.clone();
                st.error = None;
            }
            Err(e) => st.error = Some(e.clone()),
        }
        drop(st);
        result.map_err(|e| ApiError::new(axum::http::StatusCode::BAD_GATEWAY, "update_check_failed", format!("Gagal memeriksa pembaruan: {e}")))
    }

    /// Pemeriksaan otomatis: sekali setelah aplikasi dibuka, lalu tiap 24 jam. Kegagalan (mis. tanpa internet)
    /// hanya dicatat di status, tidak mengganggu.
    pub fn start_update_checks(self: &Arc<Self>) {
        if self.updater().is_none() {
            return;
        }
        let st = self.clone();
        let task = tokio::spawn(async move {
            tokio::time::sleep(FIRST_CHECK_AFTER).await;
            loop {
                let _ = st.check_update().await;
                tokio::time::sleep(CHECK_EVERY).await;
            }
        });
        if let Some(old) = self.update.task.lock().expect("lock").replace(task) {
            old.abort();
        }
    }

    pub fn stop_update_checks(&self) {
        if let Some(t) = self.update.task.lock().expect("lock").take() {
            t.abort();
        }
    }

    pub fn update_json(&self) -> Value {
        let st = self.update.status.lock().expect("lock").clone();
        let u = self.updater();
        json!({
            "supported": u.is_some(),
            "currentVersion": u.as_ref().map(|u| u.current_version()),
            "canInstall": u.as_ref().is_some_and(|u| u.can_install()),
            "channel": self.update_channel(),
            "checkedAt": st.checked_at,
            "available": st.available.map(|a| json!({ "version": a.version, "notes": a.notes, "date": a.date })),
            "error": st.error,
        })
    }

    pub fn set_update_channel(&self, channel: &str) -> Result<(), ApiError> {
        if !matches!(channel, "stable" | "rc") {
            return Err(ApiError::bad("Kanal harus stable atau rc"));
        }
        self.telemon.with_store(|s| s.kv_set("update.channel", channel)).map_err(|e| ApiError::internal(e))?;
        // Hasil pemeriksaan kanal lama tidak berlaku untuk kanal baru.
        *self.update.status.lock().expect("lock") = Status::default();
        Ok(())
    }

    /// Cadangkan basis data lalu pasang pembaruan. Tanpa cadangan yang berhasil, tidak ada yang dipasang.
    pub async fn install_update(&self) -> Result<Value, ApiError> {
        let u = self.updater().ok_or_else(not_supported)?;
        if !u.can_install() {
            return Err(ApiError::new(axum::http::StatusCode::CONFLICT, "manual_install", "Pembaruan otomatis hanya untuk AppImage dan Windows; unduh versi baru dari halaman rilis"));
        }
        let _guard = self.update.installing.try_lock().map_err(|_| ApiError::new(axum::http::StatusCode::CONFLICT, "update_busy", "Pembaruan sedang dipasang"))?;
        let info = self.update.status.lock().expect("lock").available.clone().ok_or_else(|| ApiError::new(axum::http::StatusCode::CONFLICT, "no_update", "Belum ada pembaruan; periksa dulu"))?;
        let dir = Path::new(&self.cfg.db_path).parent().map(|p| p.join("backups")).ok_or_else(|| ApiError::internal("folder data tidak diketahui"))?;
        std::fs::create_dir_all(&dir).map_err(|e| ApiError::internal(format!("membuat folder cadangan: {e}")))?;
        // Nama unik: pasang ulang setelah gagal pada waktu yang sama tidak boleh menimpa atau ditolak.
        let stem = format!("telemon-{:014}-sebelum-{}", self.now(), safe_name(&info.version));
        let mut dest = dir.join(format!("{stem}.db"));
        for n in 2..100 {
            if !dest.exists() {
                break;
            }
            dest = dir.join(format!("{stem}-{n}.db"));
        }
        self.telemon
            .with_store(|s| s.backup_to(&dest))
            .map_err(|e| ApiError::new(axum::http::StatusCode::INTERNAL_SERVER_ERROR, "backup_failed", format!("Gagal mencadangkan basis data; pembaruan dibatalkan: {e}")))?;
        prune_backups(&dir, KEEP_BACKUPS);
        u.install().await.map_err(|e| ApiError::new(axum::http::StatusCode::BAD_GATEWAY, "update_install_failed", format!("Gagal memasang pembaruan (data aman, cadangan di {}): {e}", dest.display())))?;
        Ok(json!({ "installed": info.version, "backup": dest.display().to_string() }))
    }
}

fn not_supported() -> ApiError {
    ApiError::new(axum::http::StatusCode::NOT_IMPLEMENTED, "not_implemented", "Pembaruan otomatis tidak tersedia di aplikasi ini")
}
