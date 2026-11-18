import * as React from "react"
import { SearchIcon } from "lucide-react"

import { EVENT_META, EventIcon, UserStatusBadge } from "@/components/status"
import { Button } from "@/components/ui/button"
import { Card } from "@/components/ui/card"
import { Input } from "@/components/ui/input"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table"
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group"
import { useNow } from "@/hooks/use-now"
import { formatDuration, formatNumber, formatRelative } from "@/lib/format"
import { useStore } from "@/lib/store-context"
import type { PppUser, UserStatus } from "@/lib/types"

const PAGE_SIZE = 50
type StatusFilter = UserStatus | "all"
type SortKey = "recent" | "username" | "uptime"

const STATUS_OPTIONS: { value: StatusFilter; label: string }[] = [
  { value: "all", label: "Semua" },
  { value: "active", label: "Aktif" },
  { value: "inactive", label: "Tidak aktif" },
  { value: "disabled", label: "Dinonaktifkan" },
]

function uptimeOrSeen(u: PppUser, now: number) {
  if (u.connectedAt) return { label: "Uptime", value: formatDuration(now - u.connectedAt) }
  if (u.lastSeenAt) return { label: "Terlihat", value: formatRelative(u.lastSeenAt, now) }
  return { label: "", value: "-" }
}

