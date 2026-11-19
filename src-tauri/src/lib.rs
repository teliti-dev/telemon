//! Aplikasi desktop Telemon (ADR 0021). Seluruh logika ada di `telemon-desktop` (teruji tanpa Tauri); di sini
//! hanya: pilih penyimpanan rahasia, luncurkan, buka webview ke server loopback, dan matikan dengan rapi.

mod secrets;

use std::path::PathBuf;
use std::sync::Mutex;

use tauri::{Manager, RunEvent, WebviewUrl, WebviewWindowBuilder};
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
  app.manage(Running(Mutex::new(Some(desktop))));
  Ok(WebviewUrl::External(url.parse().map_err(|e: url::ParseError| e.to_string())?))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  let app = tauri::Builder::default()
    .setup(|app| {
      if cfg!(debug_assertions) {
        app.handle().plugin(tauri_plugin_log::Builder::default().level(log::LevelFilter::Info).build())?;
      }
      let url = launch(app.handle()).unwrap_or_else(|e| {
        log::error!("peluncuran gagal: {e}");
        error_url(&e)
      });
      WebviewWindowBuilder::new(app, "main", url).title("Telemon").inner_size(1280.0, 800.0).min_inner_size(360.0, 560.0).build()?;
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
