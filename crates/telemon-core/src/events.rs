//! Deteksi event PPP: selisih `/ppp/active` antar polling (connected/disconnected) dan baris
//! log PPP (authenticated/auth failed). Murni tanpa IO supaya mudah diuji.

use std::collections::HashMap;

use crate::model::{PppActive, Row};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventKind {
    AuthFailed,
    Authenticated,
    Connected,
    Disconnected,
    RouterDown,
    RouterUp,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    /// Waktu polling yang mendeteksi (epoch ms). Jam log router tidak dipakai karena formatnya
    /// bergantung zona waktu dan tanggal router.
    pub at_ms: i64,
    pub kind: EventKind,
    pub username: Option<String>,
    pub address: Option<String>,
    pub caller_id: Option<String>,
    /// Baris log asli (untuk event dari log) atau ringkasan Telemon.
    pub message: String,
}

impl Event {
    fn new(at_ms: i64, kind: EventKind, message: String) -> Self {
        Self { at_ms, kind, username: None, address: None, caller_id: None, message }
    }
}

/// Satu baris `/log/print`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogLine {
    /// Nomor urut dari `.id` (`*1A` = 26); naik terus selama router tidak reboot.
    pub id: u64,
    pub topics: Vec<String>,
    pub message: String,
}

impl LogLine {
    pub fn from_row(r: &Row) -> Option<Self> {
        let id = u64::from_str_radix(r.get(".id")?.trim_start_matches('*'), 16).ok()?;
        let topics = r.get("topics").map(|t| t.split(',').map(|s| s.trim().to_string()).collect()).unwrap_or_default();
        Some(Self { id, topics, message: r.get("message").cloned().unwrap_or_default() })
    }
}

const PROTO_PREFIXES: [&str; 5] = ["pppoe-", "pptp-", "l2tp-", "sstp-", "ovpn-"];

/// Pisahkan "<pppoe-budi>: authenticated" atau "budi: authentication failed" menjadi (user, sisa).
fn split_user(msg: &str) -> Option<(String, &str)> {
    let msg = msg.trim();
    let (user, rest) = if let Some(inner) = msg.strip_prefix('<') {
        let end = inner.find('>')?;
        let name = &inner[..end];
        let name = PROTO_PREFIXES.iter().find_map(|p| name.strip_prefix(p)).unwrap_or(name);
        (name, inner[end + 1..].trim_start_matches([':', ' ']))
    } else {
        let i = msg.find(": ")?;
        (&msg[..i], &msg[i + 2..])
    };
    (!user.is_empty()).then(|| (user.to_string(), rest.trim()))
}

/// Hanya authenticated dan auth failed yang diambil dari log; connected/disconnected dari selisih sesi.
pub fn parse_log_event(line: &LogLine) -> Option<(EventKind, String)> {
    if !line.topics.iter().any(|t| t == "ppp") {
        return None;
    }
    let (user, rest) = split_user(&line.message)?;
    let r = rest.to_lowercase();
    let kind = if r.contains("authentication failed") || r.contains("login failed") {
        EventKind::AuthFailed
    } else if r.contains("authenticated") {
        EventKind::Authenticated
    } else {
        return None;
    };
    Some((kind, user))
}

#[derive(Debug, Default)]
pub struct Detector {
    baselined: bool,
    sessions: HashMap<String, PppActive>,
    last_log_id: Option<u64>,
    down: bool,
}

impl Detector {
    pub fn new() -> Self {
        Self::default()
    }

    /// Polling gagal. Mengembalikan RouterDown hanya pada kegagalan pertama berturut-turut.
    pub fn on_error(&mut self, now_ms: i64, reason: &str) -> Vec<Event> {
        if self.down {
            return Vec::new();
        }
        self.down = true;
        vec![Event::new(now_ms, EventKind::RouterDown, format!("Router tidak terjangkau: {reason}"))]
    }

