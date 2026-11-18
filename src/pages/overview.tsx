import {
  ArrowRightIcon,
  ShieldXIcon,
  UserCheckIcon,
  UserMinusIcon,
  UserXIcon,
  RadioTowerIcon,
} from "lucide-react"

import {
  EVENT_META,
  EventIcon,
  RouterStatusBadge,
  UserMixBar,
} from "@/components/status"
import { Button } from "@/components/ui/button"
import {
  Card,
  CardAction,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card"
import { useNow } from "@/hooks/use-now"
import { useT } from "@/lib/i18n"
import { formatNumber, formatRelative } from "@/lib/format"
import { href } from "@/lib/route"
import { useStore } from "@/lib/store-context"
import type { PppUser } from "@/lib/types"

function countByStatus(users: PppUser[]) {
  const c = { active: 0, inactive: 0, disabled: 0 }
  for (const u of users) c[u.status]++
  return c
}

export function OverviewPage({ onOpenUser }: { onOpenUser: (id: string) => void }) {
  const { routers, liveUsers: users, events, routerById } = useStore()
  const now = useNow(10_000)
  const t = useT()

  const total = countByStatus(users)
  const online = routers.filter((r) => r.status === "online").length
  const failsHour = events.filter(
    (e) => e.type === "auth_failed" && now - e.at < 3600_000
  )

  const tiles = [
    {
      label: t("Router online"),
      value: `${online}/${routers.length}`,
      icon: RadioTowerIcon,
      to: href("router"),
      note: online < routers.length ? t("{n} bermasalah", { n: routers.length - online }) : t("Semua normal"),
    },
    {
      label: t("User aktif"),
      value: formatNumber(total.active),
      icon: UserCheckIcon,
      to: href("user", { status: "active" }),
      note: t("dari {n} user", { n: formatNumber(users.length) }),
    },
    {
      label: t("Tidak aktif"),
      value: formatNumber(total.inactive),
      icon: UserMinusIcon,
      to: href("user", { status: "inactive" }),
      note: t("Secret ada, tidak terhubung"),
    },
    {
      label: t("Dinonaktifkan"),
      value: formatNumber(total.disabled),
      icon: UserXIcon,
      to: href("user", { status: "disabled" }),
      note: t("Secret disabled"),
    },
    {
      label: t("Gagal auth (1 jam)"),
      value: formatNumber(failsHour.length),
      icon: ShieldXIcon,
      to: href("event", { type: "auth_failed" }),
      note: t("{n} user", { n: new Set(failsHour.map((e) => `${e.routerId}:${e.username}`)).size }),
    },
  ]

  // User yang paling sering gagal auth dalam 24 jam.
  const failCounts = new Map<string, { n: number; last: number; routerId: string; username: string }>()
  for (const e of events) {
    if (e.type !== "auth_failed" || !e.username || now - e.at > 86400_000) continue
    const k = `${e.routerId}:${e.username}`
    const cur = failCounts.get(k)
    if (cur) cur.n++
    else failCounts.set(k, { n: 1, last: e.at, routerId: e.routerId, username: e.username })
  }
  const topFails = [...failCounts.entries()]
    .sort((a, b) => b[1].n - a[1].n || b[1].last - a[1].last)
    .slice(0, 6)

  const recent = events.slice(0, 10)

  return (
    <div className="flex flex-col gap-3">
      <section
        aria-label={t("Ringkasan angka")}
        className="grid grid-cols-2 gap-2 md:gap-3 lg:grid-cols-5"
      >
        {tiles.map((tile, i) => (
          <a
            key={tile.label}
            href={tile.to}
            className={
              "group rounded-xl outline-none focus-visible:ring-3 focus-visible:ring-ring/50" +
              (i === 0 ? " col-span-2 lg:col-span-1" : "")
            }
          >
            <Card className="h-full gap-1 py-2.5 transition-colors group-hover:bg-muted/40">
              <CardHeader className="px-3">
                <CardDescription className="flex items-center gap-1.5 text-xs">
                  <tile.icon className="size-3.5" />
                  {tile.label}
                </CardDescription>
              </CardHeader>
              <CardContent className="flex items-baseline gap-2 px-3">
                <div className="text-xl font-semibold tabular-nums">{tile.value}</div>
                <p className="min-w-0 truncate text-xs text-muted-foreground">{tile.note}</p>
              </CardContent>
            </Card>
          </a>
        ))}
      </section>

      <section aria-label={t("Router")} className="grid gap-2 sm:grid-cols-2 md:gap-3 xl:grid-cols-4">
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
                  <p className="text-xs text-muted-foreground">
                    {t("Data terakhir {time}", { time: formatRelative(r.lastPollAt, now) })}
                  </p>
                )}
              </CardContent>
            </Card>
          )
        })}
      </section>

      <div className="grid gap-3 lg:grid-cols-3">
        <Card className="gap-0 py-0 lg:col-span-2">
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
          <CardContent className="px-0">
            <ul className="divide-y">
              {recent.map((e) => {
                const user = users.find(
                  (u) => u.routerId === e.routerId && u.username === e.username
                )
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
                      <span className="hidden shrink-0 text-xs text-muted-foreground sm:inline">
                        {routerById.get(e.routerId)?.name}
                      </span>
                      <time className="shrink-0 text-xs text-muted-foreground tabular-nums">
                        {formatRelative(e.at, now)}
                      </time>
                    </button>
                  </li>
                )
              })}
            </ul>
          </CardContent>
        </Card>

        <Card className="gap-0 py-0">
          <CardHeader className="border-b px-3 py-2 [.border-b]:pb-2">
            <CardTitle>{t("Sering gagal auth")}</CardTitle>
            <CardDescription>{t("24 jam terakhir")}</CardDescription>
          </CardHeader>
          <CardContent className="px-0">
            {topFails.length === 0 ? (
              <p className="px-3 py-4 text-sm text-muted-foreground">
                {t("Tidak ada gagal auth.")}
              </p>
            ) : (
              <ul className="divide-y">
                {topFails.map(([k, f]) => (
                  <li key={k}>
                    <button
                      type="button"
                      onClick={() => onOpenUser(k)}
                      className="flex w-full items-center gap-2.5 px-3 py-1.5 text-left text-sm hover:bg-muted/50"
                    >
                      <div className="min-w-0 flex-1">
                        <div className="truncate font-mono text-[13px]">{f.username}</div>
                        <div className="truncate text-xs text-muted-foreground">
                          {routerById.get(f.routerId)?.name} · {t("terakhir {time}", { time: formatRelative(f.last, now) })}
                        </div>
                      </div>
                      <span className="shrink-0 text-sm font-semibold tabular-nums">
                        {f.n}×
                      </span>
                    </button>
                  </li>
                ))}
              </ul>
            )}
          </CardContent>
        </Card>
      </div>
    </div>
  )
}
