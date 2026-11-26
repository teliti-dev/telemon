//! Alat tata letak untuk AI (ADR 0027). `get_layout` hanya membaca. `propose_layout_change` **tidak menulis**:
//! ia memeriksa usulan terhadap tampilan yang dikirim klien dan izin penanya, lalu mengembalikan widget `Proposal`
//! untuk ditampilkan. Perubahan baru terjadi bila pengguna menekan Terapkan di UI, yang menyimpannya lewat API
//! tata letak dengan izin pengguna itu sendiri; AI tidak pernah menyentuh penyimpanan.

use std::collections::HashSet;

use serde_json::{Map, Value, json};
use telemon_core::auth::{Level, Resource};
use telemon_core::traffic::PPPOE_AGGREGATE;

use crate::tools::{Names, ToolCtx, ToolOutput, err, need};
use crate::types::{AiWidget, LayoutOp, UiContext};

/// Sama dengan batas di server tata letak.
const MAX_WIDGETS: usize = 60;
const MAX_CHANGES: usize = 10;
const RANGES: [&str; 5] = ["1h", "6h", "24h", "7d", "30d"];

/// Kunci pengaturan yang dikenal per jenis widget (cermin formulir pengaturan di UI).
fn allowed_keys(kind: &str) -> &'static [&'static str] {
    match kind {
        "bandwidth-chart" => &["router", "iface", "range"],
        "bandwidth-total" => &["range"],
        "bandwidth-per-user" => &["router", "range"],
        _ => &[],
    }
}

fn needs_router(kind: &str) -> bool {
    kind.starts_with("bandwidth-")
}

fn title_of<'a>(ui: &'a UiContext, kind: &'a str) -> &'a str {
    ui.catalog.iter().find(|t| t.kind == kind).map(|t| t.title.as_str()).unwrap_or(kind)
}

fn no_ui(step: String) -> ToolOutput {
    err(step, "tampilan Ringkasan tidak dikirim oleh klien ini; minta pengguna membuka halaman Ringkasan atau memperbarui aplikasi")
}

pub(crate) fn get_layout(ctx: &ToolCtx) -> ToolOutput {
    let step = ctx.lang.t("Membaca tata letak Ringkasan", "Reading the Overview layout").to_string();
    if let Err(e) = need(ctx, Resource::Ringkasan, "Ringkasan", &step) {
        return e;
    }
    let Some(ui) = ctx.ui else { return no_ui(step) };
    let routers: Vec<String> = if ctx.auth.authorize(ctx.caller, Resource::Router, Level::Read).is_ok() { Names::load(ctx).by_id.into_iter().map(|(_, n)| n).collect() } else { vec![] };
    let can_edit = ctx.auth.authorize(ctx.caller, Resource::Ringkasan, Level::Update).is_ok();
    let widgets: Vec<Value> = ui.widgets.iter().map(|w| json!({ "id": w.id, "type": w.kind, "title": title_of(ui, &w.kind), "config": w.config })).collect();
    let catalog: Vec<Value> = ui.catalog.iter().map(|t| json!({ "type": t.kind, "title": t.title, "description": t.description, "configurable": t.configurable })).collect();
    ToolOutput {
        for_model: json!({ "widgets": widgets, "available_types": catalog, "routers": routers, "asker_can_edit_layout": can_edit,
            "config_options": { "bandwidth-chart": "router (nama router atau 'all'), iface (nama antarmuka atau '@pppoe'), range (1h|6h|24h|7d|30d)", "bandwidth-total": "range", "bandwidth-per-user": "router (nama atau 'all'), range" } })
        .to_string(),
        // Tanpa kartu: daftarnya teknis (id dan jenis) dan hanya berguna bagi model; jawabannya berupa teks.
        widgets: vec![],
        source: Some(ctx.lang.t("Tata letak Ringkasan", "Overview layout").into()),
        step,
    }
}

