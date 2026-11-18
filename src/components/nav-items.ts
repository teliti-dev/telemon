import {
  ActivityIcon,
  GaugeIcon,
  RadioTowerIcon,
  Share2Icon,
  UsersIcon,
  type LucideIcon,
} from "lucide-react"

import type { Page } from "@/lib/route"

/** Lima menu utama; dipakai sidebar (desktop) dan menu bawah (HP). Profile ada di dalam Users. */
export const NAV: { page: Page; label: string; icon: LucideIcon }[] = [
  { page: "ringkasan", label: "Ringkasan", icon: GaugeIcon },
  { page: "user", label: "Users", icon: UsersIcon },
  { page: "event", label: "Event", icon: ActivityIcon },
  { page: "router", label: "Router", icon: RadioTowerIcon },
  { page: "berbagi", label: "Berbagi", icon: Share2Icon },
]

export function isNavActive(item: Page, current: Page) {
  return item === current || (item === "user" && current === "profile")
}
