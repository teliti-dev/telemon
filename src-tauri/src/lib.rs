//! Aplikasi desktop Telemon (ADR 0021). Seluruh logika ada di `telemon-desktop` (teruji tanpa Tauri); di sini
//! hanya: pilih penyimpanan rahasia, luncurkan, buka webview ke server loopback, dan matikan dengan rapi.

mod secrets;
mod updater;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use tauri::{Manager, RunEvent, WebviewUrl, WebviewWindowBuilder};

/// Batas ukuran jendela (piksel logis): tidak lebih kecil dari ini supaya susunan desktop tidak rusak, dan tidak lebih besar
/// dari ini supaya isi tidak merentang berlebihan di layar sangat lebar.
const MIN_WINDOW: (f64, f64) = (960.0, 620.0);
const MAX_WINDOW: (f64, f64) = (2560.0, 1600.0);
use telemon_desktop::{Desktop, DesktopConfig, TunnelChoice, PREFERRED_PORT};

struct Running(Mutex<Option<Desktop>>);

/// Halaman galat sederhana (data URL) bila peluncuran gagal, supaya pengguna tidak melihat jendela kosong
/// atau aplikasi yang diam-diam menutup (Windows rilis tidak punya konsol).
fn error_url(msg: &str) -> WebviewUrl {
  let esc = msg.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
  let html = format!(
    "<meta charset=utf-8><body style=\"font-family:system-ui;margin:2rem;color:#222\"><h2>Telemon gagal dimulai</h2><p style=\"white-space:pre-wrap\">{esc}</p>"
  );
  let encoded: String = html.bytes().map(|b| format!("%{b:02X}")).collect();
  WebviewUrl::External(format!("data:text/html;charset=utf-8,{encoded}").parse().expect("data url valid"))
}

fn launch(app: &tauri::AppHandle) -> Result<WebviewUrl, String> {
  let data_dir: PathBuf = app.path().app_data_dir().map_err(|e| e.to_string())?;
  std::fs::create_dir_all(&data_dir).map_err(|e| format!("membuat {}: {e}", data_dir.display()))?;
  let (secrets, warning) = secrets::choose(&data_dir)?;
  if let Some(w) = warning {
    log::warn!("{w}");
  }
  // Rilis: UI disalin ke resource (`ui/`) dan dilayani server loopback. Dev (`tauri dev`): UI dari Vite, yang
  // meneruskan /api dan /desktop-entry ke server ini (vite.config.ts).
  let dev = cfg!(debug_assertions);
  let static_dir = if dev { None } else { Some(app.path().resource_dir().map_err(|e| e.to_string())?.join("ui")) };
  let cfg = DesktopConfig { db_path: data_dir.join("telemon.db"), static_dir, secrets, port: PREFERRED_PORT, tunnel: TunnelChoice::Official };
  let desktop = tauri::async_runtime::block_on(Desktop::start(cfg)).map_err(|e| e.to_string())?;
  let url = if dev {
    format!("http://localhost:5173/desktop-entry?t={}", desktop.token)
  } else {
    desktop.entry_url()
  };
  // Pembaruan: penyedia dipasang ke server, pemeriksaan otomatis dimulai (sekali setelah dibuka, lalu tiap 24 jam).
  desktop.state.set_updater(Arc::new(updater::TauriUpdater::new(app.clone())));
  let state = desktop.state.clone();
  tauri::async_runtime::block_on(async move { state.start_update_checks() });
  app.manage(Running(Mutex::new(Some(desktop))));
  Ok(WebviewUrl::External(url.parse().map_err(|e: url::ParseError| e.to_string())?))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  // Klien HTTP updater memakai rustls tanpa penyedia bawaan: pasang ring (diabaikan bila sudah terpasang).
  let _ = rustls::crypto::ring::default_provider().install_default();
  let app = tauri::Builder::default()
    .setup(|app| {
      if cfg!(debug_assertions) {
        app.handle().plugin(tauri_plugin_log::Builder::default().level(log::LevelFilter::Info).build())?;
      }
      app.handle().plugin(tauri_plugin_updater::Builder::new().build())?;
      let url = launch(app.handle()).unwrap_or_else(|e| {
        log::error!("peluncuran gagal: {e}");
        error_url(&e)
      });
      let builder = WebviewWindowBuilder::new(app, "main", url)
        .title("Telemon")
        .inner_size(1280.0, 800.0)
        .min_inner_size(MIN_WINDOW.0, MIN_WINDOW.1)
        .max_inner_size(MAX_WINDOW.0, MAX_WINDOW.1);
      // macOS: bilah judul menjadi lapisan transparan (Overlay) tanpa teks judul; header UI yang menjadi bilah judulnya
      // dan tombol lalu lintas bawaan mengambang di kiri atas (lihat `useMacOverlay` di src/lib/window.ts).
      #[cfg(target_os = "macos")]
      let builder = builder.title_bar_style(tauri::TitleBarStyle::Overlay).hidden_title(true).traffic_light_position(tauri::LogicalPosition::new(18.0, 12.0));
      builder.build()?;
      Ok(())
    })
    .build(tauri::generate_context!())
    .expect("error while building tauri application");

  app.run(|handle, event| {
    // Tutup dengan rapi: hentikan berbagi, pemantau, dan server supaya basis data tertulis utuh.
    if let RunEvent::Exit = event
      && let Some(r) = handle.try_state::<Running>()
      && let Some(d) = r.0.lock().ok().and_then(|mut g| g.take())
    {
      tauri::async_runtime::block_on(d.shutdown());
    }
  });
}
