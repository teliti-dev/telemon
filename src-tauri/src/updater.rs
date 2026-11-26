//! Pembaruan aplikasi (ADR 0023): `tauri-plugin-updater` dipanggil dari Rust dan dipaparkan lewat API server
//! (`/api/update`), bukan lewat IPC webview.

use std::future::Future;
use std::pin::Pin;

use tauri::{AppHandle, Wry};
use tauri_plugin_updater::{Update, UpdaterExt};
use telemon_http::{UpdateInfo, Updater};

/// Kanal stabil: rilis terbaru yang bukan pra-rilis (GitHub `latest` tidak pernah menunjuk pra-rilis).
const STABLE: &str = "https://github.com/teliti-dev/telemon/releases/latest/download/latest.json";
/// Kanal uji: rilis bergerak `channel-rc` yang selalu memuat manifest rilis terbaru (stabil maupun pra-rilis).
const RC: &str = "https://github.com/teliti-dev/telemon/releases/download/channel-rc/latest.json";

pub struct TauriUpdater {
  app: AppHandle<Wry>,
  found: tokio::sync::Mutex<Option<Update>>,
}

impl TauriUpdater {
  pub fn new(app: AppHandle<Wry>) -> Self {
    Self { app, found: tokio::sync::Mutex::new(None) }
  }
}

impl Updater for TauriUpdater {
  fn current_version(&self) -> String {
    self.app.package_info().version.to_string()
  }

  fn can_install(&self) -> bool {
    // Linux: pembaruan hanya untuk AppImage (variabel APPIMAGE diisi pemuatnya); paket .deb dipasang manual.
    !cfg!(target_os = "linux") || std::env::var_os("APPIMAGE").is_some()
  }

  fn check(&self, channel: &str) -> Pin<Box<dyn Future<Output = Result<Option<UpdateInfo>, String>> + Send + '_>> {
    let is_rc = channel == "rc";
    let url = if is_rc { RC } else { STABLE };
    Box::pin(async move {
      let url: url::Url = url.parse().map_err(|e: url::ParseError| e.to_string())?;
      let updater = self.app.updater_builder().endpoints(vec![url]).map_err(|e| e.to_string())?.build().map_err(|e| e.to_string())?;
      let found = match updater.check().await {
        Ok(f) => f,
        // GitHub `latest` tidak pernah menunjuk pra-rilis: selama belum ada rilis stabil, manifest kanal stabil
        // memang 404. Itu berarti "belum ada pembaruan stabil", bukan galat.
        Err(tauri_plugin_updater::Error::ReleaseNotFound) if !is_rc => None,
        Err(e) => return Err(e.to_string()),
      };
      let info = found.as_ref().map(|u| UpdateInfo { version: u.version.clone(), notes: u.body.clone(), date: u.date.map(|d| d.to_string()) });
      *self.found.lock().await = found;
      Ok(info)
    })
  }

  fn install(&self) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send + '_>> {
    Box::pin(async move {
      let guard = self.found.lock().await;
      let update = guard.as_ref().ok_or("belum ada pembaruan yang diperiksa")?;
      // Tanda tangan diverifikasi dengan kunci publik di tauri.conf.json sebelum apa pun dipasang.
      update.download_and_install(|_, _| {}, || {}).await.map_err(|e| e.to_string())?;
      drop(guard);
      // Windows: pemasang menutup aplikasi sendiri. Linux (AppImage): mulai ulang dari berkas yang baru.
      self.app.restart()
    })
  }
}