    /// Polling berhasil. Polling pertama hanya menjadi acuan (tanpa event) agar menyalakan
    /// Telemon tidak membanjiri riwayat.
    pub fn step(&mut self, now_ms: i64, active: Vec<PppActive>, log: Vec<LogLine>) -> Vec<Event> {
        let mut out = Vec::new();
        if self.down {
            self.down = false;
            out.push(Event::new(now_ms, EventKind::RouterUp, "Router kembali terjangkau".into()));
        }

        let cur: HashMap<String, PppActive> = active.into_iter().map(|a| (a.id.clone(), a)).collect();
        if self.baselined {
            let mut gone: Vec<&PppActive> = self.sessions.values().filter(|a| !cur.contains_key(&a.id)).collect();
            gone.sort_by(|a, b| a.id.cmp(&b.id));
            for a in gone {
                out.push(session_event(now_ms, EventKind::Disconnected, a, format!("{} terputus", a.name)));
            }
            let mut new: Vec<&PppActive> = cur.values().filter(|a| !self.sessions.contains_key(&a.id)).collect();
            new.sort_by(|a, b| a.id.cmp(&b.id));
            for a in new {
                out.push(session_event(now_ms, EventKind::Connected, a, format!("{} terhubung", a.name)));
            }
        }

        let max_id = log.iter().map(|l| l.id).max();
        // Router reboot mengosongkan log dan nomor mulai dari awal: anggap semua baris baru.
        let reset = matches!((self.last_log_id, max_id), (Some(last), Some(max)) if max < last);
        let floor = if reset { None } else { self.last_log_id };
        if self.baselined {
            let mut fresh: Vec<&LogLine> = log.iter().filter(|l| floor.is_none_or(|f| l.id > f)).collect();
            fresh.sort_by_key(|l| l.id);
            for l in fresh {
                if let Some((kind, user)) = parse_log_event(l) {
                    let mut e = Event::new(now_ms, kind, l.message.clone());
                    e.username = Some(user);
                    out.push(e);
                }
            }
        }
        if max_id.is_some() {
            self.last_log_id = max_id;
        }

        self.sessions = cur;
        self.baselined = true;
        out
    }
}

