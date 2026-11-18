import { locale, t } from "@/lib/i18n"

const cache = new Map<string, Intl.DateTimeFormat | Intl.NumberFormat>()

function dtf(key: string, opts: Intl.DateTimeFormatOptions) {
  const loc = locale()
  const k = `${key}:${loc}`
  let f = cache.get(k) as Intl.DateTimeFormat | undefined
  if (!f) {
    f = new Intl.DateTimeFormat(loc, opts)
    cache.set(k, f)
  }
  return f
}

function nf(key: string, opts?: Intl.NumberFormatOptions) {
  const loc = locale()
  const k = `${key}:${loc}`
  let f = cache.get(k) as Intl.NumberFormat | undefined
  if (!f) {
    f = new Intl.NumberFormat(loc, opts)
    cache.set(k, f)
  }
  return f
}

export function formatTime(ts: number) {
  return dtf("time", { hour: "2-digit", minute: "2-digit", second: "2-digit" }).format(ts)
}

export function formatDateTime(ts: number) {
  return dtf("datetime", {
    day: "numeric",
    month: "short",
    hour: "2-digit",
    minute: "2-digit",
  }).format(ts)
}

export function formatNumber(n: number) {
  return nf("num").format(n)
}

/** "baru saja", "5 mnt lalu", "3 jam lalu", lalu tanggal. */
export function formatRelative(ts: number, now: number) {
  const s = Math.max(0, Math.round((now - ts) / 1000))
  if (s < 45) return t("baru saja")
  const m = Math.round(s / 60)
  if (m < 60) return t("{n} mnt lalu", { n: m })
  const h = Math.round(m / 60)
  if (h < 24) return t("{n} jam lalu", { n: h })
  return formatDateTime(ts)
}

/** Durasi ringkas: "2h 4j", "4j 12m", "12m", "40d". */
export function formatDuration(ms: number) {
  const s = Math.max(0, Math.floor(ms / 1000))
  const d = Math.floor(s / 86400)
  const h = Math.floor((s % 86400) / 3600)
  const m = Math.floor((s % 3600) / 60)
  if (d > 0) return t("{d}h {h}j", { d, h })
  if (h > 0) return t("{h}j {m}m", { h, m })
  if (m > 0) return t("{m}m", { m })
  return t("{s}d", { s })
}

/** Ukuran berkas: "2,4 MB". */
export function formatBytes(n: number) {
  const units = ["B", "KB", "MB", "GB", "TB"]
  let v = n
  let i = 0
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024
    i++
  }
  return `${nf(i === 0 ? "bytes0" : "bytes1", { maximumFractionDigits: i === 0 ? 0 : 1 }).format(v)} ${units[i]}`
}

/** "1,2 jt", "38 rb": ringkas untuk jumlah token. */
export function formatCompact(n: number) {
  return nf("compact", { notation: "compact", maximumFractionDigits: 1 }).format(n)
}

export function formatUsd(n: number) {
  return nf("usd", { style: "currency", currency: "USD", minimumFractionDigits: 2, maximumFractionDigits: 2 }).format(n)
}
