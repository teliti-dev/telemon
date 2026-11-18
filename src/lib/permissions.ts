import type { Access } from "./access"
import type { Page } from "./route"
import type { Level, Permissions, Resource } from "./types"

/** Menu yang izinnya bisa diatur per peran, berurutan seperti di menu utama. */
export const RESOURCES: { key: Resource; label: string; levels: Level[]; hint: string }[] = [
  { key: "ringkasan", label: "Ringkasan", levels: ["none", "read"], hint: "Angka dan kartu ringkasan" },
  { key: "user", label: "Users", levels: ["none", "read", "update"], hint: "Ubah: tambah, edit, hapus (soft), pulihkan, catatan" },
  { key: "profile", label: "Profile", levels: ["none", "read", "update"], hint: "Ubah: tambah, edit, hapus (soft), pulihkan" },
  { key: "event", label: "Event", levels: ["none", "read"], hint: "Feed dan riwayat event PPPoE" },
  { key: "router", label: "Router", levels: ["none", "read", "update"], hint: "Ubah: import dari Mikrotik. Tambah/hapus router hanya dari desktop" },
]

export const LEVEL_LABEL: Record<Level, string> = {
  none: "Tanpa akses",
  read: "Hanya baca",
  update: "Ubah",
}

/** Label pendek untuk tabel. */
export const LEVEL_SHORT: Record<Level, string> = { none: "—", read: "Baca", update: "Ubah" }

export function hasUpdate(p: Permissions) {
  return Object.values(p).includes("update")
}

export function canRead(p: Permissions | undefined, r: Resource) {
  return !p || p[r] !== "none"
}

const DESKTOP_ONLY: Page[] = ["berbagi", "admin", "pengaturan"]
const PAGE_RESOURCE: Partial<Record<Page, Resource>> = {
  ringkasan: "ringkasan",
  user: "user",
  profile: "profile",
  event: "event",
  router: "router",
}

/** Alasan halaman tidak boleh dibuka; null bila boleh. */
export function pageDenied(page: Page, access: Access, permissions?: Permissions): string | null {
  if (access === "desktop") return page === "akun" ? "Halaman Akun hanya untuk akses lewat browser." : null
  if (DESKTOP_ONLY.includes(page)) return "Halaman ini hanya ada di aplikasi desktop."
  const r = PAGE_RESOURCE[page]
  if (r && (!permissions || permissions[r] === "none")) return "Peran Anda tidak punya akses ke halaman ini."
  return null
}
