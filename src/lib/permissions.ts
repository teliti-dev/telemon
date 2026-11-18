import type { Access } from "./access"
import type { Page } from "./route"
import type { Level, Permissions, Resource } from "./types"

/** Menu yang izinnya bisa diatur per peran, berurutan seperti di menu utama. */
export const RESOURCES: { key: Resource; label: string; levels: Level[]; hint: string }[] = [
  { key: "ringkasan", label: "Ringkasan", levels: ["none", "read"], hint: "Angka dan kartu ringkasan" },
  { key: "user", label: "Users", levels: ["none", "read", "update"], hint: "Ubah: tambah, edit, hapus (soft), pulihkan, catatan" },
  { key: "profile", label: "Profil PPP", levels: ["none", "read", "update"], hint: "Profil PPP di router (rate limit, pool). Ubah: tambah, edit, hapus (soft), pulihkan" },
  { key: "event", label: "Event", levels: ["none", "read"], hint: "Feed dan riwayat event PPPoE" },
  { key: "router", label: "Router", levels: ["none", "read", "update"], hint: "Ubah: tambah, edit, hapus router dan import dari Mikrotik" },
  { key: "admin", label: "Admin", levels: ["none", "read", "update"], hint: "Akun dan peran. Ubah: tambah, edit, hapus; tidak bisa memberi izin melebihi milik sendiri" },
  { key: "pengaturan", label: "Pengaturan", levels: ["none", "read", "update"], hint: "Retensi, polling, sesi. Izin tulis lewat publik hanya dari desktop" },
]

export const LEVEL_LABEL: Record<Level, string> = {
  none: "Tanpa akses",
  read: "Lihat",
  update: "Ubah",
}

/** Label pendek untuk tabel. */
export const LEVEL_SHORT: Record<Level, string> = { none: "—", read: "Lihat", update: "Ubah" }

export function hasUpdate(p: Permissions) {
  return Object.values(p).includes("update")
}

export function canRead(p: Permissions | undefined, r: Resource) {
  return !p || p[r] !== "none"
}

const DESKTOP_ONLY: Page[] = ["berbagi"]
const PAGE_RESOURCE: Partial<Record<Page, Resource>> = {
  ringkasan: "ringkasan",
  user: "user",
  profile: "profile",
  event: "event",
  router: "router",
  admin: "admin",
  pengaturan: "pengaturan",
}

/** Alasan halaman tidak boleh dibuka; null bila boleh. */
export function pageDenied(page: Page, access: Access, permissions?: Permissions): string | null {
  if (access === "desktop") return page === "akun" ? "Halaman Akun hanya untuk akses lewat browser." : null
  if (DESKTOP_ONLY.includes(page)) return "Halaman ini hanya ada di aplikasi desktop."
  const r = PAGE_RESOURCE[page]
  if (r && (!permissions || permissions[r] === "none")) return "Peran Anda tidak punya akses ke halaman ini."
  return null
}
