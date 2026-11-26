import type * as React from "react"
import { CheckCircle2Icon, CircleAlertIcon, GlobeIcon, WifiIcon } from "lucide-react"

import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover"
import { useNow } from "@/hooks/use-now"
import { APP_VERSION } from "@/lib/app-info"
import { formatRelative } from "@/lib/format"
import { useT } from "@/lib/i18n"
import { href } from "@/lib/route"
import { useStore } from "@/lib/store-context"
import { cn } from "@/lib/utils"

function Row({ k, v }: { k: string; v: React.ReactNode }) {
  return (
    <div className="flex items-baseline justify-between gap-3 py-1 text-sm">
      <dt className="text-muted-foreground">{k}</dt>
      <dd className="min-w-0 truncate text-right tabular-nums">{v}</dd>
    </div>
  )
}

/**
 * Indikator Live: satu titik berwarna di samping logo (hijau berdenyut = menerima event live, abu = memuat, merah = gagal).
 * Diklik membuka popup status: koneksi, router, event terakhir, dan keadaan berbagi (LAN/Publik) di aplikasi desktop.
 */
export function LiveIndicator() {
  const t = useT()
  const { status, error, routers, events, liveUsers, share, access } = useStore()
  const now = useNow(30_000)
  const online = routers.filter((r) => r.status === "online").length
  const last = events.reduce((m, e) => Math.max(m, e.at), 0)
  const active = liveUsers.filter((u) => u.status === "active").length
  const label = status === "ready" ? t("Menerima event live") : status === "error" ? t("Gagal memuat data") : t("Memuat")
  const dot = status === "ready" ? "bg-good" : status === "error" ? "bg-critical" : "bg-muted-foreground/50"

  return (
    <Popover>
      <PopoverTrigger asChild>
        <button
          type="button"
          aria-label={`${t("Status koneksi")}: ${label}`}
          title={label}
          className="relative flex size-6 shrink-0 items-center justify-center rounded-full outline-none focus-visible:ring-3 focus-visible:ring-ring/50"
        >
          <span className="relative flex size-2.5">
            {status === "ready" && <span className="absolute inline-flex size-full animate-ping rounded-full bg-good opacity-60 motion-reduce:hidden" />}
            <span className={cn("relative inline-flex size-2.5 rounded-full", dot)} />
          </span>
        </button>
      </PopoverTrigger>
      <PopoverContent align="start" className="w-80">
        <div className="mb-1 flex items-center gap-2 text-sm font-medium">
          {status === "error" ? <CircleAlertIcon className="size-4 text-critical" /> : <CheckCircle2Icon className={cn("size-4", status === "ready" ? "text-good" : "text-muted-foreground")} />}
          {label}
        </div>
        {status === "error" && error && <p className="mb-1 text-xs text-destructive">{error}</p>}
        <dl className="divide-y">
          <Row k={t("Router online")} v={`${online} / ${routers.length}`} />
          <Row k={t("User aktif")} v={active} />
          <Row k={t("Event terakhir")} v={last ? formatRelative(last, now) : t("Belum ada")} />
          <Row k={t("Akses")} v={access === "desktop" ? t("Aplikasi desktop") : t("Browser (jarak jauh)")} />
          {access === "desktop" && share && (
            <>
              <Row
                k={t("Serve ke browser")}
                v={
                  <span className="inline-flex items-center gap-1.5">
                    <WifiIcon className="size-3.5" />
                    {share.lan.enabled ? (share.lan.url ?? t("Aktif")) : t("Mati")}
                  </span>
                }
              />
              <Row
                k={t("Akses publik")}
                v={
                  <span className={cn("inline-flex items-center gap-1.5", share.publik.status === "on" && "text-warning")}>
                    <GlobeIcon className="size-3.5" />
                    {share.publik.status === "on" ? t("Aktif") : share.publik.status === "connecting" ? t("Menyambung") : share.publik.status === "error" ? t("Galat") : t("Mati")}
                  </span>
                }
              />
            </>
          )}
          <Row k={t("Versi")} v={APP_VERSION} />
        </dl>
        {access === "desktop" && (
          <a href={href("berbagi")} className="mt-2 block text-xs text-brand underline-offset-2 hover:underline">
            {t("Atur berbagi (LAN dan Publik)")}
          </a>
        )}
      </PopoverContent>
    </Popover>
  )
}