/// Periksa dan lengkapi pengaturan satu widget. Nama router diganti id-nya. `base` = pengaturan yang sudah ada
/// (untuk `configure`), `fill_defaults` = isi nilai bawaan (untuk `add`).
fn config_for(ctx: &ToolCtx, names: &Names, kind: &str, given: &Map<String, Value>, base: Option<&Map<String, Value>>, fill_defaults: bool) -> Result<Map<String, Value>, String> {
    let keys = allowed_keys(kind);
    if keys.is_empty() {
        return if given.is_empty() { Ok(Map::new()) } else { Err(format!("widget '{kind}' tidak punya pengaturan")) };
    }
    if let Some(bad) = given.keys().find(|k| !keys.contains(&k.as_str())) {
        return Err(format!("pengaturan '{bad}' tidak dikenal untuk '{kind}'; yang ada: {}", keys.join(", ")));
    }
    let mut out = base.cloned().unwrap_or_default();
    for (k, v) in given {
        out.insert(k.clone(), v.clone());
    }
    let text = |m: &Map<String, Value>, k: &str| m.get(k).and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);
    if keys.contains(&"router") {
        let router = text(&out, "router").or_else(|| fill_defaults.then(|| "all".to_string())).unwrap_or_else(|| "all".into());
        let id = if router.eq_ignore_ascii_case("all") || router.eq_ignore_ascii_case("semua") {
            "all".to_string()
        } else {
            names.resolve(&router).ok_or_else(|| format!("router '{router}' tidak ditemukan; yang ada: {}", names.by_id.iter().map(|(_, n)| n.as_str()).take(20).collect::<Vec<_>>().join(", ")))?
        };
        out.insert("router".into(), json!(id));
    }
    if keys.contains(&"iface") {
        let router = out.get("router").and_then(Value::as_str).unwrap_or("all").to_string();
        let iface = text(&out, "iface").unwrap_or_else(|| PPPOE_AGGREGATE.to_string());
        if iface.len() > 64 {
            return Err("nama antarmuka terlalu panjang".into());
        }
        if router == "all" {
            if iface != PPPOE_AGGREGATE {
                return Err("untuk router 'all' antarmuka harus '@pppoe' (antarmuka fisik akan terhitung ganda); pilih satu router untuk antarmuka lain".into());
            }
        } else if iface != PPPOE_AGGREGATE {
            let known: Vec<String> = ctx.store.lock().expect("lock").list_traffic_ifaces(&router).unwrap_or_default().into_iter().map(|i| i.name).collect();
            if !known.iter().any(|k| k == &iface) {
                return Err(format!("antarmuka '{iface}' belum terbaca pada router itu; yang ada: @pppoe{}", known.iter().take(20).map(|k| format!(", {k}")).collect::<String>()));
            }
        }
        out.insert("iface".into(), json!(iface));
    }
    if keys.contains(&"range") {
        let range = text(&out, "range").unwrap_or_else(|| "6h".into());
        if !RANGES.contains(&range.as_str()) {
            return Err(format!("range '{range}' tidak valid; pilih: {}", RANGES.join(", ")));
        }
        out.insert("range".into(), json!(range));
    }
    Ok(out)
}

fn describe(ctx: &ToolCtx, names: &Names, ui: &UiContext, op: &LayoutOp) -> String {
    let detail = |cfg: &Map<String, Value>| -> String {
        let mut parts = vec![];
        if let Some(r) = cfg.get("router").and_then(Value::as_str) {
            parts.push(if r == "all" { ctx.lang.t("semua router", "all routers").to_string() } else { names.name(r) });
        }
        if let Some(i) = cfg.get("iface").and_then(Value::as_str) {
            parts.push(if i == PPPOE_AGGREGATE { ctx.lang.t("semua sesi PPPoE", "all PPPoE sessions").to_string() } else { i.to_string() });
        }
        if let Some(r) = cfg.get("range").and_then(Value::as_str) {
            parts.push(r.to_string());
        }
        if parts.is_empty() { String::new() } else { format!(" ({})", parts.join(", ")) }
    };
    match op {
        LayoutOp::Add { widget, config } => format!("{} {}{}", ctx.lang.t("Tambah widget", "Add widget"), title_of(ui, widget), detail(config)),
        LayoutOp::Remove { id } => {
            let kind = ui.widgets.iter().find(|w| &w.id == id).map(|w| w.kind.as_str()).unwrap_or("");
            format!("{} {}", ctx.lang.t("Hapus widget", "Remove widget"), title_of(ui, kind))
        }
        LayoutOp::Configure { id, config } => {
            let kind = ui.widgets.iter().find(|w| &w.id == id).map(|w| w.kind.as_str()).unwrap_or("");
            format!("{} {}{}", ctx.lang.t("Ubah pengaturan widget", "Change settings of widget"), title_of(ui, kind), detail(config))
        }
    }
}

