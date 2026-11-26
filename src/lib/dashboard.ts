import type { DashboardWidget } from "@/lib/types"

/** Grid Ringkasan: 20 kolom supaya lima kartu angka dan pembagian 13/7 muat tanpa pecahan. */
export const GRID_COLS = 20
export const ROW_HEIGHT = 40
export const GRID_MARGIN: [number, number] = [8, 8]
/** Sama dengan batas di server. */
export const MAX_WIDGETS = 60

/**
 * Susunan bawaan = tampilan Ringkasan sebelum ada widget: lima kartu angka, kartu router,
 * lalu event terbaru di samping daftar yang sering gagal auth. Dipakai saat belum ada tata letak tersimpan.
 */
export const DEFAULT_WIDGETS: DashboardWidget[] = [
  { id: "default-routers-online", type: "stat-routers-online", x: 0, y: 0, w: 4, h: 2 },
  { id: "default-users-active", type: "stat-users-active", x: 4, y: 0, w: 4, h: 2 },
  { id: "default-users-inactive", type: "stat-users-inactive", x: 8, y: 0, w: 4, h: 2 },
  { id: "default-users-disabled", type: "stat-users-disabled", x: 12, y: 0, w: 4, h: 2 },
  { id: "default-auth-fails", type: "stat-auth-fails", x: 16, y: 0, w: 4, h: 2 },
  { id: "default-router-cards", type: "router-cards", x: 0, y: 2, w: 20, h: 4 },
  { id: "default-recent-events", type: "recent-events", x: 0, y: 6, w: 13, h: 9 },
  { id: "default-top-auth-fails", type: "top-auth-fails", x: 13, y: 6, w: 7, h: 9 },
]

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
