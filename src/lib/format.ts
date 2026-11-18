const timeFmt = new Intl.DateTimeFormat("id-ID", {
  hour: "2-digit",
  minute: "2-digit",
  second: "2-digit",
})
const dateTimeFmt = new Intl.DateTimeFormat("id-ID", {
  day: "numeric",
  month: "short",
  hour: "2-digit",
  minute: "2-digit",
})
const numberFmt = new Intl.NumberFormat("id-ID")

export function formatTime(ts: number) {
  return timeFmt.format(ts)
}

export function formatDateTime(ts: number) {
  return dateTimeFmt.format(ts)
}

export function formatNumber(n: number) {
  return numberFmt.format(n)
}

/** "baru saja", "5 mnt lalu", "3 jam lalu", lalu tanggal. */
export function formatRelative(ts: number, now: number) {
  const s = Math.max(0, Math.round((now - ts) / 1000))
  if (s < 45) return "baru saja"
  const m = Math.round(s / 60)
  if (m < 60) return `${m} mnt lalu`
  const h = Math.round(m / 60)
  if (h < 24) return `${h} jam lalu`
  return formatDateTime(ts)
}

/** Durasi ringkas: "2h 4j", "4j 12m", "12m", "40d". */
export function formatDuration(ms: number) {
  const s = Math.max(0, Math.floor(ms / 1000))
  const d = Math.floor(s / 86400)
  const h = Math.floor((s % 86400) / 3600)
  const m = Math.floor((s % 3600) / 60)
  if (d > 0) return `${d}h ${h}j`
  if (h > 0) return `${h}j ${m}m`
  if (m > 0) return `${m}m`
  return `${s}d`
}
