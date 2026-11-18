//! Pemantau satu router: polling berkala -> event tersimpan di SQLite, plus sinkronisasi
//! user/profile dari router (saat mulai dan berkala), dan tugas purge retensi terpisah.

use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokio::sync::{mpsc, watch};
use tokio::time::{Instant, MissedTickBehavior, interval};

use crate::events::{Detector, Event};
use crate::poller::poll_once;
use crate::router::{Error, RouterApi, ppp_active, ppp_profiles, ppp_secrets};
use crate::store::{RetentionPolicy, Store};

/// Akses ke Store dibagi antar tugas; kunci hanya dipegang sebentar dan tidak melewati `await`.
pub type SharedStore = Arc<Mutex<Store>>;

#[derive(Debug, Clone, Copy)]
pub struct MonitorConfig {
    pub poll_every: Duration,
    /// Sinkronisasi user/profile berkala; `None` = hanya sekali saat mulai (dan manual).
    pub sync_every: Option<Duration>,
}

fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

/// Ambil secret, profile, dan sesi aktif lalu timpa salinan lokal (Mikrotik = sumber kebenaran).
pub async fn sync_router(api: &impl RouterApi, store: &SharedStore, router_id: &str) -> Result<(), Error> {
    let (secrets, profiles, active) = (ppp_secrets(api).await?, ppp_profiles(api).await?, ppp_active(api).await?);
    let now = now_ms();
    let mut s = store.lock().expect("store lock");
    // Galat basis data tidak boleh mematikan pemantau; dilaporkan sebagai galat fatal untuk dicatat pemanggil.
    s.sync_profiles(router_id, now, &profiles).map_err(|e| Error::Fatal(format!("simpan profile: {e}")))?;
    s.sync_users(router_id, now, &secrets, &active).map_err(|e| Error::Fatal(format!("simpan user: {e}")))?;
    Ok(())
}

/// Jalankan sampai `stop` bernilai true atau berubah. Event baru disimpan lalu (opsional)
/// diteruskan ke `tx` untuk UI; penerima yang ditutup tidak menghentikan pemantauan.
pub async fn run_router(
    api: impl RouterApi,
    store: SharedStore,
    router_id: String,
    cfg: MonitorConfig,
    tx: Option<mpsc::Sender<Event>>,
    mut stop: watch::Receiver<bool>,
) {
    let mut det = Detector::new();
    let mut tick = interval(cfg.poll_every);
    tick.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut last_sync: Option<Instant> = None;
    let mut synced_once = false;
    loop {
        tokio::select! {
            _ = tick.tick() => {}
            _ = stop.changed() => return,
        }
        if *stop.borrow() {
            return;
        }

        // Sinkronisasi dulu supaya acuan sesi dan data user sudah ada saat event datang.
        let due = match (synced_once, cfg.sync_every, last_sync) {
            (false, _, _) => true,
            (true, Some(every), Some(at)) => at.elapsed() >= every,
            _ => false,
        };
        if due && sync_router(&api, &store, &router_id).await.is_ok() {
            synced_once = true;
            last_sync = Some(Instant::now());
        }

        let events = poll_once(&api, &mut det, now_ms()).await;
        if events.is_empty() {
            continue;
        }
        if let Err(e) = store.lock().expect("store lock").insert_events(&router_id, &events) {
            eprintln!("telemon: gagal menyimpan event {router_id}: {e}");
        }
        if let Some(tx) = &tx {
            for e in events {
                let _ = tx.send(e).await;
            }
        }
    }
}

