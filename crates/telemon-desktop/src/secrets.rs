//! Penyimpanan rahasia untuk aplikasi desktop.
//!
//! - [`FileSecrets`]: berkas JSON dengan izin 0600 (Unix). Dipakai bila keyring OS tidak tersedia
//!   (mis. Linux tanpa layanan secret-service) dan di uji. **Lebih lemah dari keyring**: siapa pun yang bisa
//!   membaca berkas ini membaca kunci basis data; aplikasi memberi tahu pengguna saat memakainya.
//! - Implementasi keyring OS ada di `src-tauri` (bergantung pada pustaka sistem).

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;

use telemon_core::SecretStore;

pub struct FileSecrets {
    path: PathBuf,
    cache: Mutex<BTreeMap<String, String>>,
}

impl FileSecrets {
    pub fn open(path: PathBuf) -> Result<Self, String> {
        let cache = match std::fs::read_to_string(&path) {
            Ok(t) => serde_json::from_str(&t).map_err(|e| format!("{} rusak: {e}", path.display()))?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => BTreeMap::new(),
            Err(e) => return Err(format!("membaca {}: {e}", path.display())),
        };
        Ok(Self { path, cache: Mutex::new(cache) })
    }

    fn flush(&self, map: &BTreeMap<String, String>) -> Result<(), String> {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        // Tulis ke berkas sementara (izin dibatasi sejak dibuat) lalu ganti: tidak pernah ada berkas setengah tertulis.
        let tmp = self.path.with_extension("tmp");
        let data = serde_json::to_vec_pretty(map).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::io::Write;
            use std::os::unix::fs::OpenOptionsExt;
            let mut f = std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(&tmp).map_err(|e| e.to_string())?;
            f.write_all(&data).map_err(|e| e.to_string())?;
            f.sync_all().map_err(|e| e.to_string())?;
        }
        #[cfg(not(unix))]
        std::fs::write(&tmp, &data).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, &self.path).map_err(|e| e.to_string())
    }
}

impl SecretStore for FileSecrets {
    fn get(&self, key: &str) -> Option<String> {
        self.cache.lock().expect("lock").get(key).cloned()
    }
    fn set(&self, key: &str, value: &str) -> Result<(), String> {
        let mut m = self.cache.lock().expect("lock");
        let mut next = m.clone();
        next.insert(key.into(), value.into());
        self.flush(&next)?;
        *m = next;
        Ok(())
    }
    fn delete(&self, key: &str) {
        let mut m = self.cache.lock().expect("lock");
        let mut next = m.clone();
        if next.remove(key).is_some() && self.flush(&next).is_ok() {
            *m = next;
        }
    }
}
