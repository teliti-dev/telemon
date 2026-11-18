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
  const { routers, users, events, routerById } = useStore()
  const now = useNow(10_000)

  const total = countByStatus(users)
  const online = routers.filter((r) => r.status === "online").length
  const failsHour = events.filter(
    (e) => e.type === "auth_failed" && now - e.at < 3600_000
  )

  const tiles = [
    {
      label: "Router online",
      value: `${online}/${routers.length}`,
      icon: RadioTowerIcon,
      to: href("router"),
      note: online < routers.length ? `${routers.length - online} bermasalah` : "Semua normal",
    },
    {
      label: "User aktif",
      value: formatNumber(total.active),
      icon: UserCheckIcon,
      to: href("user", { status: "active" }),
      note: `dari ${formatNumber(users.length)} user`,
    },
    {
      label: "Tidak aktif",
      value: formatNumber(total.inactive),
      icon: UserMinusIcon,
      to: href("user", { status: "inactive" }),
      note: "Secret ada, tidak terhubung",
    },
    {
      label: "Dinonaktifkan",
      value: formatNumber(total.disabled),
      icon: UserXIcon,
      to: href("user", { status: "disabled" }),
      note: "Secret disabled",
    },
    {
      label: "Gagal auth (1 jam)",
      value: formatNumber(failsHour.length),
      icon: ShieldXIcon,
      to: href("event", { type: "auth_failed" }),
      note: `${new Set(failsHour.map((e) => `${e.routerId}:${e.username}`)).size} user`,
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

  const recent = events.slice(0, 8)

  return (
    <div className="flex flex-col gap-4 md:gap-6">
      <section
        aria-label="Ringkasan angka"
        className="grid grid-cols-2 gap-3 md:gap-4 lg:grid-cols-5"
      >
        {tiles.map((t, i) => (
          <a
            key={t.label}
            href={t.to}
            className={
              "group rounded-xl outline-none focus-visible:ring-3 focus-visible:ring-ring/50" +
              (i === 0 ? " col-span-2 lg:col-span-1" : "")
            }
          >
            <Card className="h-full gap-2 py-4 transition-colors group-hover:bg-muted/40">
              <CardHeader className="px-4">
                <CardDescription className="flex items-center gap-2">
                  <t.icon className="size-4" />
                  {t.label}
                </CardDescription>
              </CardHeader>
              <CardContent className="px-4">
                <div className="text-2xl font-semibold tabular-nums md:text-3xl">
                  {t.value}
                </div>
                <p className="mt-1 truncate text-xs text-muted-foreground">{t.note}</p>
              </CardContent>
            </Card>
          </a>
        ))}
      </section>

      <section aria-label="Router" className="grid gap-3 sm:grid-cols-2 md:gap-4 xl:grid-cols-4">
        {routers.map((r) => {
          const c = countByStatus(users.filter((u) => u.routerId === r.id))
          return (
            <Card key={r.id} className="gap-3 py-4">
              <CardHeader className="px-4">
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
              <CardContent className="space-y-2 px-4">
                <UserMixBar active={c.active} inactive={c.inactive} disabled={c.disabled} />
                <div className="flex flex-wrap gap-x-3 gap-y-1 text-xs text-muted-foreground">
                  <span>
                    <span className="font-medium text-foreground tabular-nums">{c.active}</span> aktif
                  </span>
                  <span>
                    <span className="font-medium text-foreground tabular-nums">{c.inactive}</span> tidak aktif
                  </span>
                  <span>
                    <span className="font-medium text-foreground tabular-nums">{c.disabled}</span> dinonaktifkan
                  </span>
                </div>
                {r.status !== "online" && r.lastPollAt && (
                  <p className="text-xs text-muted-foreground">
                    Data terakhir {formatRelative(r.lastPollAt, now)}
                  </p>
                )}
              </CardContent>
            </Card>
          )
        })}
      </section>

      <div className="grid gap-4 md:gap-6 lg:grid-cols-3">
        <Card className="gap-0 py-0 lg:col-span-2">
          <CardHeader className="border-b px-4 py-3 [.border-b]:pb-3">
            <CardTitle>Event terbaru</CardTitle>
            <CardAction>
              <Button variant="ghost" size="sm" asChild>
                <a href={href("event")}>
                  Semua <ArrowRightIcon />
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
                      className="flex w-full items-center gap-3 px-4 py-2.5 text-left text-sm hover:bg-muted/50 disabled:hover:bg-transparent"
                    >
                      <EventIcon type={e.type} />
                      <div className="min-w-0 flex-1">
                        <div className="truncate">
                          <span className="font-medium">{e.username ?? routerById.get(e.routerId)?.name}</span>{" "}
                          <span className="text-muted-foreground">
                            {EVENT_META[e.type].label.toLowerCase()}
                          </span>
                        </div>
                        <div className="truncate text-xs text-muted-foreground">
                          {routerById.get(e.routerId)?.name}
                        </div>
                      </div>
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
          <CardHeader className="border-b px-4 py-3 [.border-b]:pb-3">
            <CardTitle>Sering gagal auth</CardTitle>
            <CardDescription>24 jam terakhir</CardDescription>
          </CardHeader>
          <CardContent className="px-0">
            {topFails.length === 0 ? (
              <p className="px-4 py-6 text-sm text-muted-foreground">
                Tidak ada gagal auth.
              </p>
            ) : (
              <ul className="divide-y">
                {topFails.map(([k, f]) => (
                  <li key={k}>
                    <button
                      type="button"
                      onClick={() => onOpenUser(k)}
                      className="flex w-full items-center gap-3 px-4 py-2.5 text-left text-sm hover:bg-muted/50"
                    >
                      <div className="min-w-0 flex-1">
                        <div className="truncate font-mono text-[13px]">{f.username}</div>
                        <div className="truncate text-xs text-muted-foreground">
                          {routerById.get(f.routerId)?.name} · terakhir {formatRelative(f.last, now)}
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
