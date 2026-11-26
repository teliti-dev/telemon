//! Alat analitik baca untuk AI (ADR 0027): statistik event, ringkasan bandwidth, dan kesehatan router.
//! Semuanya hanya membaca dan menegakkan izin penanya seperti alat lain di `tools`. Angka dihitung di sini
//! (agregasi SQL dan aritmetika), bukan oleh model, supaya jawabannya bisa diperiksa.

use std::collections::BTreeMap;

use serde_json::{Value, json};
use telemon_core::auth::{Level, Resource};
use telemon_core::traffic::PPPOE_AGGREGATE;
use telemon_core::{EventKind, EventQuery};

use crate::tools::{MAX_ROWS, Names, ToolCtx, ToolOutput, err, fmt_utc, link, n, need, s};
use crate::types::{AiTone, AiWidget, BarItem, ListRow, StatItem};

const HOUR: i64 = 3_600_000;

/// Teks tampilan bps: "850 Kbps", "12.3 Mbps".
fn bps(v: f64) -> String {
    let (mut v, mut i) = (v.max(0.0), 0);
    let units = ["bps", "Kbps", "Mbps", "Gbps", "Tbps"];
    while v >= 1000.0 && i < units.len() - 1 {
        v /= 1000.0;
        i += 1;
    }
    if i == 0 || v >= 100.0 { format!("{} {}", v.round() as i64, units[i]) } else { format!("{:.1} {}", v, units[i]) }
}

fn resolve_router(ctx: &ToolCtx, names: &Names, a: &Value, step: &str) -> Result<Option<String>, ToolOutput> {
    match s(a, "router") {
        Some(w) if w.eq_ignore_ascii_case("all") || w.eq_ignore_ascii_case("semua") => Ok(None),
        Some(w) => names.resolve(&w).map(Some).ok_or_else(|| err(step.to_string(), format!("router '{w}' tidak ditemukan"))),
        None => {
            let _ = ctx;
            Ok(None)
        }
    }
}

// ---------------------------------------------------------------- statistik event

