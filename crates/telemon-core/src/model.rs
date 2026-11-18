//! Model PPP hasil baca router. Parser menerima baris atribut RouterOS apa adanya.

use std::collections::HashMap;
use std::fmt;

pub type Row = HashMap<String, String>;

fn get(r: &Row, k: &str) -> String {
    r.get(k).cloned().unwrap_or_default()
}

fn opt(r: &Row, k: &str) -> Option<String> {
    r.get(k).filter(|v| !v.is_empty()).cloned()
}

/// RouterOS menulis boolean sebagai `true`/`false` (v7) atau `yes`/`no` (v6).
pub fn is_true(v: &str) -> bool {
    matches!(v, "true" | "yes")
}

/// "1w2d3h4m5s" -> detik. Mengembalikan None bila formatnya tidak dikenal.
pub fn parse_uptime(s: &str) -> Option<u64> {
    if s.is_empty() {
        return None;
    }
    let (mut total, mut num, mut seen) = (0u64, 0u64, false);
    for c in s.chars() {
        if let Some(d) = c.to_digit(10) {
            num = num.checked_mul(10)?.checked_add(u64::from(d))?;
            seen = true;
            continue;
        }
        let unit = match c {
            'w' => 604_800,
            'd' => 86_400,
            'h' => 3_600,
            'm' => 60,
            's' => 1,
            _ => return None,
        };
        if !seen {
            return None;
        }
        total = total.checked_add(num.checked_mul(unit)?)?;
        num = 0;
        seen = false;
    }
    if seen { None } else { Some(total) }
}

/// Sesi PPP aktif (`/ppp/active`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PppActive {
    pub id: String,
    pub name: String,
    pub service: String,
    pub address: Option<String>,
    pub caller_id: Option<String>,
    pub uptime_secs: Option<u64>,
}

impl PppActive {
    pub fn from_row(r: &Row) -> Self {
        Self {
            id: get(r, ".id"),
            name: get(r, "name"),
            service: get(r, "service"),
            address: opt(r, "address"),
            caller_id: opt(r, "caller-id"),
            uptime_secs: r.get("uptime").and_then(|u| parse_uptime(u)),
        }
    }
}

/// Secret PPP (`/ppp/secret`). Password hanya ada di memori; `Debug` tidak mencetaknya (ADR 0009).
#[derive(Clone, PartialEq, Eq)]
pub struct PppSecret {
    pub id: String,
    pub name: String,
    pub service: String,
    pub profile: String,
    pub disabled: bool,
    pub remote_address: Option<String>,
    pub comment: Option<String>,
    pub last_logged_out: Option<String>,
    pub password: Option<String>,
}

impl PppSecret {
    pub fn from_row(r: &Row) -> Self {
        Self {
            id: get(r, ".id"),
            name: get(r, "name"),
            service: get(r, "service"),
            profile: get(r, "profile"),
            disabled: is_true(&get(r, "disabled")),
            remote_address: opt(r, "remote-address"),
            comment: opt(r, "comment"),
            last_logged_out: opt(r, "last-logged-out"),
            password: opt(r, "password"),
        }
    }
}

impl fmt::Debug for PppSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PppSecret")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("service", &self.service)
            .field("profile", &self.profile)
            .field("disabled", &self.disabled)
            .field("remote_address", &self.remote_address)
            .field("comment", &self.comment)
            .field("last_logged_out", &self.last_logged_out)
            .field("password", &self.password.as_ref().map(|_| "<disembunyikan>"))
            .finish()
    }
}

/// Profile PPP (`/ppp/profile`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PppProfile {
    pub id: String,
    pub name: String,
    pub rate_limit: Option<String>,
    pub local_address: Option<String>,
    pub remote_address: Option<String>,
    pub only_one: Option<String>,
    pub comment: Option<String>,
}

impl PppProfile {
    pub fn from_row(r: &Row) -> Self {
        Self {
            id: get(r, ".id"),
            name: get(r, "name"),
            rate_limit: opt(r, "rate-limit"),
            local_address: opt(r, "local-address"),
            remote_address: opt(r, "remote-address"),
            only_one: opt(r, "only-one"),
            comment: opt(r, "comment"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(kv: &[(&str, &str)]) -> Row {
        kv.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    #[test]
    fn uptime() {
        assert_eq!(parse_uptime("1w2d3h4m5s"), Some(604_800 + 2 * 86_400 + 3 * 3_600 + 4 * 60 + 5));
        assert_eq!(parse_uptime("45s"), Some(45));
        assert_eq!(parse_uptime("10m"), Some(600));
        assert_eq!(parse_uptime(""), None);
        assert_eq!(parse_uptime("12"), None);
        assert_eq!(parse_uptime("h"), None);
        assert_eq!(parse_uptime("1x"), None);
        assert_eq!(parse_uptime("99999999999999999999999s"), None);
    }

    #[test]
    fn booleans() {
        assert!(is_true("true") && is_true("yes"));
        assert!(!is_true("false") && !is_true("no") && !is_true(""));
    }

    #[test]
    fn active_row() {
        let a = PppActive::from_row(&row(&[(".id", "*1"), ("name", "budi"), ("service", "pppoe"), ("address", "100.64.1.2"), ("caller-id", "AA:BB:CC:DD:EE:FF"), ("uptime", "1d2h")]));
        assert_eq!(a.name, "budi");
        assert_eq!(a.uptime_secs, Some(86_400 + 7_200));
        assert_eq!(a.caller_id.as_deref(), Some("AA:BB:CC:DD:EE:FF"));
        let b = PppActive::from_row(&row(&[("name", "x")]));
        assert_eq!(b.address, None);
    }

    #[test]
    fn secret_row_and_redaction() {
        let s = PppSecret::from_row(&row(&[(".id", "*2"), ("name", "sari"), ("profile", "10M"), ("disabled", "yes"), ("password", "rahasia123"), ("comment", "")]));
        assert!(s.disabled);
        assert_eq!(s.comment, None);
        assert_eq!(s.password.as_deref(), Some("rahasia123"));
        assert!(!format!("{s:?}").contains("rahasia123"));
    }

    #[test]
    fn profile_row() {
        let p = PppProfile::from_row(&row(&[(".id", "*3"), ("name", "10M"), ("rate-limit", "10M/10M"), ("only-one", "yes")]));
        assert_eq!(p.rate_limit.as_deref(), Some("10M/10M"));
        assert_eq!(p.local_address, None);
    }
}
