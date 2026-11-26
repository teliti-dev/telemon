import { WIDGET_BY_TYPE, WIDGETS } from "@/components/dashboard/registry"
import type { TelemonApi } from "@/lib/data/api"
import { DEFAULT_WIDGETS, MAX_WIDGETS, newWidgetId, nextY } from "@/lib/dashboard"
import { t } from "@/lib/i18n"
import { PPPOE, RANGES } from "@/lib/traffic"
import type { AiUiContext, DashboardWidget, LayoutOp, RouterInfo } from "@/lib/types"

type Cfg = Record<string, unknown>

/** Periksa pengaturan satu widget; mengembalikan pengaturan bersih atau alasan penolakan. */
function cleanConfig(type: string, cfg: Cfg, routerIds: string[]): { ok: Cfg } | { error: string } {
  const def = WIDGET_BY_TYPE.get(type)
  const keys = def?.configKeys ?? []
  const bad = Object.keys(cfg).find((k) => !keys.includes(k))
  if (bad) return { error: t("Pengaturan {key} tidak dikenal untuk widget ini", { key: bad }) }
  const out: Cfg = { ...cfg }
  if ("router" in out && !(out.router === "all" || (typeof out.router === "string" && routerIds.includes(out.router)))) return { error: t("Router tidak ditemukan") }
  if ("range" in out && !RANGES.includes(out.range as never)) return { error: t("Rentang tidak valid") }
  if ("iface" in out) {
    if (typeof out.iface !== "string" || out.iface.length === 0 || out.iface.length > 64) return { error: t("Antarmuka tidak valid") }
    if ((out.router ?? "all") === "all" && out.iface !== PPPOE) return { error: t("Total semua router hanya untuk antarmuka @pppoe") }
  }
  return { ok: out }
}

/**
 * Terapkan usulan perubahan ke tata letak. Semua atau tidak sama sekali: bila ada satu perubahan yang tidak sah
 * (jenis tak dikenal, id sudah hilang, pengaturan salah, melebihi batas), tata letak dikembalikan apa adanya.
 * UI memeriksa ulang walau server sudah memeriksa: yang menyimpan tata letak adalah UI atas nama penggunanya.
 */
export function applyLayoutOps(widgets: DashboardWidget[], ops: LayoutOp[], routerIds: string[]): { widgets: DashboardWidget[]; problems: string[] } {
  let cur = [...widgets]
  const problems: string[] = []
  for (const op of ops) {
    if (op.op === "add") {
      const def = WIDGET_BY_TYPE.get(op.type)
      if (!def) {
        problems.push(t("Jenis widget {type} tidak dikenal", { type: op.type }))
        continue
      }
      const c = cleanConfig(op.type, { ...(def.defaultConfig ?? {}), ...(op.config ?? {}) }, routerIds)
      if ("error" in c) {
        problems.push(c.error)
        continue
      }
      cur = [...cur, { id: newWidgetId(op.type), type: op.type, x: 0, y: nextY(cur), w: def.w, h: def.h, ...(def.configKeys ? { config: c.ok } : {}) }]
    } else if (op.op === "remove") {
      if (!cur.some((w) => w.id === op.id)) problems.push(t("Widget yang akan dihapus sudah tidak ada"))
      cur = cur.filter((w) => w.id !== op.id)
    } else {
      const w = cur.find((x) => x.id === op.id)
      if (!w) {
        problems.push(t("Widget yang akan diubah sudah tidak ada"))
        continue
      }
      const c = cleanConfig(w.type, { ...(w.config ?? {}), ...op.config }, routerIds)
      if ("error" in c) {
        problems.push(c.error)
        continue
      }
      cur = cur.map((x) => (x.id === op.id ? { ...x, config: c.ok } : x))
    }
  }
  if (cur.length > MAX_WIDGETS) problems.push(t("Jumlah widget melebihi batas {n}", { n: MAX_WIDGETS }))
  return problems.length ? { widgets, problems } : { widgets: cur, problems }
}

/** Kalimat ringkas per perubahan untuk kartu usulan. */
export function describeOps(ops: LayoutOp[], widgets: DashboardWidget[], routers: RouterInfo[]): string[] {
  const title = (type: string) => {
    const d = WIDGET_BY_TYPE.get(type)
    return d ? t(d.title) /* i18n-ignore: kunci tk */ : type
  }
  const detail = (cfg?: Cfg) => {
    if (!cfg) return ""
    const parts: string[] = []
    if (typeof cfg.router === "string") parts.push(cfg.router === "all" ? t("semua router") : (routers.find((r) => r.id === cfg.router)?.name ?? cfg.router))
    if (typeof cfg.iface === "string") parts.push(cfg.iface === PPPOE ? t("semua sesi PPPoE") : cfg.iface)
    if (typeof cfg.range === "string") parts.push(cfg.range)
    return parts.length ? ` (${parts.join(", ")})` : ""
  }
  return ops.map((op) => {
    if (op.op === "add") return `${t("Tambah widget")} ${title(op.type)}${detail(op.config)}`
    const w = widgets.find((x) => x.id === op.id)
    const name = w ? title(w.type) : op.id
    return op.op === "remove" ? `${t("Hapus widget")} ${name}` : `${t("Ubah pengaturan widget")} ${name}${detail(op.config)}`
  })
}

export function buildUiContext(widgets: DashboardWidget[]): AiUiContext {
  return {
    widgets: widgets.map((w) => ({ id: w.id, type: w.type, ...(w.config ? { config: w.config } : {}) })),
    // Judul dan deskripsi dikirim dalam bahasa UI yang aktif supaya AI menjawab dengan istilah yang dilihat pengguna.
    catalog: WIDGETS.map((d) => ({ type: d.type, title: t(d.title) /* i18n-ignore: kunci tk */, description: t(d.description) /* i18n-ignore: kunci tk */, configurable: !!d.Config })),
  }
}

/** Tampilan Ringkasan yang berlaku sekarang untuk dikirim ke AI; kosong bila tidak bisa dibaca (mis. tanpa izin Ringkasan). */
export async function currentUiContext(api: TelemonApi): Promise<AiUiContext | undefined> {
  try {
    const l = await api.getDashboard()
    return buildUiContext((l ? l.widgets : DEFAULT_WIDGETS).filter((w) => WIDGET_BY_TYPE.has(w.type)))
  } catch {
    return undefined
  }
}