pub(crate) fn propose_layout_change(ctx: &ToolCtx, a: &Value) -> ToolOutput {
    let step = ctx.lang.t("Menyusun usulan perubahan widget", "Drafting a widget change proposal").to_string();
    if ctx.auth.authorize(ctx.caller, Resource::Ringkasan, Level::Update).is_err() {
        return err(format!("{step} — {}", ctx.lang.t("tidak ada izin", "no permission")), "peran penanya tidak boleh mengubah tata letak Ringkasan (izin Ubah Ringkasan); beri tahu pengguna dan jangan mengusulkan apa pun");
    }
    let Some(ui) = ctx.ui else { return no_ui(step) };
    let Some(changes) = a.get("changes").and_then(Value::as_array).filter(|c| !c.is_empty() && c.len() <= MAX_CHANGES) else {
        return err(step, format!("changes harus berisi 1 sampai {MAX_CHANGES} perubahan"));
    };
    let names = Names::load(ctx);
    let router_ok = ctx.auth.authorize(ctx.caller, Resource::Router, Level::Read).is_ok();
    let mut ops: Vec<LayoutOp> = vec![];
    let mut removed: HashSet<String> = HashSet::new();
    let mut touched: HashSet<String> = HashSet::new();
    let mut count = ui.widgets.len();
    let empty = Map::new();
    for (i, c) in changes.iter().enumerate() {
        let at = |m: &str| format!("perubahan #{}: {m}", i + 1);
        let cfg = c.get("config").and_then(Value::as_object).unwrap_or(&empty);
        match c.get("op").and_then(Value::as_str) {
            Some("add") => {
                let Some(kind) = c.get("type").and_then(Value::as_str) else { return err(step, at("add memerlukan 'type'")) };
                if !ui.catalog.iter().any(|t| t.kind == kind) {
                    return err(step, at(&format!("jenis widget '{kind}' tidak ada; yang tersedia: {}", ui.catalog.iter().map(|t| t.kind.as_str()).collect::<Vec<_>>().join(", "))));
                }
                if needs_router(kind) && !router_ok {
                    return err(step, at("peran penanya tidak boleh melihat Router, jadi widget bandwidth tidak bisa diusulkan"));
                }
                let config = match config_for(ctx, &names, kind, cfg, None, true) {
                    Ok(m) => m,
                    Err(m) => return err(step, at(&m)),
                };
                count += 1;
                ops.push(LayoutOp::Add { widget: kind.to_string(), config });
            }
            Some(op @ ("remove" | "configure")) => {
                let Some(id) = c.get("id").and_then(Value::as_str) else { return err(step, at(&format!("{op} memerlukan 'id' dari get_layout"))) };
                let Some(w) = ui.widgets.iter().find(|w| w.id == id) else { return err(step, at(&format!("widget dengan id '{id}' tidak ada; panggil get_layout"))) };
                if removed.contains(id) || (op == "remove" && touched.contains(id)) {
                    return err(step, at(&format!("widget '{id}' sudah dipakai oleh perubahan lain dalam usulan ini")));
                }
                if op == "remove" {
                    removed.insert(id.to_string());
                    count = count.saturating_sub(1);
                    ops.push(LayoutOp::Remove { id: id.to_string() });
                } else {
                    if allowed_keys(&w.kind).is_empty() {
                        return err(step, at(&format!("widget '{}' tidak punya pengaturan", w.kind)));
                    }
                    if needs_router(&w.kind) && !router_ok {
                        return err(step, at("peran penanya tidak boleh melihat Router, jadi widget bandwidth tidak bisa diubah"));
                    }
                    let config = match config_for(ctx, &names, &w.kind, cfg, w.config.as_ref(), false) {
                        Ok(m) => m,
                        Err(m) => return err(step, at(&m)),
                    };
                    touched.insert(id.to_string());
                    ops.push(LayoutOp::Configure { id: id.to_string(), config });
                }
            }
            _ => return err(step, at("op harus add, remove, atau configure")),
        }
    }
    if count > MAX_WIDGETS {
        return err(step, format!("hasilnya melebihi batas {MAX_WIDGETS} widget"));
    }
    let lines: Vec<String> = ops.iter().map(|o| describe(ctx, &names, ui, o)).collect();
    ToolOutput {
        for_model: json!({ "status": "proposal_ready", "applied": false, "changes": lines,
            "note": "Belum ada yang berubah. Usulan tampil di chat dengan tombol Terapkan; ceritakan apa yang diusulkan dan minta pengguna menekan Terapkan bila setuju." })
        .to_string(),
        widgets: vec![AiWidget::Proposal { ops, state: None }],
        source: Some(ctx.lang.t("Tata letak Ringkasan", "Overview layout").into()),
        step: format!("{step} — {}", lines.len()),
    }
}
