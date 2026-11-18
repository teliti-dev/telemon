import {
  ActivityIcon,
  GaugeIcon,
  RadioTowerIcon,
  SettingsIcon,
  Share2Icon,
  UserCogIcon,
  UsersIcon,
  type LucideIcon,
} from "lucide-react"

import type { Access } from "@/lib/access"
import type { Page } from "@/lib/route"
import type { Permissions, Resource } from "@/lib/types"

export interface NavItem {
  page: Page
  label: string
  icon: LucideIcon
  /** Izin yang menentukan menu tampil untuk akun jarak jauh (cukup salah satunya bukan "Tanpa akses"). */
  resources?: Resource[]
  /** Hanya di aplikasi desktop (kelola akses, akun, peran, pengaturan). */
  desktopOnly?: boolean
  /** Rata kanan di menu atas. */
  end?: boolean
  /** Masuk "Lainnya" di menu bawah HP. */
  more?: boolean
}

/** Menu utama; dipakai menu atas (desktop) dan menu bawah (HP). Profil PPP ada di dalam Users. */
export const NAV: NavItem[] = [
  { page: "ringkasan", label: "Ringkasan", icon: GaugeIcon, resources: ["ringkasan"] },
  { page: "user", label: "Users", icon: UsersIcon, resources: ["user", "profile"] },
  { page: "event", label: "Event", icon: ActivityIcon, resources: ["event"] },
  { page: "router", label: "Router", icon: RadioTowerIcon, resources: ["router"] },
  { page: "berbagi", label: "Berbagi", icon: Share2Icon, desktopOnly: true, end: true, more: true },
  { page: "admin", label: "Admin", icon: UserCogIcon, resources: ["admin"], end: true, more: true },
  { page: "pengaturan", label: "Pengaturan", icon: SettingsIcon, resources: ["pengaturan"], end: true, more: true },
]

/** Menu yang tampil: desktop semua; jarak jauh hanya yang izinnya bukan "Tanpa akses". */
export function navFor(access: Access, permissions?: Permissions) {
  return NAV.filter((i) => {
    if (access === "desktop") return true
    if (i.desktopOnly) return false
    return !i.resources || i.resources.some((r) => permissions?.[r] && permissions[r] !== "none")
  }).map((i) => (i.page === "user" && access === "remote" && permissions?.user === "none" ? { ...i, page: "profile" as Page } : i))
}

export function isNavActive(item: Page, current: Page) {
  return item === current || (item === "user" && current === "profile")
}
