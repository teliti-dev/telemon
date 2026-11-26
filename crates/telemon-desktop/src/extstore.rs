//! Pengunduh HTTPS untuk toko extension (ADR 0028). Hanya `https`, batas ukuran, dan waktu tunggu; paket yang
//! diunduh tetap diperiksa SHA-256 terhadap katalog bertanda tangan dan tanda tangan paketnya sendiri.

use std::io::Read;
use std::time::Duration;

use telemon_http::ExtFetch;

pub struct HttpsFetcher {
    agent: ureq::Agent,
}

impl Default for HttpsFetcher {
    fn default() -> Self {
        Self { agent: ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(120))).https_only(true).max_redirects(5).build().into() }
    }
}

/// Hanya `https://`: katalog dan paket tidak pernah lewat HTTP polos.
pub fn https_only(url: &str) -> Result<(), String> {
    if url.starts_with("https://") { Ok(()) } else { Err("Toko extension hanya memakai https".into()) }
}

impl ExtFetch for HttpsFetcher {
    fn get(&self, url: &str, max_bytes: usize) -> Result<Vec<u8>, String> {
        https_only(url)?;
        let mut resp = self.agent.get(url).call().map_err(|e| format!("mengunduh {url}: {e}"))?;
        let mut out = Vec::new();
        resp.body_mut().as_reader().take(max_bytes as u64 + 1).read_to_end(&mut out).map_err(|e| format!("membaca unduhan: {e}"))?;
        if out.len() > max_bytes {
            return Err(format!("unduhan lebih besar dari {max_bytes} byte; dibatalkan"));
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_https_is_allowed() {
        assert!(https_only("https://github.com/x").is_ok());
        for bad in ["http://github.com/x", "file:///etc/passwd", "ftp://x", "//x", "HTTPS://x.test"] {
            assert!(https_only(bad).is_err(), "{bad}");
        }
        let f = HttpsFetcher::default();
        assert!(f.get("http://127.0.0.1:9/x", 10).is_err());
    }
}
