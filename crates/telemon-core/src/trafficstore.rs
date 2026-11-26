//! Penyimpanan deret waktu bandwidth (ADR 0026): sampel laju per antarmuka dan daftar antarmuka yang pernah terlihat.

use rusqlite::params;

use crate::store::Store;
use crate::traffic::{IfaceInfo, Sample};

type Result<T> = rusqlite::Result<T>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IfaceRow {
    pub name: String,
    pub kind: String,
    pub running: bool,
    pub last_seen: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TrafficPoint {
    /// Awal ember waktu (epoch ms).
    pub at: i64,
    pub rx_bps: f64,
    pub tx_bps: f64,
    /// Rata-rata jumlah sesi PPPoE (hanya untuk antarmuka virtual).
    pub sessions: Option<f64>,
}

impl Store {
    /// Simpan satu putaran pembacaan: perbarui daftar antarmuka dan tambah sampel, dalam satu transaksi.
    pub fn insert_traffic(&mut self, router_id: &str, at: i64, ifaces: &[IfaceInfo], samples: &[Sample]) -> Result<()> {
        let tx = self.conn.transaction()?;
        {
            let mut up = tx.prepare(
                "INSERT INTO traffic_ifaces (router_id, name, kind, running, last_seen) VALUES (?1,?2,?3,?4,?5)
                 ON CONFLICT(router_id, name) DO UPDATE SET kind = ?3, running = ?4, last_seen = ?5",
            )?;
            for i in ifaces {
                up.execute(params![router_id, i.name, i.kind, i.running, at])?;
            }
            let mut ins = tx.prepare("INSERT INTO traffic_samples (router_id, iface, at, rx_bps, tx_bps, sessions) VALUES (?1,?2,?3,?4,?5,?6)")?;
            for s in samples {
                ins.execute(params![router_id, s.iface, at, s.rx_bps, s.tx_bps, s.sessions])?;
            }
        }
        tx.commit()
    }

    pub fn list_traffic_ifaces(&self, router_id: &str) -> Result<Vec<IfaceRow>> {
        let mut st = self.conn.prepare("SELECT name, kind, running, last_seen FROM traffic_ifaces WHERE router_id = ?1 ORDER BY name COLLATE NOCASE")?;
        let rows = st.query_map([router_id], |r| Ok(IfaceRow { name: r.get(0)?, kind: r.get(1)?, running: r.get::<_, i64>(2)? != 0, last_seen: r.get(3)? }))?;
        rows.collect()
    }

    /// Rata-rata per ember `bucket_ms`. `router_id = None` menjumlahkan semua router (rata-rata per router dulu,
    /// lalu dijumlah), hanya bermakna untuk antarmuka virtual PPPoE (total pelanggan; antarmuka fisik akan terhitung ganda).
    pub fn traffic_series(&self, router_id: Option<&str>, iface: &str, from: i64, to: i64, bucket_ms: i64) -> Result<Vec<TrafficPoint>> {
        let bucket_ms = bucket_ms.max(1);
        let map = |r: &rusqlite::Row<'_>| Ok(TrafficPoint { at: r.get(0)?, rx_bps: r.get(1)?, tx_bps: r.get(2)?, sessions: r.get(3)? });
        match router_id {
            Some(id) => {
                let mut st = self.conn.prepare(
                    "SELECT (at / ?1) * ?1 AS b, AVG(rx_bps), AVG(tx_bps), AVG(sessions) FROM traffic_samples
                     WHERE router_id = ?2 AND iface = ?3 AND at >= ?4 AND at <= ?5 GROUP BY b ORDER BY b",
                )?;
                st.query_map(params![bucket_ms, id, iface, from, to], map)?.collect()
            }
            None => {
                let mut st = self.conn.prepare(
                    "SELECT b, SUM(rx), SUM(tx), SUM(sess) FROM (
                       SELECT router_id, (at / ?1) * ?1 AS b, AVG(rx_bps) AS rx, AVG(tx_bps) AS tx, AVG(sessions) AS sess FROM traffic_samples
                       WHERE iface = ?2 AND at >= ?3 AND at <= ?4 GROUP BY router_id, b
                     ) GROUP BY b ORDER BY b",
                )?;
                st.query_map(params![bucket_ms, iface, from, to], map)?.collect()
            }
        }
    }

    /// Hapus sampel lebih lama dari `before_ms` dan antarmuka yang tidak terlihat sejak itu.
    pub fn purge_traffic(&mut self, before_ms: i64) -> Result<u64> {
        let tx = self.conn.transaction()?;
        let n = tx.execute("DELETE FROM traffic_samples WHERE at < ?1", [before_ms])? as u64;
        tx.execute("DELETE FROM traffic_ifaces WHERE last_seen < ?1", [before_ms])?;
        tx.commit()?;
        Ok(n)
    }

    pub fn delete_traffic_of_router(&mut self, router_id: &str) -> Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM traffic_samples WHERE router_id = ?1", [router_id])?;
        tx.execute("DELETE FROM traffic_ifaces WHERE router_id = ?1", [router_id])?;
        tx.commit()
    }

    pub fn count_traffic_samples(&self) -> Result<u64> {
        self.conn.query_row("SELECT COUNT(*) FROM traffic_samples", [], |r| r.get::<_, i64>(0)).map(|n| n as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(iface: &str, rx: i64, tx: i64, sessions: Option<i64>) -> Sample {
        Sample { iface: iface.into(), rx_bps: rx, tx_bps: tx, sessions }
    }
    fn info(name: &str, running: bool) -> IfaceInfo {
        IfaceInfo { name: name.into(), kind: "ether".into(), running }
    }

    #[test]
    fn simpan_daftar_antarmuka_dan_ember_rata_rata() {
        let mut st = Store::open_in_memory().unwrap();
        st.insert_traffic("r1", 1_000, &[info("ether1", true)], &[s("ether1", 100, 200, None)]).unwrap();
        st.insert_traffic("r1", 2_000, &[info("ether1", false), info("ether2", true)], &[s("ether1", 300, 400, None)]).unwrap();
        let l = st.list_traffic_ifaces("r1").unwrap();
        assert_eq!(l.iter().map(|i| (i.name.as_str(), i.running, i.last_seen)).collect::<Vec<_>>(), vec![("ether1", false, 2_000), ("ether2", true, 2_000)]);
        assert!(st.list_traffic_ifaces("r2").unwrap().is_empty());
        // Satu ember 10 detik: rata-rata dua sampel.
        let p = st.traffic_series(Some("r1"), "ether1", 0, 10_000, 10_000).unwrap();
        assert_eq!(p, vec![TrafficPoint { at: 0, rx_bps: 200.0, tx_bps: 300.0, sessions: None }]);
        // Ember 1 detik: dua titik terpisah; rentang membatasi.
        assert_eq!(st.traffic_series(Some("r1"), "ether1", 0, 10_000, 1_000).unwrap().len(), 2);
        assert_eq!(st.traffic_series(Some("r1"), "ether1", 1_500, 10_000, 1_000).unwrap().len(), 1);
        assert!(st.traffic_series(Some("r1"), "tidak-ada", 0, 10_000, 1_000).unwrap().is_empty());
    }

    #[test]
    fn total_semua_router_menjumlah_rata_rata_per_router() {
        let mut st = Store::open_in_memory().unwrap();
        // r1 punya dua sampel dalam ember (rata-rata 200/100, 10 sesi); r2 satu sampel (50/50, 4 sesi).
        st.insert_traffic("r1", 1_000, &[], &[s("@pppoe", 100, 50, Some(8))]).unwrap();
        st.insert_traffic("r1", 2_000, &[], &[s("@pppoe", 300, 150, Some(12))]).unwrap();
        st.insert_traffic("r2", 1_500, &[], &[s("@pppoe", 50, 50, Some(4))]).unwrap();
        let p = st.traffic_series(None, "@pppoe", 0, 10_000, 10_000).unwrap();
        assert_eq!(p, vec![TrafficPoint { at: 0, rx_bps: 250.0, tx_bps: 150.0, sessions: Some(14.0) }]);
    }

    #[test]
    fn purge_dan_hapus_router() {
        let mut st = Store::open_in_memory().unwrap();
        st.insert_traffic("r1", 1_000, &[info("a", true)], &[s("a", 1, 1, None)]).unwrap();
        st.insert_traffic("r1", 9_000, &[info("b", true)], &[s("b", 1, 1, None)]).unwrap();
        st.insert_traffic("r2", 9_000, &[info("c", true)], &[s("c", 1, 1, None)]).unwrap();
        assert_eq!(st.purge_traffic(5_000).unwrap(), 1);
        assert_eq!(st.count_traffic_samples().unwrap(), 2);
        assert_eq!(st.list_traffic_ifaces("r1").unwrap().iter().map(|i| i.name.as_str()).collect::<Vec<_>>(), vec!["b"], "antarmuka lama ikut dibersihkan");
        st.delete_traffic_of_router("r1").unwrap();
        assert_eq!((st.count_traffic_samples().unwrap(), st.list_traffic_ifaces("r1").unwrap().len(), st.list_traffic_ifaces("r2").unwrap().len()), (1, 0, 1));
    }
}
