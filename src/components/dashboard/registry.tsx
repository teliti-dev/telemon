import {
  ActivityIcon,
  ArrowRightIcon,
  RadioTowerIcon,
  ShieldXIcon,
  TagsIcon,
  UserCheckIcon,
  UserMinusIcon,
  UsersIcon,
  UserXIcon,
  GaugeIcon,
  type LucideIcon,
} from "lucide-react"

import { EVENT_META, EventIcon, RouterStatusBadge, UserMixBar } from "@/components/status"
import { Button } from "@/components/ui/button"
import { Card, CardAction, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import { useAutoHeight } from "@/components/dashboard/auto-height"
import { useNow } from "@/hooks/use-now"
import { formatNumber, formatRelative } from "@/lib/format"
import { tk, useT } from "@/lib/i18n"
import { href } from "@/lib/route"
import { useStore } from "@/lib/store-context"
import type { PppUser } from "@/lib/types"
import { AvgPerUserWidget, BANDWIDTH_CHART_DEFAULT, BandwidthChartConfig, BandwidthChartWidget, PER_USER_DEFAULT, PerUserConfig, TOTAL_DEFAULT, TotalBandwidthConfig, TotalBandwidthWidget } from "@/components/dashboard/bandwidth-widgets"
import type { WidgetDef, WidgetProps } from "@/components/dashboard/widget-types"

export type { WidgetDef, WidgetProps }

function countByStatus(users: PppUser[]) {
  const c = { active: 0, inactive: 0, disabled: 0 }
  for (const u of users) c[u.status]++
  return c
}

// ---------------- kartu angka ----------------

function StatTile({ label, value, note, icon: Icon, to }: { label: string; value: string; note: string; icon: LucideIcon; to: string }) {
  return (
    <a href={to} className="group block h-full rounded-xl outline-none focus-visible:ring-3 focus-visible:ring-ring/50">
      <Card className="h-full justify-center gap-1 py-2.5 transition-colors group-hover:bg-muted/40">
        <CardHeader className="px-3">
          <CardDescription className="flex items-center gap-1.5 text-xs">
            <Icon className="size-3.5" />
            {label}
          </CardDescription>
        </CardHeader>
        <CardContent className="flex items-baseline gap-2 px-3">
          <div className="text-xl font-semibold tabular-nums">{value}</div>
          <p className="min-w-0 truncate text-xs text-muted-foreground">{note}</p>
        </CardContent>
      </Card>
    </a>
  )
}

function RoutersOnline() {
  const t = useT()
  const { routers } = useStore()
  const online = routers.filter((r) => r.status === "online").length
  return (
    <StatTile
      label={t("Router online")}
      value={`${online}/${routers.length}`}
      icon={RadioTowerIcon}
      to={href("router")}
      note={online < routers.length ? t("{n} bermasalah", { n: routers.length - online }) : t("Semua normal")}
    />
  )
}

function UsersActive() {
  const t = useT()
  const { liveUsers: users } = useStore()
  return (
    <StatTile label={t("User aktif")} value={formatNumber(countByStatus(users).active)} icon={UserCheckIcon} to={href("user", { status: "active" })} note={t("dari {n} user", { n: formatNumber(users.length) })} />
  )
}

function UsersInactive() {
  const t = useT()
  const { liveUsers: users } = useStore()
  return (
    <StatTile label={t("Tidak aktif")} value={formatNumber(countByStatus(users).inactive)} icon={UserMinusIcon} to={href("user", { status: "inactive" })} note={t("Secret ada, tidak terhubung")} />
  )
}

function UsersDisabled() {
  const t = useT()
  const { liveUsers: users } = useStore()
  return (
    <StatTile label={t("Dinonaktifkan")} value={formatNumber(countByStatus(users).disabled)} icon={UserXIcon} to={href("user", { status: "disabled" })} note={t("Secret disabled")} />
  )
}

function AuthFails() {
  const t = useT()
  const { events } = useStore()
  const now = useNow(10_000)
  const fails = events.filter((e) => e.type === "auth_failed" && now - e.at < 3600_000)
  return (
    <StatTile
      label={t("Gagal auth (1 jam)")}
      value={formatNumber(fails.length)}
      icon={ShieldXIcon}
      to={href("event", { type: "auth_failed" })}
      note={t("{n} user", { n: new Set(fails.map((e) => `${e.routerId}:${e.username}`)).size })}
    />
  )
}

// ---------------- kartu router ----------------

function RouterCards({ widget }: WidgetProps) {
  const t = useT()
  const measure = useAutoHeight(widget.id)
  const { routers, liveUsers: users } = useStore()
  const now = useNow(10_000)
  return (
    <div className="h-full overflow-auto">
    <section ref={measure} aria-label={t("Router")} className="grid content-start gap-2 p-px sm:grid-cols-2 md:gap-3 xl:grid-cols-4">
      {routers.map((r) => {
        const c = countByStatus(users.filter((u) => u.routerId === r.id))
        return (
          <Card key={r.id} className="gap-2 py-2.5">
            <CardHeader className="px-3">
              <CardTitle className="truncate">
                <a href={href("user", { router: r.id })} className="hover:underline">
                  {r.name}
                </a>
              </CardTitle>
              <CardDescription className="truncate font-mono text-xs">
                {r.host}:{r.port}
              </CardDescription>
              <CardAction>
                <RouterStatusBadge status={r.status} />
              </CardAction>
            </CardHeader>
            <CardContent className="space-y-1.5 px-3">
              <UserMixBar active={c.active} inactive={c.inactive} disabled={c.disabled} />
              <div className="flex flex-wrap gap-x-3 gap-y-1 text-xs text-muted-foreground">
                <span>
                  <span className="font-medium text-foreground tabular-nums">{c.active}</span> {t("aktif")}
                </span>
                <span>
                  <span className="font-medium text-foreground tabular-nums">{c.inactive}</span> {t("tidak aktif")}
                </span>
                <span>
                  <span className="font-medium text-foreground tabular-nums">{c.disabled}</span> {t("dinonaktifkan")}
                </span>
              </div>
              {r.status !== "online" && r.lastPollAt && (
                <p className="text-xs text-muted-foreground">{t("Data terakhir {time}", { time: formatRelative(r.lastPollAt, now) })}</p>
              )}
            </CardContent>
          </Card>
        )
      })}
    </section>
    </div>
  )
}

// ---------------- event terbaru ----------------

function RecentEvents({ onOpenUser }: WidgetProps) {
  const t = useT()
  const { events, liveUsers: users, routerById } = useStore()
  const now = useNow(10_000)
  const recent = events.slice(0, 30)
  return (
    <Card className="h-full min-h-0 gap-0 py-0">
      <CardHeader className="border-b px-3 py-2 [.border-b]:pb-2">
        <CardTitle>{t("Event terbaru")}</CardTitle>
        <CardAction>
          <Button variant="ghost" size="sm" asChild>
            <a href={href("event")}>
              {t("Semua")} <ArrowRightIcon />
            </a>
          </Button>
        </CardAction>
      </CardHeader>
      <CardContent className="min-h-0 flex-1 overflow-auto px-0">
        <ul className="divide-y">
          {recent.map((e) => {
            const user = users.find((u) => u.routerId === e.routerId && u.username === e.username)
            return (
              <li key={e.id}>
                <button
                  type="button"
                  disabled={!user}
                  onClick={() => user && onOpenUser(user.id)}
                  className="flex w-full items-center gap-2.5 px-3 py-1.5 text-left text-sm hover:bg-muted/50 disabled:hover:bg-transparent"
                >
                  <EventIcon type={e.type} />
                  <div className="min-w-0 flex-1 truncate">
                    <span className="font-medium">{e.username ?? routerById.get(e.routerId)?.name}</span>{" "}
                    <span className="text-muted-foreground">
                      {/* i18n-ignore: label sudah ditandai tk() di status.tsx */}
                      {t(EVENT_META[e.type].label).toLowerCase()}
                    </span>
                  </div>
                  <span className="hidden shrink-0 text-xs text-muted-foreground sm:inline">{routerById.get(e.routerId)?.name}</span>
                  <time className="shrink-0 text-xs text-muted-foreground tabular-nums">{formatRelative(e.at, now)}</time>
                </button>
              </li>
            )
          })}
        </ul>
      </CardContent>
    </Card>
  )
}

// ---------------- sering gagal auth ----------------

function TopAuthFails({ onOpenUser }: WidgetProps) {
  const t = useT()
  const { events, routerById } = useStore()
  const now = useNow(10_000)
  const counts = new Map<string, { n: number; last: number; routerId: string; username: string }>()
  for (const e of events) {
    if (e.type !== "auth_failed" || !e.username || now - e.at > 86400_000) continue
    const k = `${e.routerId}:${e.username}`
    const cur = counts.get(k)
    if (cur) cur.n++
    else counts.set(k, { n: 1, last: e.at, routerId: e.routerId, username: e.username })
  }
  const top = [...counts.entries()].sort((a, b) => b[1].n - a[1].n || b[1].last - a[1].last).slice(0, 12)
  return (
    <Card className="h-full min-h-0 gap-0 py-0">
      <CardHeader className="border-b px-3 py-2 [.border-b]:pb-2">
        <CardTitle>{t("Sering gagal auth")}</CardTitle>
        <CardDescription>{t("24 jam terakhir")}</CardDescription>
      </CardHeader>
      <CardContent className="min-h-0 flex-1 overflow-auto px-0">
        {top.length === 0 ? (
          <p className="px-3 py-4 text-sm text-muted-foreground">{t("Tidak ada gagal auth.")}</p>
        ) : (
          <ul className="divide-y">
            {top.map(([k, f]) => (
              <li key={k}>
                <button type="button" onClick={() => onOpenUser(k)} className="flex w-full items-center gap-2.5 px-3 py-1.5 text-left text-sm hover:bg-muted/50">
                  <div className="min-w-0 flex-1">
                    <div className="truncate font-mono text-[13px]">{f.username}</div>
                    <div className="truncate text-xs text-muted-foreground">
                      {routerById.get(f.routerId)?.name} · {t("terakhir {time}", { time: formatRelative(f.last, now) })}
                    </div>
                  </div>
                  <span className="shrink-0 text-sm font-semibold tabular-nums">{f.n}×</span>
                </button>
              </li>
            ))}
          </ul>
        )}
      </CardContent>
    </Card>
  )
}

// ---------------- widget tambahan dari data yang sudah ada ----------------

function UserStatusMix() {
  const t = useT()
  const { liveUsers: users } = useStore()
  const c = countByStatus(users)
  return (
    <Card className="h-full min-h-0 gap-2 py-2.5">
      <CardHeader className="px-3">
        <CardTitle>{t("Komposisi user")}</CardTitle>
        <CardDescription>{t("Semua router")}</CardDescription>
      </CardHeader>
      <CardContent className="space-y-2 px-3">
        <UserMixBar active={c.active} inactive={c.inactive} disabled={c.disabled} />
        <dl className="grid grid-cols-3 gap-2 text-center">
          {[
            { k: t("aktif"), n: c.active },
            { k: t("tidak aktif"), n: c.inactive },
            { k: t("dinonaktifkan"), n: c.disabled },
          ].map((x) => (
            <div key={x.k}>
              <dd className="text-lg font-semibold tabular-nums">{formatNumber(x.n)}</dd>
              <dt className="text-xs text-muted-foreground">{x.k}</dt>
            </div>
          ))}
        </dl>
      </CardContent>
    </Card>
  )
}

function EventsPerHour() {
  const t = useT()
  const { events } = useStore()
  const now = useNow(60_000)
  const HOUR = 3600_000
  const start = Math.floor(now / HOUR) * HOUR - 23 * HOUR
  const buckets = Array.from({ length: 24 }, (_, i) => ({ at: start + i * HOUR, n: 0 }))
  for (const e of events) {
    const i = Math.floor((e.at - start) / HOUR)
    if (i >= 0 && i < 24) buckets[i].n++
  }
  const max = Math.max(1, ...buckets.map((b) => b.n))
  const total = buckets.reduce((s, b) => s + b.n, 0)
  return (
    <Card className="h-full min-h-0 gap-2 py-2.5">
      <CardHeader className="px-3">
        <CardTitle>{t("Event per jam")}</CardTitle>
        <CardDescription>{t("{n} event dalam 24 jam terakhir", { n: formatNumber(total) })}</CardDescription>
      </CardHeader>
      <CardContent className="flex min-h-0 flex-1 flex-col px-3">
        <div className="flex min-h-0 flex-1 items-end gap-0.5" role="img" aria-label={t("Grafik batang jumlah event per jam")}>
          {buckets.map((b) => (
            <div
              key={b.at}
              title={`${new Date(b.at).toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" })}: ${b.n}`}
              className="flex-1 rounded-sm bg-brand/80"
              style={{ height: `${Math.max(b.n > 0 ? 6 : 2, (b.n / max) * 100)}%`, opacity: b.n > 0 ? 1 : 0.25 }}
            />
          ))}
        </div>
        <div className="mt-1 flex justify-between text-[10px] text-muted-foreground tabular-nums">
          <span>{new Date(start).toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" })}</span>
          <span>{t("sekarang")}</span>
        </div>
      </CardContent>
    </Card>
  )
}

function UsersPerProfile() {
  const t = useT()
  const { liveUsers: users } = useStore()
  const counts = new Map<string, number>()
  for (const u of users) counts.set(u.profile, (counts.get(u.profile) ?? 0) + 1)
  const rows = [...counts.entries()].sort((a, b) => b[1] - a[1]).slice(0, 8)
  const max = Math.max(1, ...rows.map((r) => r[1]))
  return (
    <Card className="h-full min-h-0 gap-0 py-0">
      <CardHeader className="border-b px-3 py-2 [.border-b]:pb-2">
        <CardTitle>{t("User per profil")}</CardTitle>
        <CardDescription>{t("Profil PPP terbanyak dipakai")}</CardDescription>
      </CardHeader>
      <CardContent className="min-h-0 flex-1 overflow-auto px-3 py-2">
        <ul className="grid gap-1.5">
          {rows.map(([name, n]) => (
            <li key={name} className="grid grid-cols-[minmax(0,8rem)_1fr_auto] items-center gap-2 text-sm">
              <span className="truncate font-mono text-xs">{name}</span>
              <span className="h-2 overflow-hidden rounded-full bg-muted">
                <span className="block h-full rounded-full bg-brand/80" style={{ width: `${(n / max) * 100}%` }} />
              </span>
              <span className="tabular-nums">{formatNumber(n)}</span>
            </li>
          ))}
        </ul>
      </CardContent>
    </Card>
  )
}

// ---------------- daftar jenis ----------------

const TILE = { w: 4, h: 24, resizable: false, tile: true } as const

export const WIDGETS: WidgetDef[] = [
  { type: "stat-routers-online", title: tk("Router online"), description: tk("Jumlah router yang terhubung dari semua router."), icon: RadioTowerIcon, ...TILE, Component: RoutersOnline },
  { type: "stat-users-active", title: tk("User aktif"), description: tk("User yang sedang terhubung."), icon: UserCheckIcon, ...TILE, Component: UsersActive },
  { type: "stat-users-inactive", title: tk("Tidak aktif"), description: tk("Secret ada tetapi tidak terhubung."), icon: UserMinusIcon, ...TILE, Component: UsersInactive },
  { type: "stat-users-disabled", title: tk("Dinonaktifkan"), description: tk("Secret yang dinonaktifkan."), icon: UserXIcon, ...TILE, Component: UsersDisabled },
  { type: "stat-auth-fails", title: tk("Gagal auth (1 jam)"), description: tk("Percobaan masuk yang gagal dalam satu jam terakhir."), icon: ShieldXIcon, ...TILE, Component: AuthFails },
  { type: "router-cards", title: tk("Kartu router"), description: tk("Status dan komposisi user per router."), icon: RadioTowerIcon, w: 20, h: 48, minW: 6, minH: 24, resizable: true, stackAuto: true, autoHeight: true, Component: RouterCards },
  { type: "recent-events", title: tk("Event terbaru"), description: tk("Daftar event PPPoE terbaru; klik untuk membuka user."), icon: ActivityIcon, w: 13, h: 108, minW: 6, minH: 60, resizable: true, Component: RecentEvents },
  { type: "top-auth-fails", title: tk("Sering gagal auth"), description: tk("User yang paling sering gagal auth dalam 24 jam."), icon: ShieldXIcon, w: 7, h: 108, minW: 5, minH: 48, resizable: true, Component: TopAuthFails },
  { type: "user-status-mix", title: tk("Komposisi user"), description: tk("Perbandingan user aktif, tidak aktif, dan dinonaktifkan."), icon: UsersIcon, w: 7, h: 48, minW: 5, minH: 36, resizable: true, Component: UserStatusMix },
  { type: "events-per-hour", title: tk("Event per jam"), description: tk("Grafik batang jumlah event dalam 24 jam terakhir."), icon: ActivityIcon, w: 10, h: 60, minW: 6, minH: 48, resizable: true, Component: EventsPerHour },
  { type: "bandwidth-chart", title: tk("Bandwidth"), description: tk("Grafik laju unduh dan unggah satu antarmuka atau seluruh sesi PPPoE; router dan antarmuka dipilih di pengaturan widget."), icon: ActivityIcon, w: 12, h: 84, minW: 6, minH: 60, resizable: true, defaultConfig: BANDWIDTH_CHART_DEFAULT, Config: BandwidthChartConfig, Component: BandwidthChartWidget },
  { type: "bandwidth-total", title: tk("Total bandwidth pelanggan"), description: tk("Jumlah laju seluruh sesi PPPoE dari semua router, dengan jumlah sesi."), icon: GaugeIcon, w: 8, h: 60, minW: 5, minH: 48, resizable: true, defaultConfig: TOTAL_DEFAULT, Config: TotalBandwidthConfig, Component: TotalBandwidthWidget },
  { type: "bandwidth-per-user", title: tk("Rata-rata per user"), description: tk("Rata-rata unduh dan unggah per user aktif, untuk satu router atau semuanya."), icon: UsersIcon, w: 8, h: 60, minW: 5, minH: 48, resizable: true, defaultConfig: PER_USER_DEFAULT, Config: PerUserConfig, Component: AvgPerUserWidget },
  { type: "users-per-profile", title: tk("User per profil"), description: tk("Profil PPP yang paling banyak dipakai."), icon: TagsIcon, w: 7, h: 72, minW: 5, minH: 48, resizable: true, Component: UsersPerProfile },
]

export const WIDGET_BY_TYPE = new Map(WIDGETS.map((w) => [w.type, w]))