pub(crate) fn event_stats(ctx: &ToolCtx, a: &Value) -> ToolOutput {
    let step = ctx.lang.t("Menghitung statistik event", "Computing event statistics").to_string();
    if let Err(e) = need(ctx, Resource::Event, "Event", &step) {
        return e;
    }
    let names = Names::load(ctx);
    let router = match resolve_router(ctx, &names, a, &step) {
        Ok(r) => r,
        Err(e) => return e,
    };
    let hours = n(a, "hours", 24, 720);
    let until = ctx.now_ms;
    let since = until - hours * HOUR;
    let bucket = if hours <= 48 { HOUR } else { 24 * HOUR };
    let daily = bucket != HOUR;

    let (timeline, top_auth, top_drop, per_router) = {
        let st = ctx.store.lock().expect("lock");
        (
            st.event_timeline(router.as_deref(), since, until, bucket).unwrap_or_default(),
            st.event_top_users("auth_failed", router.as_deref(), since, until, 10).unwrap_or_default(),
            st.event_top_users("disconnected", router.as_deref(), since, until, 10).unwrap_or_default(),
            st.event_counts_by_router(since, until).unwrap_or_default(),
        )
    };

    let mut by_kind: BTreeMap<String, u64> = BTreeMap::new();
    let mut buckets: BTreeMap<i64, BTreeMap<String, u64>> = BTreeMap::new();
    for (b, kind, c) in &timeline {
        *by_kind.entry(kind.clone()).or_default() += c;
        *buckets.entry(*b).or_default().entry(kind.clone()).or_default() += c;
    }
    let total: u64 = by_kind.values().sum();
    let count = |k: &str| by_kind.get(k).copied().unwrap_or(0);
    let label = |b: i64| {
        let t = fmt_utc(b);
        if daily { t[5..10].to_string() } else { t[11..16].to_string() }
    };
    let rows: Vec<Value> = buckets
        .iter()
        .map(|(b, k)| {
            let g = |x: &str| k.get(x).copied().unwrap_or(0);
            json!({ "at": fmt_utc(*b), "total": k.values().sum::<u64>(), "auth_failed": g("auth_failed"), "disconnected": g("disconnected"), "connected": g("connected"), "router_down": g("router_down") })
        })
        .collect();
    let busiest = buckets.iter().max_by_key(|(b, k)| (k.values().sum::<u64>(), **b)).map(|(b, k)| json!({ "at": fmt_utc(*b), "total": k.values().sum::<u64>() }));

    let user_row = |(r, u, c, last): &(String, String, u64, i64)| json!({ "username": u, "router": names.name(r), "count": c, "last": fmt_utc(*last) });
    let mut routers: BTreeMap<String, (u64, u64)> = BTreeMap::new();
    for (r, kind, c) in &per_router {
        if router.as_deref().is_some_and(|x| x != r) {
            continue;
        }
        let e = routers.entry(r.clone()).or_default();
        e.0 += c;
        if kind == "router_down" {
            e.1 += c;
        }
    }
    let mut router_rows: Vec<(String, u64, u64)> = routers.into_iter().map(|(r, (t, d))| (names.name(&r), t, d)).collect();
    router_rows.sort_by(|x, y| y.1.cmp(&x.1).then(x.0.cmp(&y.0)));
    router_rows.truncate(MAX_ROWS);

    let model = json!({
        "hours": hours, "bucket": if daily { "day" } else { "hour" }, "total_events": total,
        "by_kind": by_kind, "busiest_bucket": busiest, "timeline": rows,
        "top_auth_failed": top_auth.iter().map(user_row).collect::<Vec<_>>(),
        "top_disconnected": top_drop.iter().map(user_row).collect::<Vec<_>>(),
        "routers": router_rows.iter().map(|(n, t, d)| json!({ "router": n, "events": t, "router_down": d })).collect::<Vec<_>>(),
    });

    let tone = |c: u64, bad: bool| Some(if c == 0 { AiTone::Good } else if bad { AiTone::Bad } else { AiTone::Warn });
    let mut widgets = vec![AiWidget::Stats {
        items: vec![
            StatItem { label: ctx.lang.t("Event", "Events").into(), value: total.to_string(), tone: None },
            StatItem { label: ctx.lang.t("Gagal auth", "Auth failed").into(), value: count("auth_failed").to_string(), tone: tone(count("auth_failed"), true) },
            StatItem { label: ctx.lang.t("Terputus", "Disconnected").into(), value: count("disconnected").to_string(), tone: tone(count("disconnected"), false) },
            StatItem { label: ctx.lang.t("Router down", "Router down").into(), value: count("router_down").to_string(), tone: tone(count("router_down"), true) },
        ],
    }];
    if buckets.len() > 1 {
        widgets.push(AiWidget::Bars {
            title: if daily { ctx.lang.t("Event per hari (UTC)", "Events per day (UTC)") } else { ctx.lang.t("Event per jam (UTC)", "Events per hour (UTC)") }.into(),
            data: buckets.iter().map(|(b, k)| BarItem { label: label(*b), value: k.values().sum::<u64>() as i64 }).collect(),
        });
    }
    let to_list = |title: &str, list: &[(String, String, u64, i64)], t: AiTone, kind: EventKind| AiWidget::List {
        title: Some(title.into()),
        rows: list
            .iter()
            .map(|(r, u, c, last)| ListRow {
                primary: u.clone(),
                secondary: Some(format!("{} · {}", names.name(r), crate::tools::ago(ctx, *last))),
                trailing: Some(format!("{c}×")),
                tone: Some(t),
                to: Some(link("event", &[("type", kind.as_str())])),
            })
            .collect(),
    };
    if !top_auth.is_empty() {
        widgets.push(to_list(ctx.lang.t("Paling sering gagal auth", "Most auth failures"), &top_auth, AiTone::Bad, EventKind::AuthFailed));
    }
    if !top_drop.is_empty() {
        widgets.push(to_list(ctx.lang.t("Paling sering terputus", "Most disconnects"), &top_drop, AiTone::Warn, EventKind::Disconnected));
    }
    ToolOutput { for_model: model.to_string(), widgets, source: Some(format!("Event ({hours} {})", ctx.lang.t("jam", "h"))), step: format!("{step} — {total}") }
}

// ---------------------------------------------------------------- ringkasan bandwidth

fn range_hours(r: &str) -> Option<i64> {
    Some(match r {
        "1h" => 1,
        "6h" => 6,
        "24h" => 24,
        "7d" => 168,
        "30d" => 720,
        _ => return None,
    })
}

struct Pts {
    /// (waktu, unduh/masuk, unggah/keluar, sesi) dalam bit per detik.
    v: Vec<(i64, f64, f64, Option<f64>)>,
}

