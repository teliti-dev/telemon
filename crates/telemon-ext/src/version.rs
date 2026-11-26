//! Versi dan rentang versi (semver sederhana) untuk kompatibilitas extension (ADR 0028).
//!
//! Versi = `X.Y.Z`; bagian praterbit (`-rc.6`) dan build (`+x`) diabaikan saat membandingkan, supaya
//! `0.1.0-rc.6` dianggap `0.1.0`. Rentang = pembanding dipisah spasi (semua harus cocok):
//! `>=1.2.0 <2.0.0`, `^1.2.3` (>=1.2.3 <2.0.0; untuk 0.x: <0.(y+1).0), `~1.2.3` (>=1.2.3 <1.3.0), `=1.2.3`, `*`.

use std::cmp::Ordering;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version(pub u64, pub u64, pub u64);

impl Version {
    pub fn parse(s: &str) -> Option<Self> {
        let core = s.trim().split(['-', '+']).next()?;
        let mut it = core.split('.');
        let v = |p: Option<&str>| -> Option<u64> {
            let p = p?;
            (!p.is_empty() && p.len() <= 9 && p.chars().all(|c| c.is_ascii_digit())).then(|| p.parse().ok())?
        };
        let (a, b, c) = (v(it.next())?, v(it.next())?, v(it.next())?);
        it.next().is_none().then_some(Self(a, b, c))
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.0, self.1, self.2)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Op {
    Ge,
    Gt,
    Le,
    Lt,
    Eq,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Range {
    text: String,
    parts: Vec<(Op, Version)>,
}

impl Range {
    pub fn parse(s: &str) -> Result<Self, String> {
        let text = s.trim().to_string();
        if text.is_empty() || text.len() > 80 {
            return Err("Rentang versi kosong atau terlalu panjang".into());
        }
        let mut parts = Vec::new();
        for tok in text.split_whitespace() {
            if tok == "*" {
                continue;
            }
            let bad = || format!("Rentang versi tidak valid: {tok}");
            if let Some(r) = tok.strip_prefix('^') {
                let v = Version::parse(r).ok_or_else(bad)?;
                let upper = if v.0 > 0 { Version(v.0 + 1, 0, 0) } else if v.1 > 0 { Version(0, v.1 + 1, 0) } else { Version(0, 0, v.2 + 1) };
                parts.push((Op::Ge, v));
                parts.push((Op::Lt, upper));
            } else if let Some(r) = tok.strip_prefix('~') {
                let v = Version::parse(r).ok_or_else(bad)?;
                parts.push((Op::Ge, v));
                parts.push((Op::Lt, Version(v.0, v.1 + 1, 0)));
            } else {
                let (op, rest) = if let Some(r) = tok.strip_prefix(">=") {
                    (Op::Ge, r)
                } else if let Some(r) = tok.strip_prefix("<=") {
                    (Op::Le, r)
                } else if let Some(r) = tok.strip_prefix('>') {
                    (Op::Gt, r)
                } else if let Some(r) = tok.strip_prefix('<') {
                    (Op::Lt, r)
                } else if let Some(r) = tok.strip_prefix('=') {
                    (Op::Eq, r)
                } else {
                    (Op::Eq, tok)
                };
                parts.push((op, Version::parse(rest).ok_or_else(bad)?));
            }
        }
        Ok(Self { text, parts })
    }

    pub fn matches(&self, v: Version) -> bool {
        self.parts.iter().all(|(op, r)| {
            let o = v.cmp(r);
            match op {
                Op::Ge => o != Ordering::Less,
                Op::Gt => o == Ordering::Greater,
                Op::Le => o != Ordering::Greater,
                Op::Lt => o == Ordering::Less,
                Op::Eq => o == Ordering::Equal,
            }
        })
    }

    pub fn as_str(&self) -> &str {
        &self.text
    }
}