/// Purge retensi berkala (mis. tiap jam). Kebijakan dibaca ulang tiap putaran lewat `policy`,
/// sehingga perubahan di Pengaturan berlaku tanpa memulai ulang.
pub async fn run_purge(store: SharedStore, policy: impl Fn() -> RetentionPolicy, every: Duration, mut stop: watch::Receiver<bool>) {
    let mut tick = interval(every);
    tick.set_missed_tick_behavior(MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            _ = tick.tick() => {}
            _ = stop.changed() => return,
        }
        if *stop.borrow() {
            return;
        }
        if let Err(e) = store.lock().expect("store lock").purge(now_ms(), &policy()) {
            eprintln!("telemon: purge gagal: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Row;
    use crate::poller::UserStatus;
    use crate::store::EventQuery;
    use std::collections::VecDeque;

    fn row(kv: &[(&str, &str)]) -> Row {
        kv.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    /// Router tiruan: secret dan profile tetap; sesi aktif berganti tiap pembacaan `/ppp/active`
    /// (hanya dihitung setelah sinkronisasi pertama, yang juga membaca `/ppp/active`).
    struct Fake {
        active: Mutex<VecDeque<Vec<Row>>>,
    }

    impl RouterApi for Fake {
        async fn print(&self, path: &str) -> Result<Vec<Row>, Error> {
            Ok(match path {
                "/ppp/secret" => vec![
                    row(&[(".id", "*1"), ("name", "budi"), ("profile", "10M"), ("disabled", "false")]),
                    row(&[(".id", "*2"), ("name", "sari"), ("profile", "20M"), ("disabled", "false")]),
                ],
                "/ppp/profile" => vec![row(&[(".id", "*9"), ("name", "10M"), ("rate-limit", "10M/10M")])],
                "/ppp/active" => {
                    let mut q = self.active.lock().unwrap();
                    if q.len() > 1 { q.pop_front().unwrap() } else { q.front().cloned().unwrap_or_default() }
                }
                _ => vec![],
            })
        }
    }

    async fn wait_until(mut f: impl FnMut() -> bool) {
        for _ in 0..300 {
            if f() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("kondisi tidak tercapai tepat waktu");
    }

    #[tokio::test]
    async fn router_ke_event_tersimpan_dan_status_user_berubah() {
        let store: SharedStore = Arc::new(Mutex::new(Store::open_in_memory().unwrap()));
        let act = |id: &str, name: &str| row(&[(".id", id), ("name", name), ("service", "pppoe"), ("address", "100.64.0.5"), ("caller-id", "AA:BB"), ("uptime", "5s")]);
        // Urutan baca /ppp/active: sinkronisasi (budi), polling acuan (budi), polling 2 (sari menggantikan budi).
        let api = Fake { active: Mutex::new(VecDeque::from(vec![vec![act("*a1", "budi")], vec![act("*a1", "budi")], vec![act("*a2", "sari")]])) };
        let (stop_tx, stop_rx) = watch::channel(false);
        let (tx, mut rx) = mpsc::channel(16);
        let cfg = MonitorConfig { poll_every: Duration::from_millis(10), sync_every: None };
        let h = tokio::spawn(run_router(api, store.clone(), "r1".into(), cfg, Some(tx), stop_rx));

        wait_until(|| store.lock().unwrap().count_events().unwrap() >= 2).await;
        stop_tx.send(true).unwrap();
        tokio::time::timeout(Duration::from_secs(2), h).await.expect("berhenti").unwrap();

        let s = store.lock().unwrap();
        let evs = s.list_events(&EventQuery { router_id: Some("r1".into()), limit: 10, ..Default::default() }).unwrap();
        let kinds: Vec<_> = evs.iter().map(|e| (e.event.kind.as_str(), e.event.username.clone().unwrap())).collect();
        assert!(kinds.contains(&("disconnected", "budi".to_string())), "{kinds:?}");
        assert!(kinds.contains(&("connected", "sari".to_string())), "{kinds:?}");

        // Status user mengikuti event tanpa menunggu sinkronisasi berikutnya.
        let users = s.list_users(Some("r1"), false).unwrap();
        let st = |n: &str| users.iter().find(|u| u.username == n).unwrap().status;
        assert_eq!(st("budi"), UserStatus::Inactive);
        assert_eq!(st("sari"), UserStatus::Active);
        assert_eq!(s.list_users(Some("r1"), false).unwrap().len(), 2);

        // Event juga diteruskan ke UI.
        assert!(rx.try_recv().is_ok());
    }

    #[tokio::test]
    async fn purge_berkala_membaca_kebijakan_terbaru() {
        let store: SharedStore = Arc::new(Mutex::new(Store::open_in_memory().unwrap()));
        let old = Event { at_ms: 1, kind: crate::events::EventKind::Connected, username: None, address: None, caller_id: None, message: "lama".into() };
        store.lock().unwrap().insert_events("r1", &[old]).unwrap();
        let (stop_tx, stop_rx) = watch::channel(false);
        let h = tokio::spawn(run_purge(store.clone(), || RetentionPolicy { event_days: Some(1), max_events: 1000, deleted_days: None, audit_days: None }, Duration::from_millis(10), stop_rx));
        wait_until(|| store.lock().unwrap().count_events().unwrap() == 0).await;
        stop_tx.send(true).unwrap();
        tokio::time::timeout(Duration::from_secs(2), h).await.expect("berhenti").unwrap();
    }
}
