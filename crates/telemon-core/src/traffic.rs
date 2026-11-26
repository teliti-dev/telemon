//! Bandwidth per antarmuka dari counter `/interface` (ADR 0026).
//!
//! Router memberi counter kumulatif `rx-byte` dan `tx-byte`; laju dihitung dari selisih dua pembacaan.
//! Antarmuka PPPoE dinamis (`pppoe-in`, satu per sesi pelanggan) tidak dicatat satu per satu: lajunya dijumlah
//! menjadi antarmuka virtual [`PPPOE_AGGREGATE`] bersama jumlah sesi, sehingga rata-rata per user
//! tinggal membagi. Arah mengikuti sudut pandang router: untuk PPPoE, `tx` = unduhan pelanggan, `rx` = unggahan.

use std::collections::HashMap;
use std::time::Duration;

use crate::model::Row;

/// Nama antarmuka virtual: jumlah semua sesi PPPoE satu router.
pub const PPPOE_AGGREGATE: &str = "@pppoe";
const PPPOE_KIND: &str = "pppoe-in";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrafficConfig {
    pub every: Duration,
    pub retention: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IfaceInfo {
    pub name: String,
    pub kind: String,
    pub running: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sample {
    pub iface: String,
    /// bit per detik.
    pub rx_bps: i64,
    pub tx_bps: i64,
    /// Hanya untuk antarmuka virtual PPPoE: jumlah sesi saat pembacaan.
    pub sessions: Option<i64>,
}

#[derive(Debug, Default)]
pub struct Collected {
    pub ifaces: Vec<IfaceInfo>,
    pub samples: Vec<Sample>,
}

/// Menyimpan counter pembacaan sebelumnya untuk satu router.
#[derive(Debug, Default)]
pub struct Collector {
    prev: HashMap<String, (u64, u64)>,
    prev_at: Option<i64>,
}

fn flag(row: &Row, key: &str) -> bool {
    row.get(key).is_some_and(|v| v == "true" || v == "yes")
}

fn rate(delta: u64, dt_ms: i64) -> i64 {
    i64::try_from(u128::from(delta) * 8000 / dt_ms as u128).unwrap_or(i64::MAX)
}

impl Collector {
    pub fn new() -> Self {
        Self::default()
    }

    /// Olah hasil `/interface/print`. Pembacaan pertama hanya menyimpan counter (belum ada laju).
    /// Counter yang turun (router restart, antarmuka dibuat ulang) dilewati, bukan dicatat negatif.
    pub fn collect(&mut self, rows: &[Row], now_ms: i64) -> Collected {
        let dt = self.prev_at.map(|p| now_ms - p).filter(|d| *d > 0);
        let mut out = Collected::default();
        let mut next: HashMap<String, (u64, u64)> = HashMap::new();
        let (mut sum_rx, mut sum_tx, mut sessions, mut seen_pppoe) = (0u64, 0u64, 0i64, false);

        for r in rows {
            let name = r.get("name").map(String::as_str).unwrap_or("");
            if name.is_empty() {
                continue;
            }
            let kind = r.get("type").cloned().unwrap_or_default();
            let running = flag(r, "running");
            let counters = r.get("rx-byte").and_then(|v| v.parse::<u64>().ok()).zip(r.get("tx-byte").and_then(|v| v.parse::<u64>().ok()));
            let delta = match (counters, self.prev.get(name), dt) {
                (Some((rx, tx)), Some(&(prx, ptx)), Some(_)) if rx >= prx && tx >= ptx => Some((rx - prx, tx - ptx)),
                _ => None,
            };
            if let Some(c) = counters {
                next.insert(name.to_string(), c);
            }

            if kind == PPPOE_KIND {
                seen_pppoe = true;
                if running {
                    sessions += 1;
                }
                if let Some((drx, dtx)) = delta {
                    sum_rx += drx;
                    sum_tx += dtx;
                }
                continue;
            }
            out.ifaces.push(IfaceInfo { name: name.to_string(), kind, running });
            if flag(r, "disabled") || !running {
                continue;
            }
            if let (Some((drx, dtx)), Some(dt)) = (delta, dt) {
                out.samples.push(Sample { iface: name.to_string(), rx_bps: rate(drx, dt), tx_bps: rate(dtx, dt), sessions: None });
            }
        }

        if seen_pppoe {
            out.ifaces.push(IfaceInfo { name: PPPOE_AGGREGATE.into(), kind: "pppoe-aggregate".into(), running: sessions > 0 });
            if let Some(dt) = dt {
                out.samples.push(Sample { iface: PPPOE_AGGREGATE.into(), rx_bps: rate(sum_rx, dt), tx_bps: rate(sum_tx, dt), sessions: Some(sessions) });
            }
        }
        self.prev = next;
        self.prev_at = Some(now_ms);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(name: &str, kind: &str, running: bool, rx: u64, tx: u64) -> Row {
        [("name", name.to_string()), ("type", kind.to_string()), ("running", running.to_string()), ("disabled", "false".into()), ("rx-byte", rx.to_string()), ("tx-byte", tx.to_string())]
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect()
    }
    fn sample<'a>(c: &'a Collected, name: &str) -> Option<&'a Sample> {
        c.samples.iter().find(|s| s.iface == name)
    }

    #[test]
    fn pembacaan_pertama_hanya_daftar_antarmuka_lalu_laju_dari_selisih() {
        let mut c = Collector::new();
        let first = c.collect(&[row("ether1", "ether", true, 1_000, 2_000)], 0);
        assert!(first.samples.is_empty());
        assert_eq!(first.ifaces, vec![IfaceInfo { name: "ether1".into(), kind: "ether".into(), running: true }]);
        // 125.000 byte dalam 10 detik = 100 kbit/s; 250.000 byte = 200 kbit/s.
        let second = c.collect(&[row("ether1", "ether", true, 126_000, 252_000)], 10_000);
        let s = sample(&second, "ether1").unwrap();
        assert_eq!((s.rx_bps, s.tx_bps, s.sessions), (100_000, 200_000, None));
    }

    #[test]
    fn counter_turun_dan_antarmuka_mati_tidak_dicatat() {
        let mut c = Collector::new();
        c.collect(&[row("ether1", "ether", true, 5_000_000, 5_000_000), row("ether2", "ether", true, 10, 10)], 0);
        // ether1 restart (counter turun): dilewati. ether2 turun (tidak running): tidak dicatat tetapi tetap terdaftar.
        let s = c.collect(&[row("ether1", "ether", true, 100, 100), row("ether2", "ether", false, 20, 20)], 10_000);
        assert!(s.samples.is_empty(), "{:?}", s.samples);
        assert_eq!(s.ifaces.len(), 2);
        assert!(!s.ifaces.iter().find(|i| i.name == "ether2").unwrap().running);
        // Pembacaan berikutnya normal lagi dari counter baru.
        let n = c.collect(&[row("ether1", "ether", true, 125_100, 100)], 20_000);
        assert_eq!(sample(&n, "ether1").unwrap().rx_bps, 100_000);
    }

    #[test]
    fn pppoe_dijumlah_menjadi_satu_antarmuka_dengan_jumlah_sesi() {
        let mut c = Collector::new();
        c.collect(&[row("<pppoe-a>", "pppoe-in", true, 0, 0), row("<pppoe-b>", "pppoe-in", true, 0, 0), row("ether1", "ether", true, 0, 0)], 0);
        // Sesi baru <pppoe-c> muncul: dihitung sebagai sesi, lajunya baru ada di pembacaan berikutnya.
        let s = c.collect(
            &[row("<pppoe-a>", "pppoe-in", true, 125_000, 1_250_000), row("<pppoe-b>", "pppoe-in", true, 125_000, 1_250_000), row("<pppoe-c>", "pppoe-in", true, 999, 999), row("ether1", "ether", true, 0, 0)],
            10_000,
        );
        let a = sample(&s, PPPOE_AGGREGATE).unwrap();
        assert_eq!((a.rx_bps, a.tx_bps, a.sessions), (200_000, 2_000_000, Some(3)));
        assert!(sample(&s, "<pppoe-a>").is_none(), "antarmuka sesi tidak dicatat sendiri");
        let names: Vec<&str> = s.ifaces.iter().map(|i| i.name.as_str()).collect();
        assert_eq!(names, vec!["ether1", PPPOE_AGGREGATE]);
        // Semua sesi selesai: jumlah sesi 0 dan laju 0, bukan sampel hilang.
        let none = c.collect(&[row("ether1", "ether", true, 0, 0), row("<pppoe-x>", "pppoe-in", false, 0, 0)], 20_000);
        assert_eq!(sample(&none, PPPOE_AGGREGATE).map(|s| (s.rx_bps, s.tx_bps, s.sessions)), Some((0, 0, Some(0))));
    }

    #[test]
    fn baris_rusak_dan_jam_mundur_aman() {
        let mut c = Collector::new();
        let bad: Row = [("name".to_string(), "x".to_string())].into_iter().collect();
        c.collect(&[bad.clone(), row("ether1", "ether", true, 1, 1)], 5_000);
        // Jam sama atau mundur: tidak ada laju (hindari bagi nol atau nilai negatif).
        assert!(c.collect(&[row("ether1", "ether", true, 9, 9)], 5_000).samples.is_empty());
        assert!(c.collect(&[row("ether1", "ether", true, 99, 99)], 1_000).samples.is_empty());
    }
}
