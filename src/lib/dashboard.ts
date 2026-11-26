import type { DashboardWidget } from "@/lib/types"

/**
 * Grid Ringkasan: 20 kolom supaya lima kartu angka dan pembagian 13/7 muat tanpa pecahan. Baris setinggi 4 px tanpa
 * jarak vertikal grid; jarak antar widget (`WIDGET_GAP`) dibuat sebagai padding bawah di dalam tiap widget. Dengan
 * langkah setipis itu widget yang tingginya mengikuti isi (kartu router) tidak menyisakan ruang kosong.
 * Tinggi widget yang terlihat = 4h - WIDGET_GAP piksel.
 */
export const GRID_COLS = 20
export const ROW_HEIGHT = 4
export const GRID_MARGIN: [number, number] = [8, 0]
export const WIDGET_GAP = 8
/** Sama dengan batas di server. */
export const MAX_WIDGETS = 60

/**
 * Susunan bawaan = tampilan Ringkasan sebelum ada widget: lima kartu angka, kartu router,
 * lalu event terbaru di samping daftar yang sering gagal auth. Dipakai saat belum ada tata letak tersimpan.
 */
export const DEFAULT_WIDGETS: DashboardWidget[] = [
  { id: "default-routers-online", type: "stat-routers-online", x: 0, y: 0, w: 4, h: 24 },
  { id: "default-users-active", type: "stat-users-active", x: 4, y: 0, w: 4, h: 24 },
  { id: "default-users-inactive", type: "stat-users-inactive", x: 8, y: 0, w: 4, h: 24 },
  { id: "default-users-disabled", type: "stat-users-disabled", x: 12, y: 0, w: 4, h: 24 },
  { id: "default-auth-fails", type: "stat-auth-fails", x: 16, y: 0, w: 4, h: 24 },
  { id: "default-router-cards", type: "router-cards", x: 0, y: 24, w: 20, h: 48 },
  { id: "default-recent-events", type: "recent-events", x: 0, y: 72, w: 13, h: 108 },
  { id: "default-top-auth-fails", type: "top-auth-fails", x: 13, y: 72, w: 7, h: 108 },
]

/** Jumlah baris grid yang cukup untuk tinggi isi `px` (piksel). */
export function rowsForHeight(px: number) {
  return Math.max(1, Math.ceil((px + WIDGET_GAP) / ROW_HEIGHT))
}

export function newWidgetId(type: string) {
  const rand = typeof crypto !== "undefined" && "randomUUID" in crypto ? crypto.randomUUID().slice(0, 8) : Math.random().toString(36).slice(2, 10)
  return `${type}-${rand}`.slice(0, 40)
}

/** Baris pertama di bawah semua widget (untuk widget yang baru ditambahkan). */
export function nextY(widgets: DashboardWidget[]) {
  return widgets.reduce((m, w) => Math.max(m, w.y + w.h), 0)
}

/** Urutan baca (atas ke bawah, kiri ke kanan), dipakai tampilan HP yang satu kolom. */
export function readingOrder<T extends { x: number; y: number }>(items: T[]) {
  return [...items].sort((a, b) => a.y - b.y || a.x - b.x)
}

/** Dua tata letak sama bila setiap widget punya jenis dan kotak yang sama (urutan dan pengaturan tidak dihitung). */
export function sameLayout(a: DashboardWidget[], b: DashboardWidget[]) {
  const key = (w: DashboardWidget) => `${w.id}|${w.type}|${w.x}|${w.y}|${w.w}|${w.h}`
  const ka = a.map(key).sort()
  const kb = b.map(key).sort()
  return ka.length === kb.length && ka.every((k, i) => k === kb[i])
}

const CHANGED = "telemon:dashboard-changed"

/** Beritahu halaman Ringkasan yang terbuka bahwa tata letak berubah dari tempat lain (mis. usulan AI diterapkan). */
export function notifyDashboardChanged() {
  try {
    window.dispatchEvent(new Event(CHANGED))
  } catch {
    /* bukan di browser */
  }
}

export function onDashboardChanged(cb: () => void) {
  window.addEventListener(CHANGED, cb)
  return () => window.removeEventListener(CHANGED, cb)
}
