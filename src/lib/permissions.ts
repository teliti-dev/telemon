import type { Access } from "./access"
import { t, tk } from "@/lib/i18n"
import type { Page } from "./route"
import type { Level, Permissions, Resource } from "./types"

/** Menu yang izinnya bisa diatur per peran, berurutan seperti di menu utama. */
export const RESOURCES: { key: Resource; label: string; levels: Level[]; hint: string }[] = [
  { key: "ringkasan", label: tk("Ringkasan"), levels: ["none", "read"], hint: tk("Angka dan kartu ringkasan") },
  { key: "user", label: tk("Users"), levels: ["none", "read", "update"], hint: tk("Ubah: tambah, edit, hapus (soft), pulihkan, catatan") },
  { key: "profile", label: tk("Profil PPP"), levels: ["none", "read", "update"], hint: tk("Profil PPP di router (rate limit, pool). Ubah: tambah, edit, hapus (soft), pulihkan") },
  { key: "event", label: tk("Event"), levels: ["none", "read"], hint: tk("Feed dan riwayat event PPPoE") },
  { key: "router", label: tk("Router"), levels: ["none", "read", "update"], hint: tk("Ubah: tambah, edit, hapus router dan import dari Mikrotik") },
  { key: "admin", label: tk("Admin"), levels: ["none", "read", "update"], hint: tk("Akun dan peran. Ubah: tambah, edit, hapus; tidak bisa memberi izin melebihi milik sendiri") },
  { key: "pengaturan", label: tk("Pengaturan"), levels: ["none", "read", "update"], hint: tk("Retensi, polling, sesi. Izin tulis lewat publik hanya dari desktop") },
  { key: "ai", label: tk("AI chat"), levels: ["none", "read"], hint: tk("Tanya AI soal data yang boleh dilihat peran ini (hanya membaca)") },
]

export const LEVEL_LABEL: Record<Level, string> = {
  none: tk("Tanpa akses"),
  read: tk("Lihat"),
  update: tk("Ubah"),
}

/** Label pendek untuk tabel. */
export const LEVEL_SHORT: Record<Level, string> = { none: "—", read: tk("Lihat"), update: tk("Ubah") }

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
  if (access === "desktop") return page === "akun" ? t("Halaman Akun hanya untuk akses lewat browser.") : null
  if (DESKTOP_ONLY.includes(page)) return t("Halaman ini hanya ada di aplikasi desktop.")
  const r = PAGE_RESOURCE[page]
  if (r && (!permissions || permissions[r] === "none")) return t("Peran Anda tidak punya akses ke halaman ini.")
  return null
}
