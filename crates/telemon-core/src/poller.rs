//! Polling berkala satu router: ambil sesi aktif dan log, ubah jadi event.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokio::sync::mpsc;
use tokio::time::{MissedTickBehavior, interval};

use crate::events::{Detector, Event, LogLine};
use crate::model::{PppActive, PppSecret};
use crate::router::{Error, RouterApi, ppp_active};

/// Status user di UI: dinonaktifkan di router, sedang punya sesi aktif, atau tidak aktif.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserStatus {
    Active,
    Inactive,
    Disabled,
}

pub fn user_status(secret: &PppSecret, active: &[PppActive]) -> UserStatus {
    if secret.disabled {
        UserStatus::Disabled
    } else if active.iter().any(|a| a.name == secret.name) {
        UserStatus::Active
    } else {
        UserStatus::Inactive
    }
}

fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

/// Satu putaran: baca router lalu jalankan detektor. Galat jaringan menjadi event RouterDown.
pub async fn poll_once(api: &impl RouterApi, det: &mut Detector, now_ms: i64) -> Vec<Event> {
    poll_once_status(api, det, now_ms).await.0
}

/// Seperti [`poll_once`], dan mengembalikan juga hasil bacaan (untuk memperbarui status router).
pub async fn poll_once_status(api: &impl RouterApi, det: &mut Detector, now_ms: i64) -> (Vec<Event>, Result<(), Error>) {
    match fetch(api).await {
        Ok((active, log)) => (det.step(now_ms, active, log), Ok(())),
        Err(e) => (det.on_error(now_ms, &e.to_string()), Err(e)),
    }
}

async fn fetch(api: &impl RouterApi) -> Result<(Vec<PppActive>, Vec<LogLine>), Error> {
    let active = ppp_active(api).await?;
    let log = api.print("/log").await?.iter().filter_map(LogLine::from_row).collect();
    Ok((active, log))
}

/// Polling terus-menerus sampai penerima ditutup. Polling pertama langsung dijalankan.
pub async fn run_poller(api: impl RouterApi, every: Duration, tx: mpsc::Sender<Event>) {
    let mut det = Detector::new();
    let mut tick = interval(every);
    tick.set_missed_tick_behavior(MissedTickBehavior::Delay);
    loop {
        tick.tick().await;
        for e in poll_once(&api, &mut det, now_ms()).await {
            if tx.send(e).await.is_err() {
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::EventKind;
    use crate::router::Row;
    use std::collections::VecDeque;
    use std::sync::Mutex;

    type Step = Result<(Vec<Row>, Vec<Row>), Error>;

    /// Router tiruan berskrip: tiap polling mengambil satu langkah (active, log) atau galat.
    struct Script {
        steps: Mutex<VecDeque<Step>>,
        current: Mutex<Option<(Vec<Row>, Vec<Row>)>>,
    }

    impl Script {
        fn new(steps: Vec<Step>) -> Self {
            Self { steps: Mutex::new(steps.into()), current: Mutex::new(None) }
        }
    }

    impl RouterApi for Script {
        async fn print(&self, path: &str) -> Result<Vec<Row>, Error> {
            if path == "/ppp/active" {
                // Awal putaran: ambil langkah berikutnya (langkah habis = galat putus).
                let next = self.steps.lock().unwrap().pop_front().unwrap_or_else(|| Err(Error::Fatal("habis".into())))?;
                let rows = next.0.clone();
                *self.current.lock().unwrap() = Some(next);
                return Ok(rows);
            }
            Ok(self.current.lock().unwrap().as_ref().map(|c| c.1.clone()).unwrap_or_default())
        }
    }

    fn row(kv: &[(&str, &str)]) -> Row {
        kv.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }
    fn active(id: &str, name: &str) -> Row {
        row(&[(".id", id), ("name", name), ("service", "pppoe")])
    }

    #[tokio::test]
    async fn poll_once_urutan_event() {
        let api = Script::new(vec![
            Ok((vec![active("*1", "budi")], vec![])),
            Ok((vec![active("*1", "budi"), active("*2", "sari")], vec![row(&[(".id", "*5"), ("topics", "ppp,error"), ("message", "ani: authentication failed")])])),
            Err(Error::Fatal("putus".into())),
            Ok((vec![], vec![])),
        ]);
        let mut d = Detector::new();
        assert!(poll_once(&api, &mut d, 1).await.is_empty());
        let e = poll_once(&api, &mut d, 2).await;
        let k: Vec<_> = e.iter().map(|x| x.kind).collect();
        assert_eq!(k, vec![EventKind::Connected, EventKind::AuthFailed]);
        let e = poll_once(&api, &mut d, 3).await;
        assert_eq!(e.len(), 1);
        assert_eq!(e[0].kind, EventKind::RouterDown);
        let e = poll_once(&api, &mut d, 4).await;
        let k: Vec<_> = e.iter().map(|x| x.kind).collect();
        assert_eq!(k, vec![EventKind::RouterUp, EventKind::Disconnected, EventKind::Disconnected]);
    }

    #[tokio::test]
    async fn run_poller_mengirim_event_dan_berhenti() {
        let api = Script::new(vec![
            Ok((vec![], vec![])),
            Ok((vec![active("*1", "budi")], vec![])),
        ]);
        let (tx, mut rx) = mpsc::channel(8);
        let h = tokio::spawn(run_poller(api, Duration::from_millis(10), tx));
        let ev = tokio::time::timeout(Duration::from_secs(2), rx.recv()).await.expect("event datang").unwrap();
        assert_eq!(ev.kind, EventKind::Connected);
        assert_eq!(ev.username.as_deref(), Some("budi"));
        drop(rx);
        tokio::time::timeout(Duration::from_secs(2), h).await.expect("poller berhenti").unwrap();
    }

    #[test]
    fn status_user() {
        let mut s = PppSecret::from_row(&row(&[("name", "budi")]));
        let a = vec![PppActive::from_row(&active("*1", "budi"))];
        assert_eq!(user_status(&s, &a), UserStatus::Active);
        assert_eq!(user_status(&s, &[]), UserStatus::Inactive);
        s.disabled = true;
        assert_eq!(user_status(&s, &a), UserStatus::Disabled);
    }
}