impl Pts {
    fn avg(&self, f: impl Fn(&(i64, f64, f64, Option<f64>)) -> f64) -> f64 {
        if self.v.is_empty() { 0.0 } else { self.v.iter().map(f).sum::<f64>() / self.v.len() as f64 }
    }
    fn peak(&self, f: impl Fn(&(i64, f64, f64, Option<f64>)) -> f64) -> (f64, i64) {
        self.v.iter().map(|p| (f(p), p.0)).fold((0.0, 0), |m, x| if x.0 > m.0 { x } else { m })
    }
}

pub(crate) fn bandwidth_summary(ctx: &ToolCtx, a: &Value) -> ToolOutput {
    let step = ctx.lang.t("Menghitung ringkasan bandwidth", "Computing the bandwidth summary").to_string();
    if let Err(e) = need(ctx, Resource::Router, "Router", &step) {
        return e;
    }
    let names = Names::load(ctx);
    let router = match resolve_router(ctx, &names, a, &step) {
        Ok(r) => r,
        Err(e) => return e,
    };
    let iface = s(a, "iface").unwrap_or_else(|| PPPOE_AGGREGATE.to_string());
    if router.is_none() && iface != PPPOE_AGGREGATE {
        return err(step, "total semua router hanya untuk antarmuka '@pppoe'; sebutkan satu router untuk antarmuka lain");
    }
    let range = s(a, "range").unwrap_or_else(|| "24h".into());
    let Some(hours) = range_hours(&range) else { return err(step, "range harus 1h, 6h, 24h, 7d, atau 30d") };
    let settings = ctx.store.lock().expect("lock").get_settings().unwrap_or_default();
    if !settings.bandwidth.enabled {
        return err(step, "pengumpulan bandwidth sedang dimatikan di Pengaturan; beri tahu pengguna untuk menyalakannya");
    }
    let (to, span) = (ctx.now_ms, hours * HOUR);
    let from = to - span;
    let bucket = (i64::from(settings.bandwidth.interval_seconds) * 1000).max((span + 239) / 240);
    let pppoe = iface == PPPOE_AGGREGATE;
    // PPPoE: sudut pandang pelanggan (unduh = tx router). Antarmuka lain mengikuti router (masuk = rx).
    let load = |r: Option<&str>, from: i64, to: i64| -> Pts {
        let rows = ctx.store.lock().expect("lock").traffic_series(r, &iface, from, to, bucket).unwrap_or_default();
        Pts { v: rows.into_iter().map(|p| if pppoe { (p.at, p.tx_bps, p.rx_bps, p.sessions) } else { (p.at, p.rx_bps, p.tx_bps, p.sessions) }).collect() }
    };
    let cur = load(router.as_deref(), from, to);
    if cur.v.is_empty() {
        return err(step, format!("belum ada data bandwidth untuk antarmuka '{iface}' pada rentang {range}; sampel pertama muncul setelah satu interval pembacaan ({} detik)", settings.bandwidth.interval_seconds));
    }
    let prev = load(router.as_deref(), from - span, from - 1);

    let (down_label, up_label) = if pppoe { ("download", "upload") } else { ("in", "out") };
    let now_pt = cur.v.last().copied().unwrap_or((0, 0.0, 0.0, None));
    let (avg_d, avg_u) = (cur.avg(|p| p.1), cur.avg(|p| p.2));
    let ((peak_d, peak_d_at), (peak_u, _)) = (cur.peak(|p| p.1), cur.peak(|p| p.2));
    let sessions: Vec<f64> = cur.v.iter().filter_map(|p| p.3).collect();
    let avg_sessions = if sessions.is_empty() { None } else { Some(sessions.iter().sum::<f64>() / sessions.len() as f64) };
    let per_user: Vec<(f64, f64)> = cur.v.iter().filter_map(|p| p.3.filter(|s| *s > 0.0).map(|s| (p.1 / s, p.2 / s))).collect();
    let per_user_avg = if per_user.is_empty() { None } else { Some((per_user.iter().map(|x| x.0).sum::<f64>() / per_user.len() as f64, per_user.iter().map(|x| x.1).sum::<f64>() / per_user.len() as f64)) };
    let prev_avg_d = if prev.v.is_empty() { None } else { Some(prev.avg(|p| p.1)) };
    let change = prev_avg_d.filter(|p| *p > 0.0).map(|p| ((avg_d - p) / p * 100.0 * 10.0).round() / 10.0);
    // Jam tersibuk (UTC) dari rata-rata per jam dalam sehari, hanya berarti untuk rentang >= 24 jam.
    let busiest_hour = (hours >= 24).then(|| {
        let mut by_hour: BTreeMap<i64, (f64, u32)> = BTreeMap::new();
        for p in &cur.v {
            let e = by_hour.entry((p.0 / HOUR).rem_euclid(24)).or_default();
            e.0 += p.1;
            e.1 += 1;
        }
        by_hour.into_iter().map(|(h, (sum, c))| (h, sum / f64::from(c))).fold((0, 0.0), |m, x| if x.1 > m.1 { x } else { m })
    });

    let mut model = json!({
        "iface": iface, "router": router.as_deref().map(|r| names.name(r)).unwrap_or_else(|| "all".into()), "range": range,
        "direction_note": if pppoe { "download = ke arah pelanggan, upload = dari pelanggan" } else { "in/out menurut sudut pandang router" },
        down_label: { "now": bps(now_pt.1), "average": bps(avg_d), "peak": bps(peak_d), "peak_at": fmt_utc(peak_d_at), "now_bps": now_pt.1.round() as i64, "average_bps": avg_d.round() as i64 },
        up_label: { "now": bps(now_pt.2), "average": bps(avg_u), "peak": bps(peak_u), "now_bps": now_pt.2.round() as i64, "average_bps": avg_u.round() as i64 },
        "samples": cur.v.len(),
    });
    if let Some(sn) = avg_sessions {
        model["average_sessions"] = json!((sn * 10.0).round() / 10.0);
    }
    if let Some((d, u)) = per_user_avg {
        model["average_per_active_user"] = json!({ down_label: bps(d), up_label: bps(u) });
    }
    if let Some(c) = change {
        model["average_download_change_vs_previous_period_percent"] = json!(c);
    }
    if let Some((h, v)) = busiest_hour {
        model["busiest_hour_utc"] = json!({ "hour": h, "average": bps(v) });
    }

    let mut items = vec![
        StatItem { label: if pppoe { ctx.lang.t("Unduh sekarang", "Download now") } else { ctx.lang.t("Masuk sekarang", "In now") }.into(), value: bps(now_pt.1), tone: None },
        StatItem { label: ctx.lang.t("Rata-rata", "Average").into(), value: bps(avg_d), tone: None },
        StatItem { label: ctx.lang.t("Puncak", "Peak").into(), value: bps(peak_d), tone: None },
        StatItem { label: if pppoe { ctx.lang.t("Unggah sekarang", "Upload now") } else { ctx.lang.t("Keluar sekarang", "Out now") }.into(), value: bps(now_pt.2), tone: None },
    ];
    if let Some(sn) = avg_sessions {
        items.push(StatItem { label: ctx.lang.t("Sesi (rata-rata)", "Sessions (avg)").into(), value: format!("{}", sn.round() as i64), tone: None });
    }
    if let Some((d, _)) = per_user_avg {
        items.push(StatItem { label: ctx.lang.t("Per user (unduh)", "Per user (down)").into(), value: bps(d), tone: None });
    }
    let mut widgets = vec![AiWidget::Stats { items }];

    // Rincian per router untuk total pelanggan.
    if router.is_none() && pppoe {
        let routers = ctx.store.lock().expect("lock").list_routers().unwrap_or_default();
        let mut rows: Vec<(String, f64, f64, Option<f64>)> = routers
            .iter()
            .filter_map(|r| {
                let p = load(Some(&r.id), from, to);
                (!p.v.is_empty()).then(|| (r.name.clone(), p.avg(|x| x.1), p.peak(|x| x.1).0, if p.v.iter().any(|x| x.3.is_some()) { Some(p.avg(|x| x.3.unwrap_or(0.0))) } else { None }))
            })
            .collect();
        rows.sort_by(|a, b| b.1.total_cmp(&a.1));
        rows.truncate(MAX_ROWS);
        if !rows.is_empty() {
            model["per_router"] = json!(rows.iter().map(|(n, avg, peak, sess)| json!({ "router": n, "average_download": bps(*avg), "peak_download": bps(*peak), "average_sessions": sess.map(|s| (s * 10.0).round() / 10.0) })).collect::<Vec<_>>());
            if rows.len() > 1 {
                let big = rows.iter().any(|r| r.1 >= 10_000_000.0);
                let (unit, div) = if big { ("Mbps", 1_000_000.0) } else { ("Kbps", 1_000.0) };
                widgets.push(AiWidget::Bars {
                    title: format!("{} ({unit})", ctx.lang.t("Rata-rata unduh per router", "Average download per router")),
                    data: rows.iter().map(|r| BarItem { label: r.0.clone(), value: (r.1 / div).round() as i64 }).collect(),
                });
            }
        }
    }
    ToolOutput { for_model: model.to_string(), widgets, source: Some(format!("Bandwidth ({range})")), step: format!("{step} — {range}") }
}

