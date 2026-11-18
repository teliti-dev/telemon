import {
  ActivityIcon,
  GaugeIcon,
  RadioTowerIcon,
  Share2Icon,
  UsersIcon,
  type LucideIcon,
} from "lucide-react"

import type { Access } from "@/lib/access"
import type { Page } from "@/lib/route"

/** Lima menu utama; dipakai menu atas (desktop) dan menu bawah (HP). `end`: rata kanan di menu atas. Profile ada di dalam Users. */
export const NAV: { page: Page; label: string; icon: LucideIcon; desktopOnly?: boolean; end?: boolean }[] = [
  { page: "ringkasan", label: "Ringkasan", icon: GaugeIcon },
  { page: "user", label: "Users", icon: UsersIcon },
  { page: "event", label: "Event", icon: ActivityIcon },
  { page: "router", label: "Router", icon: RadioTowerIcon },
  { page: "berbagi", label: "Berbagi", icon: Share2Icon, desktopOnly: true, end: true },
]

/** Menu yang tampil: Berbagi (kelola akses dan akun) hanya di aplikasi desktop. */
export function navFor(access: Access) {
  return NAV.filter((i) => access === "desktop" || !i.desktopOnly)
}

export function isNavActive(item: Page, current: Page) {
  return item === current || (item === "user" && current === "profile")
}
