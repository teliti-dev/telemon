//! TLS untuk RouterOS api-ssl (port 8729) dengan rustls + ring (tanpa OpenSSL/aws-lc).
//!
//! Router biasanya memakai sertifikat self-signed, jadi rantai CA tidak berguna. Telemon memakai
//! **pin sidik jari**: sidik jari SHA-256 sertifikat router dicatat saat pertama kali (setelah
//! pengguna memeriksanya, [`fetch_fingerprint`]) dan setiap sambungan berikutnya menolak
//! sertifikat yang sidik jarinya berbeda. Tanda tangan handshake tetap diverifikasi.
//!
//! Catatan RouterOS: `api-ssl` harus diberi sertifikat (`/ip service set api-ssl certificate=...`).
//! Tanpa sertifikat router memakai anonymous Diffie-Hellman yang tidak didukung rustls.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{CryptoProvider, WebPkiSupportedAlgorithms, verify_tls12_signature, verify_tls13_signature};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{ClientConfig, DigitallySignedStruct, Error as TlsError, SignatureScheme};
use sha2::{Digest, Sha256};
use tokio::net::TcpStream;
use tokio::time::timeout;
use tokio_rustls::TlsConnector;

use crate::router::Error;

/// Sidik jari SHA-256 sertifikat (DER) router.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Fingerprint(pub [u8; 32]);

impl Fingerprint {
    pub fn of_cert(der: &[u8]) -> Self {
        let mut out = [0u8; 32];
        out.copy_from_slice(Sha256::digest(der).as_slice());
        Self(out)
    }

    /// `AA:BB:...` (huruf besar), seperti yang ditampilkan Winbox/OpenSSL.
    pub fn to_hex(&self) -> String {
        self.0.iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(":")
    }

    /// Menerima dengan atau tanpa `:`/spasi, huruf besar atau kecil.
    pub fn parse(s: &str) -> Option<Self> {
        let hex: String = s.chars().filter(|c| !matches!(c, ':' | ' ' | '-')).collect();
        if hex.len() != 64 || !hex.is_ascii() {
            return None;
        }
        let mut out = [0u8; 32];
        for (i, b) in out.iter_mut().enumerate() {
            *b = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).ok()?;
        }
        Some(Self(out))
    }
}

impl std::fmt::Debug for Fingerprint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Fingerprint({})", self.to_hex())
    }
}

fn provider() -> Arc<CryptoProvider> {
    Arc::new(rustls::crypto::ring::default_provider())
}

/// Menerima hanya sertifikat dengan sidik jari yang di-pin; tanda tangan tetap diperiksa.
#[derive(Debug)]
struct PinVerifier {
    pin: Fingerprint,
    algs: WebPkiSupportedAlgorithms,
}

impl ServerCertVerifier for PinVerifier {
    fn verify_server_cert(&self, end_entity: &CertificateDer<'_>, _inter: &[CertificateDer<'_>], _name: &ServerName<'_>, _ocsp: &[u8], _now: UnixTime) -> Result<ServerCertVerified, TlsError> {
        if Fingerprint::of_cert(end_entity) == self.pin {
            Ok(ServerCertVerified::assertion())
        } else {
            Err(TlsError::General("sidik jari sertifikat router tidak cocok dengan yang dipercaya".into()))
        }
    }

    fn verify_tls12_signature(&self, message: &[u8], cert: &CertificateDer<'_>, dss: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, TlsError> {
        verify_tls12_signature(message, cert, dss, &self.algs)
    }

    fn verify_tls13_signature(&self, message: &[u8], cert: &CertificateDer<'_>, dss: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, TlsError> {
        verify_tls13_signature(message, cert, dss, &self.algs)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.algs.supported_schemes()
    }
}

/// Hanya untuk [`fetch_fingerprint`]: menerima sertifikat apa pun dan mencatat sidik jarinya.
#[derive(Debug)]
struct CaptureVerifier {
    algs: WebPkiSupportedAlgorithms,
    seen: Mutex<Option<Fingerprint>>,
}

