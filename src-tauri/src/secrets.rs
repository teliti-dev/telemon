//! Penyimpanan rahasia aplikasi desktop: keyring OS bila tersedia dan lolos uji tulis-baca-hapus; bila tidak
//! (mis. Linux tanpa layanan Secret Service), berkas `secrets.json` berizin 0600 di folder data dengan peringatan.

use std::path::Path;
use std::sync::Arc;

use keyring::Entry;
use telemon_core::SecretStore;
use telemon_desktop::secrets::FileSecrets;

const SERVICE: &str = "dev.teliti.telemon";

struct Keyring;

impl SecretStore for Keyring {
    fn get(&self, key: &str) -> Option<String> {
        Entry::new(SERVICE, key).ok()?.get_password().ok()
    }
    fn set(&self, key: &str, value: &str) -> Result<(), String> {
        Entry::new(SERVICE, key).and_then(|e| e.set_password(value)).map_err(|e| e.to_string())
    }
    fn delete(&self, key: &str) {
        if let Ok(e) = Entry::new(SERVICE, key) {
            let _ = e.delete_credential();
        }
    }
}

fn keyring_works() -> Result<(), String> {
    if let Err(e) = Entry::store_status() {
        return Err(e.to_string());
    }
    // Uji nyata: sebagian layanan terpasang tetapi terkunci atau tanpa sesi grafis.
    let k = Keyring;
    k.set("probe", "ok")?;
    let read = k.get("probe");
    k.delete("probe");
    if read.as_deref() == Some("ok") { Ok(()) } else { Err("tulis-baca uji gagal".into()) }
}

/// Mengembalikan penyimpanan terpilih dan pesan peringatan bila memakai berkas.
pub fn choose(data_dir: &Path) -> Result<(Arc<dyn SecretStore>, Option<String>), String> {
    match keyring_works() {
        Ok(()) => Ok((Arc::new(Keyring), None)),
        Err(why) => {
            let f = FileSecrets::open(data_dir.join("secrets.json"))?;
            Ok((Arc::new(f), Some(format!("Keyring sistem tidak tersedia ({why}); kunci dan kata sandi router disimpan di berkas secrets.json (izin 0600) di folder data. Itu lebih lemah dari keyring."))))
        }
    }
}