// ---------------------------------------------------------------- kesehatan router

pub(crate) fn router_health(ctx: &ToolCtx, a: &Value) -> ToolOutput {
    let step = ctx.lang.t("Memeriksa kesehatan router", "Checking router health").to_string();
    if let Err(e) = need(ctx, Resource::Router, "Router", &step) {
        return e;
    }
    let hours = n(a, "hours", 24, 720);
    let (to, span) = (ctx.now_ms, hours * HOUR);
    let from = to - span;
    let may_events = ctx.auth.authorize(ctx.caller, Resource::Event, Level::Read).is_ok();
    let routers = ctx.store.lock().expect("lock").list_routers().unwrap_or_default();
    // Event router_down/router_up jarang, jadi batas 500 baris cukup; diurutkan dari yang terlama.
    let mut events = if may_events {
        let q = EventQuery { kinds: vec![EventKind::RouterDown, EventKind::RouterUp], since_ms: Some(from), limit: 500, ..Default::default() };
        ctx.store.lock().expect("lock").list_events(&q).unwrap_or_default()
    } else {
        vec![]
    };
    events.sort_by_key(|e| e.event.at_ms);

    let mut rows = vec![];
    let mut list = vec![];
    for r in &routers {
        let (mut downs, mut down_ms, mut down_at): (u64, i64, Option<i64>) = (0, 0, None);
        let mut first = true;
        for e in events.iter().filter(|e| e.router_id == r.id) {
            match e.event.kind {
                EventKind::RouterDown => {
                    downs += 1;
                    down_at.get_or_insert(e.event.at_ms);
                }
                EventKind::RouterUp => {
                    // Naik tanpa turun yang terlihat di rentang ini: anggap sudah down sejak awal rentang.
                    let start = down_at.take().unwrap_or(if first { from } else { e.event.at_ms });
                    down_ms += (e.event.at_ms - start).max(0);
                }
                _ => {}
            }
            first = false;
        }
        if let Some(d) = down_at {
            down_ms += (to - d).max(0);
        }
        let avail = (100.0 * (1.0 - down_ms as f64 / span as f64)).clamp(0.0, 100.0);
        let avail = (avail * 10.0).round() / 10.0;
        let mins = down_ms / 60_000;
        let mut row = json!({ "router": r.name, "status": r.status.as_str(), "last_poll": r.last_poll_at.map(fmt_utc), "error": r.error });
        if may_events {
            row["down_events"] = json!(downs);
            row["estimated_downtime_minutes"] = json!(mins);
            row["availability_percent"] = json!(avail);
        }
        rows.push(row);
        let (secondary, tone) = if may_events {
            (Some(format!("{} {avail}% · {downs}× down · {mins} {}", ctx.lang.t("tersedia", "available"), ctx.lang.t("mnt", "min"))), if downs == 0 && r.status.as_str() == "online" { AiTone::Good } else if r.status.as_str() == "online" { AiTone::Warn } else { AiTone::Bad })
        } else {
            (None, if r.status.as_str() == "online" { AiTone::Good } else { AiTone::Bad })
        };
        list.push(ListRow { primary: r.name.clone(), secondary, trailing: Some(r.status.as_str().into()), tone: Some(tone), to: Some(link("router", &[])) });
    }
    ToolOutput {
        for_model: json!({ "hours": hours, "routers": rows, "downtime_visible": may_events, "note": if may_events { "downtime dihitung dari event router_down dan router_up" } else { "peran penanya tidak boleh melihat event, jadi hanya status sekarang yang tersedia" } }).to_string(),
        widgets: vec![AiWidget::List { title: Some(ctx.lang.t("Kesehatan router", "Router health").into()), rows: list }],
        source: Some(format!("Router ({hours} {})", ctx.lang.t("jam", "h"))),
        step,
    }
}