impl ServerCertVerifier for CaptureVerifier {
    fn verify_server_cert(&self, end_entity: &CertificateDer<'_>, _inter: &[CertificateDer<'_>], _name: &ServerName<'_>, _ocsp: &[u8], _now: UnixTime) -> Result<ServerCertVerified, TlsError> {
        *self.seen.lock().expect("lock") = Some(Fingerprint::of_cert(end_entity));
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(&self, message: &[u8], cert: &CertificateDer<'_>, dss: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, TlsError> {
        verify_tls12_signature(message, cert, dss, &self.algs)
    }

    fn verify_tls13_signature(&self, message: &[u8], cert: &CertificateDer<'_>, dss: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, TlsError> {
        verify_tls13_signature(message, cert, dss, &self.algs)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.algs.supported_schemes()
    }
}

fn config_with(verifier: Arc<dyn ServerCertVerifier>) -> Result<ClientConfig, Error> {
    Ok(ClientConfig::builder_with_provider(provider())
        .with_safe_default_protocol_versions()
        .map_err(|e| Error::Connect(e.to_string()))?
        .dangerous()
        .with_custom_certificate_verifier(verifier)
        .with_no_client_auth())
}

/// Konfigurasi klien yang hanya mempercayai sertifikat ber-sidik-jari `pin`.
pub fn pinned_config(pin: Fingerprint) -> Result<Arc<ClientConfig>, Error> {
    let algs = provider().signature_verification_algorithms;
    Ok(Arc::new(config_with(Arc::new(PinVerifier { pin, algs }))?))
}

/// Nama server untuk SNI; RouterOS tidak memeriksanya.
pub fn server_name() -> ServerName<'static> {
    ServerName::try_from("mikrotik").expect("nama valid")
}

/// Ambil sidik jari sertifikat router (percaya-saat-pertama): hanya menyelesaikan handshake TLS lalu
/// menutup, tanpa mengirim kredensial. Tampilkan hasilnya ke pengguna untuk dikonfirmasi sebelum
/// di-pin. Handshake harus berhasil (tanda tangan sah), bukan sekadar sertifikat terbaca.
pub async fn fetch_fingerprint(addr: &str, wait: Duration) -> Result<Fingerprint, Error> {
    let capture = Arc::new(CaptureVerifier { algs: provider().signature_verification_algorithms, seen: Mutex::new(None) });
    let connector = TlsConnector::from(Arc::new(config_with(capture.clone())?));
    let run = async {
        let tcp = TcpStream::connect(addr).await.map_err(|e| Error::Connect(e.to_string()))?;
        connector.connect(server_name(), tcp).await.map_err(|e| Error::Connect(format!("handshake TLS gagal: {e}")))?;
        Ok::<(), Error>(())
    };
    timeout(wait, run).await.map_err(|_| Error::Connect("waktu habis saat handshake TLS".into()))??;
    let seen = *capture.seen.lock().expect("lock");
    seen.ok_or_else(|| Error::Connect("router tidak mengirim sertifikat".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sidik_jari_hex_bolak_balik() {
        let f = Fingerprint::of_cert(b"contoh");
        let hex = f.to_hex();
        assert_eq!(hex.len(), 32 * 3 - 1);
        assert_eq!(Fingerprint::parse(&hex), Some(f));
        assert_eq!(Fingerprint::parse(&hex.to_lowercase().replace(':', "")), Some(f));
        assert_eq!(Fingerprint::parse(&format!("  {hex}  ")), None.or(Fingerprint::parse(hex.trim())));
        assert_eq!(Fingerprint::parse("AA:BB"), None);
        assert_eq!(Fingerprint::parse(&"ZZ".repeat(32)), None);
        assert_eq!(Fingerprint::parse(&"é".repeat(32)), None);
        assert_ne!(Fingerprint::of_cert(b"a"), Fingerprint::of_cert(b"b"));
    }
}
