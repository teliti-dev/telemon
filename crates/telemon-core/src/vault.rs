//! Enkripsi nilai rahasia di basis data (ADR 0009): ChaCha20-Poly1305 (Rust murni, tanpa
//! pustaka C). Kunci 32 byte dipegang pemanggil (keyring OS di aplikasi), tidak pernah disimpan
//! bersama datanya. Tiap nilai diikat ke identitas barisnya lewat AAD sehingga blob yang
//! dipindahkan ke baris lain gagal dibuka.

use chacha20poly1305::aead::{Aead, Generate, Key, KeyInit, Payload};
use chacha20poly1305::{ChaCha20Poly1305, Nonce};

const NONCE_LEN: usize = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VaultError {
    /// Blob terlalu pendek atau bentuknya salah.
    Corrupt,
    /// Kunci salah, data diubah, atau dipindahkan ke baris lain.
    Decrypt,
    /// Pembangkit angka acak sistem gagal.
    Random,
}

impl std::fmt::Display for VaultError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            VaultError::Corrupt => "data terenkripsi rusak",
            VaultError::Decrypt => "kunci salah atau data diubah",
            VaultError::Random => "gagal membangkitkan angka acak",
        })
    }
}

impl std::error::Error for VaultError {}

/// Kunci acak baru 32 byte dari pembangkit acak sistem operasi.
pub fn generate_key() -> Result<[u8; 32], VaultError> {
    let key = Key::<ChaCha20Poly1305>::try_generate().map_err(|_| VaultError::Random)?;
    let mut out = [0u8; 32];
    out.copy_from_slice(key.as_slice());
    Ok(out)
}

pub struct Vault {
    cipher: ChaCha20Poly1305,
}

impl Vault {
    pub fn new(key: &[u8; 32]) -> Self {
        let k = Key::<ChaCha20Poly1305>::try_from(&key[..]).expect("kunci 32 byte");
        Self { cipher: ChaCha20Poly1305::new(&k) }
    }

    /// `nonce(12) || ciphertext+tag`. Nonce acak baru tiap pemanggilan.
    pub fn seal(&self, aad: &[u8], plain: &[u8]) -> Result<Vec<u8>, VaultError> {
        let nonce = Nonce::try_generate().map_err(|_| VaultError::Random)?;
        let ct = self.cipher.encrypt(&nonce, Payload { msg: plain, aad }).map_err(|_| VaultError::Decrypt)?;
        let mut out = Vec::with_capacity(NONCE_LEN + ct.len());
        out.extend_from_slice(nonce.as_slice());
        out.extend_from_slice(&ct);
        Ok(out)
    }

    pub fn open(&self, aad: &[u8], blob: &[u8]) -> Result<Vec<u8>, VaultError> {
        // 16 byte tag Poly1305 minimal.
        if blob.len() < NONCE_LEN + 16 {
            return Err(VaultError::Corrupt);
        }
        let nonce = Nonce::try_from(&blob[..NONCE_LEN]).map_err(|_| VaultError::Corrupt)?;
        self.cipher.decrypt(&nonce, Payload { msg: &blob[NONCE_LEN..], aad }).map_err(|_| VaultError::Decrypt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bolak_balik_dan_nonce_unik() {
        let v = Vault::new(&generate_key().unwrap());
        let a = v.seal(b"r1:budi", b"rahasia123").unwrap();
        let b = v.seal(b"r1:budi", b"rahasia123").unwrap();
        assert_ne!(a, b, "nonce harus berbeda");
        assert_eq!(v.open(b"r1:budi", &a).unwrap(), b"rahasia123");
        assert!(!a.windows(10).any(|w| w == b"rahasia123"), "plaintext tidak boleh tampak");
        assert_eq!(v.open(b"r1:budi", &v.seal(b"x", b"").unwrap()).unwrap_err(), VaultError::Decrypt, "AAD beda");
    }

    #[test]
    fn kunci_salah_aad_salah_dan_diubah_gagal() {
        let (k1, k2) = (generate_key().unwrap(), generate_key().unwrap());
        assert_ne!(k1, k2);
        let (v1, v2) = (Vault::new(&k1), Vault::new(&k2));
        let blob = v1.seal(b"id", b"nilai").unwrap();
        assert_eq!(v2.open(b"id", &blob).unwrap_err(), VaultError::Decrypt);
        assert_eq!(v1.open(b"id-lain", &blob).unwrap_err(), VaultError::Decrypt);
        let mut t = blob.clone();
        *t.last_mut().unwrap() ^= 1;
        assert_eq!(v1.open(b"id", &t).unwrap_err(), VaultError::Decrypt);
        let mut t = blob.clone();
        t[0] ^= 1;
        assert_eq!(v1.open(b"id", &t).unwrap_err(), VaultError::Decrypt);
        assert_eq!(v1.open(b"id", &blob[..10]).unwrap_err(), VaultError::Corrupt);
        assert_eq!(v1.open(b"id", &[]).unwrap_err(), VaultError::Corrupt);
    }
}
