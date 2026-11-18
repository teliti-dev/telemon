import { useNow } from "@/hooks/use-now"
import type { Page } from "@/lib/route"
import { useStore } from "@/lib/store-context"

/** Angka di menu: gagal auth 1 jam terakhir (Event) dan router bermasalah (Router). */
export function useNavBadges(): Partial<Record<Page, number>> {
  const { routers, events } = useStore()
  const now = useNow(30_000)
  return {
    event: events.filter((e) => e.type === "auth_failed" && now - e.at < 3600_000).length,
    router: routers.filter((r) => r.status !== "online").length,
  }
}