export function UsersPage({
  params,
  onOpenUser,
}: {
  params: URLSearchParams
  onOpenUser: (id: string) => void
}) {
  const { users, routers, routerById } = useStore()
  const now = useNow(15_000)

  const [query, setQuery] = React.useState(params.get("q") ?? "")
  const [status, setStatus] = React.useState<StatusFilter>(
    (params.get("status") as StatusFilter) ?? "all"
  )
  const [routerId, setRouterId] = React.useState(params.get("router") ?? "all")
  const [sort, setSort] = React.useState<SortKey>("recent")
  const [limit, setLimit] = React.useState(PAGE_SIZE)

  const filtered = React.useMemo(() => {
    const q = query.trim().toLowerCase()
    const list = users.filter(
      (u) =>
        (status === "all" || u.status === status) &&
        (routerId === "all" || u.routerId === routerId) &&
        (!q ||
          u.username.toLowerCase().includes(q) ||
          u.address?.includes(q) ||
          u.callerId?.toLowerCase().includes(q) ||
          u.comment?.toLowerCase().includes(q))
    )
    const sorters: Record<SortKey, (a: PppUser, b: PppUser) => number> = {
      recent: (a, b) => (b.lastEventAt ?? 0) - (a.lastEventAt ?? 0),
      username: (a, b) => a.username.localeCompare(b.username),
      uptime: (a, b) =>
        (a.connectedAt ?? Number.MAX_SAFE_INTEGER) - (b.connectedAt ?? Number.MAX_SAFE_INTEGER),
    }
    return list.sort(sorters[sort])
  }, [users, query, status, routerId, sort])

  const shown = filtered.slice(0, limit)

  return (
    <div className="flex flex-col gap-3">
      <div className="flex flex-col gap-2 lg:flex-row lg:items-center">
        <div className="relative lg:w-72">
          <SearchIcon className="pointer-events-none absolute top-1/2 left-2.5 size-4 -translate-y-1/2 text-muted-foreground" />
          <Input
            type="search"
            value={query}
            onChange={(e) => {
              setQuery(e.target.value)
              setLimit(PAGE_SIZE)
            }}
            placeholder="Cari username, IP, MAC…"
            aria-label="Cari user"
            className="pl-8"
          />
        </div>
        <div className="-mx-1 overflow-x-auto px-1">
          <ToggleGroup
            type="single"
            variant="outline"
            value={status}
            onValueChange={(v) => {
              if (v) {
                setStatus(v as StatusFilter)
                setLimit(PAGE_SIZE)
              }
            }}
            aria-label="Filter status"
          >
            {STATUS_OPTIONS.map((o) => (
              <ToggleGroupItem key={o.value} value={o.value} className="px-3">
                {o.label}
              </ToggleGroupItem>
            ))}
          </ToggleGroup>
        </div>
        <div className="grid grid-cols-2 gap-2 lg:ml-auto lg:flex">
          <Select
            value={routerId}
            onValueChange={(v) => {
              setRouterId(v)
              setLimit(PAGE_SIZE)
            }}
          >
            <SelectTrigger className="w-full lg:w-44" aria-label="Router">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="all">Semua router</SelectItem>
              {routers.map((r) => (
                <SelectItem key={r.id} value={r.id}>
                  {r.name}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          <Select value={sort} onValueChange={(v) => setSort(v as SortKey)}>
            <SelectTrigger className="w-full lg:w-40" aria-label="Urutkan">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="recent">Event terbaru</SelectItem>
              <SelectItem value="username">Username A-Z</SelectItem>
              <SelectItem value="uptime">Uptime terlama</SelectItem>
            </SelectContent>
          </Select>
        </div>
      </div>

      <p className="-mb-1 text-xs text-muted-foreground" aria-live="polite">
        {formatNumber(filtered.length)} user
        {filtered.length > shown.length && `, menampilkan ${formatNumber(shown.length)}`}
      </p>

      {filtered.length === 0 ? (
        <Card className="items-center py-12 text-sm text-muted-foreground">
          Tidak ada user yang cocok dengan filter.
        </Card>
      ) : (
        <>
          {/* Desktop: tabel */}
          <Card className="hidden py-0 md:block">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead className="pl-3">Username</TableHead>
                  <TableHead className="hidden lg:table-cell">Profil</TableHead>
                  <TableHead>Status</TableHead>
                  <TableHead>Router</TableHead>
                  <TableHead>IP</TableHead>
                  <TableHead className="hidden xl:table-cell">MAC</TableHead>
                  <TableHead>Uptime / terlihat</TableHead>
                  <TableHead className="pr-3">Event terakhir</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {shown.map((u) => {
                  const t = uptimeOrSeen(u, now)
                  return (
                    <TableRow
                      key={u.id}
                      tabIndex={0}
                      onClick={() => onOpenUser(u.id)}
                      onKeyDown={(e) => e.key === "Enter" && onOpenUser(u.id)}
                      className="cursor-pointer"
                    >
                      <TableCell className="pl-3 font-mono text-[13px]" title={u.comment}>
                        {u.username}
                        {u.comment && (
                          <span className="ml-1.5 font-sans text-xs text-muted-foreground">· {u.comment}</span>
                        )}
                      </TableCell>
                      <TableCell className="hidden text-muted-foreground lg:table-cell">{u.profile}</TableCell>
                      <TableCell>
                        <UserStatusBadge status={u.status} />
                      </TableCell>
                      <TableCell>{routerById.get(u.routerId)?.name}</TableCell>
                      <TableCell className="font-mono text-[13px]">{u.address ?? "-"}</TableCell>
                      <TableCell className="hidden font-mono text-[13px] xl:table-cell">
                        {u.callerId ?? "-"}
                      </TableCell>
                      <TableCell className={u.connectedAt ? "tabular-nums" : "text-muted-foreground tabular-nums"}>
                        {t.value}
                      </TableCell>
                      <TableCell className="pr-3">
                        {u.lastEvent && u.lastEventAt ? (
                          <span className="flex items-center gap-2">
                            <EventIcon type={u.lastEvent} />
                            <span>{EVENT_META[u.lastEvent].label}</span>
                            <span className="text-xs text-muted-foreground">
                              {formatRelative(u.lastEventAt, now)}
                            </span>
                          </span>
                        ) : (
                          "-"
                        )}
                      </TableCell>
                    </TableRow>
                  )
                })}
              </TableBody>
            </Table>
          </Card>

          {/* Mobile: kartu */}
          <ul className="flex flex-col gap-1.5 md:hidden">
            {shown.map((u) => {
              const t = uptimeOrSeen(u, now)
              return (
                <li key={u.id}>
                  <button
                    type="button"
                    onClick={() => onOpenUser(u.id)}
                    className="w-full rounded-lg border bg-card px-2.5 py-2 text-left text-sm active:bg-muted/50"
                  >
                    <div className="flex items-start justify-between gap-2">
                      <span className="min-w-0 font-mono text-[13px] break-all">{u.username}</span>
                      <UserStatusBadge status={u.status} className="shrink-0" />
                    </div>
                    <div className="mt-0.5 truncate text-xs text-muted-foreground">
                      {routerById.get(u.routerId)?.name} · {u.profile}
                      {u.address && ` · ${u.address}`}
                    </div>
                    <div className="mt-1 flex items-center justify-between gap-2 text-xs">
                      <span className="text-muted-foreground">
                        {t.label} <span className="text-foreground tabular-nums">{t.value}</span>
                      </span>
                      {u.lastEvent && u.lastEventAt && (
                        <span className="flex items-center gap-1.5">
                          <EventIcon type={u.lastEvent} className="size-3.5" />
                          {EVENT_META[u.lastEvent].label}
                          <span className="text-muted-foreground">
                            {formatRelative(u.lastEventAt, now)}
                          </span>
                        </span>
                      )}
                    </div>
                  </button>
                </li>
              )
            })}
          </ul>

          {filtered.length > shown.length && (
            <Button
              variant="outline"
              className="self-center"
              onClick={() => setLimit((n) => n + PAGE_SIZE)}
            >
              Tampilkan {Math.min(PAGE_SIZE, filtered.length - shown.length)} lagi
            </Button>
          )}
        </>
      )}
    </div>
  )
}