fn session_event(at_ms: i64, kind: EventKind, a: &PppActive, message: String) -> Event {
    Event {
        at_ms,
        kind,
        username: Some(a.name.clone()),
        address: a.address.clone(),
        caller_id: a.caller_id.clone(),
        message,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sess(id: &str, name: &str) -> PppActive {
        PppActive { id: id.into(), name: name.into(), service: "pppoe".into(), address: Some("100.64.1.2".into()), caller_id: Some("AA:BB".into()), uptime_secs: Some(5) }
    }
    fn log(id: u64, topics: &str, msg: &str) -> LogLine {
        LogLine { id, topics: topics.split(',').map(String::from).collect(), message: msg.into() }
    }
    fn kinds(e: &[Event]) -> Vec<(EventKind, Option<&str>)> {
        e.iter().map(|x| (x.kind, x.username.as_deref())).collect()
    }

    #[test]
    fn polling_pertama_hanya_acuan() {
        let mut d = Detector::new();
        let e = d.step(1, vec![sess("*1", "budi")], vec![log(5, "pppoe,ppp,error", "sari: authentication failed")]);
        assert!(e.is_empty());
    }

    #[test]
    fn connected_dan_disconnected_dari_selisih() {
        let mut d = Detector::new();
        d.step(1, vec![sess("*1", "budi")], vec![]);
        let e = d.step(2, vec![sess("*1", "budi"), sess("*2", "sari")], vec![]);
        assert_eq!(kinds(&e), vec![(EventKind::Connected, Some("sari"))]);
        assert_eq!(e[0].address.as_deref(), Some("100.64.1.2"));
        assert_eq!(e[0].at_ms, 2);
        let e = d.step(3, vec![sess("*2", "sari")], vec![]);
        assert_eq!(kinds(&e), vec![(EventKind::Disconnected, Some("budi"))]);
        assert!(d.step(4, vec![sess("*2", "sari")], vec![]).is_empty());
    }

    #[test]
    fn sambung_ulang_dengan_id_baru() {
        let mut d = Detector::new();
        d.step(1, vec![sess("*1", "budi")], vec![]);
        let e = d.step(2, vec![sess("*9", "budi")], vec![]);
        assert_eq!(kinds(&e), vec![(EventKind::Disconnected, Some("budi")), (EventKind::Connected, Some("budi"))]);
    }

    #[test]
    fn auth_failed_dan_authenticated_dari_log() {
        let mut d = Detector::new();
        d.step(1, vec![], vec![log(10, "pppoe,ppp,info", "lama: authenticated")]);
        let e = d.step(
            2,
            vec![],
            vec![
                log(10, "pppoe,ppp,info", "lama: authenticated"),
                log(11, "pppoe,ppp,error", "budi: authentication failed"),
                log(12, "pppoe,ppp,info", "<pppoe-sari>: authenticated"),
                log(13, "system,info", "budi: authentication failed"),
                log(14, "pppoe,ppp,info", "<pppoe-sari>: connected"),
            ],
        );
        assert_eq!(kinds(&e), vec![(EventKind::AuthFailed, Some("budi")), (EventKind::Authenticated, Some("sari"))]);
        assert_eq!(e[0].message, "budi: authentication failed");
        // Baris yang sama tidak diproses dua kali.
        let again = d.step(3, vec![], vec![log(11, "pppoe,ppp,error", "budi: authentication failed"), log(12, "pppoe,ppp,info", "<pppoe-sari>: authenticated"), log(14, "pppoe,ppp,info", "<pppoe-sari>: connected")]);
        assert!(again.is_empty());
    }

    #[test]
    fn log_reset_setelah_reboot() {
        let mut d = Detector::new();
        d.step(1, vec![], vec![log(500, "ppp", "x: authenticated")]);
        let e = d.step(2, vec![], vec![log(1, "ppp,error", "budi: authentication failed")]);
        assert_eq!(kinds(&e), vec![(EventKind::AuthFailed, Some("budi"))]);
    }

    #[test]
    fn router_down_sekali_lalu_up() {
        let mut d = Detector::new();
        d.step(1, vec![sess("*1", "budi")], vec![]);
        let e = d.on_error(2, "timeout");
        assert_eq!(e.len(), 1);
        assert_eq!(e[0].kind, EventKind::RouterDown);
        assert!(d.on_error(3, "timeout").is_empty());
        // Saat pulih, sesi yang hilang selama gangguan dilaporkan terputus.
        let e = d.step(4, vec![], vec![]);
        assert_eq!(kinds(&e), vec![(EventKind::RouterUp, None), (EventKind::Disconnected, Some("budi"))]);
    }

    #[test]
    fn parser_log_variasi() {
        let p = |m: &str| parse_log_event(&log(1, "pppoe,ppp,info", m));
        assert_eq!(p("<l2tp-ani>: authenticated"), Some((EventKind::Authenticated, "ani".into())));
        assert_eq!(p("ani: login failed"), Some((EventKind::AuthFailed, "ani".into())));
        assert_eq!(p("<pppoe-ani>: terminating... - hung up"), None);
        assert_eq!(p("tanpa pemisah"), None);
        assert_eq!(p(": authenticated"), None);
    }

    #[test]
    fn baris_log_dari_row() {
        let mut r = Row::new();
        r.insert(".id".into(), "*1A".into());
        r.insert("topics".into(), "pppoe,ppp,error".into());
        r.insert("message".into(), "x: authentication failed".into());
        let l = LogLine::from_row(&r).unwrap();
        assert_eq!(l.id, 26);
        assert_eq!(l.topics, vec!["pppoe", "ppp", "error"]);
        assert!(LogLine::from_row(&Row::new()).is_none());
    }
}
