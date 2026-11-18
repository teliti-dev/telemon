import * as React from "react"
import { LayoutGridIcon, ListIcon, SearchIcon } from "lucide-react"

import {
  EVENT_META,
  EventIcon,
  USER_TONE,
  USER_TONES,
  UserToneBadge,
  userTone,
} from "@/components/status"
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
import { cn } from "@/lib/utils"
import type { PppUser, UserStatus } from "@/lib/types"

const PAGE_SIZE = 60
type StatusFilter = UserStatus | "all"
type SortKey = "recent" | "username" | "uptime"
type ViewMode = "list" | "grid"

const VIEW_KEY = "telemon.users.view"
function loadView(): ViewMode {
  try {
    // Bawaan grid; bila pernah memilih, pakai pilihan terakhir.
    return localStorage.getItem(VIEW_KEY) === "list" ? "list" : "grid"
  } catch {
    return "grid"
  }
}
function saveView(v: ViewMode) {
  try {
    localStorage.setItem(VIEW_KEY, v)
  } catch {
    /* penyimpanan browser tidak tersedia; pilihan hanya berlaku sesi ini */
  }
}

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
  const { users, routers, routerById, metas } = useStore()
  const now = useNow(15_000)

  const [query, setQuery] = React.useState(params.get("q") ?? "")
  const [status, setStatus] = React.useState<StatusFilter>(
    (params.get("status") as StatusFilter) ?? "all"
  )
  const [routerId, setRouterId] = React.useState(params.get("router") ?? "all")
  const [sort, setSort] = React.useState<SortKey>("recent")
  const [view, setView] = React.useState<ViewMode>(loadView)
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
          u.comment?.toLowerCase().includes(q) ||
          metas[u.id]?.customerName?.toLowerCase().includes(q) ||
          metas[u.id]?.phone?.includes(q) ||
          metas[u.id]?.tags.some((t) => t.toLowerCase().includes(q)))
    )
    const sorters: Record<SortKey, (a: PppUser, b: PppUser) => number> = {
      recent: (a, b) => (b.lastEventAt ?? 0) - (a.lastEventAt ?? 0),
      username: (a, b) => a.username.localeCompare(b.username),
      uptime: (a, b) =>
        (a.connectedAt ?? Number.MAX_SAFE_INTEGER) - (b.connectedAt ?? Number.MAX_SAFE_INTEGER),
    }
    return list.sort(sorters[sort])
  }, [users, metas, query, status, routerId, sort])

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
        <div className="flex gap-2 lg:ml-auto">
          <Select
            value={routerId}
            onValueChange={(v) => {
              setRouterId(v)
              setLimit(PAGE_SIZE)
            }}
          >
            <SelectTrigger className="min-w-0 flex-1 lg:w-44 lg:flex-none" aria-label="Router">
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
            <SelectTrigger className="min-w-0 flex-1 lg:w-40 lg:flex-none" aria-label="Urutkan">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="recent">Event terbaru</SelectItem>
              <SelectItem value="username">Username A-Z</SelectItem>
              <SelectItem value="uptime">Uptime terlama</SelectItem>
            </SelectContent>
          </Select>
          <ToggleGroup
            type="single"
            variant="outline"
            value={view}
            onValueChange={(v) => {
              if (v) {
                setView(v as ViewMode)
                saveView(v as ViewMode)
              }
            }}
            aria-label="Tampilan"
            className="shrink-0"
          >
            <ToggleGroupItem value="list" aria-label="Tampilan list" title="List">
              <ListIcon />
            </ToggleGroupItem>
            <ToggleGroupItem value="grid" aria-label="Tampilan grid" title="Grid">
              <LayoutGridIcon />
            </ToggleGroupItem>
          </ToggleGroup>
        </div>
      </div>

      <div className="-mb-1 flex flex-wrap items-center gap-x-3 gap-y-1 text-xs text-muted-foreground">
        <p aria-live="polite">
          {formatNumber(filtered.length)} user
          {filtered.length > shown.length && `, menampilkan ${formatNumber(shown.length)}`}
        </p>
        <ul className="flex flex-wrap gap-x-3 gap-y-1 sm:ml-auto" aria-label="Keterangan warna">
          {USER_TONES.map((t) => (
            <li key={t} className="flex items-center gap-1.5">
              <span className={cn("size-2 rounded-full", USER_TONE[t].dot)} />
              {USER_TONE[t].label}
            </li>
          ))}
        </ul>
      </div>

      {filtered.length === 0 ? (
        <Card className="items-center py-12 text-sm text-muted-foreground">
          Tidak ada user yang cocok dengan filter.
        </Card>
      ) : (
        <>
          {/* Desktop: tabel */}
          <Card className={view === "list" ? "hidden py-0 md:block" : "hidden"}>
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
                  const tone = USER_TONE[userTone(u)]
                  const label = metas[u.id]?.customerName ?? u.comment
                  return (
                    <TableRow
                      key={u.id}
                      tabIndex={0}
                      onClick={() => onOpenUser(u.id)}
                      onKeyDown={(e) => e.key === "Enter" && onOpenUser(u.id)}
                      className={cn("cursor-pointer", tone.row)}
                    >
                      <TableCell className={cn("pl-3 font-mono text-[13px]", tone.bar)} title={label}>
                        {u.username}
                        {label && (
                          <span className="ml-1.5 font-sans text-xs text-muted-foreground">· {label}</span>
                        )}
                      </TableCell>
                      <TableCell className="hidden text-muted-foreground lg:table-cell">{u.profile}</TableCell>
                      <TableCell>
                        <UserToneBadge tone={userTone(u)} />
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

          {/* List di HP: daftar rapat dalam satu card */}
          {view === "list" && (
            <Card className="gap-0 py-0 md:hidden">
              <ul className="divide-y">
                {shown.map((u) => {
                  const t = uptimeOrSeen(u, now)
                  const tone = USER_TONE[userTone(u)]
                  return (
                    <li key={u.id}>
                      <button
                        type="button"
                        onClick={() => onOpenUser(u.id)}
                        className={cn("flex w-full items-center gap-2.5 px-3 py-2 text-left active:bg-muted/50", tone.row, tone.bar)}
                      >
                        <span className={cn("size-2 shrink-0 rounded-full", tone.dot)} aria-label={tone.label} />
                        <div className="min-w-0 flex-1">
                          <div className="truncate font-mono text-[13px]">{u.username}</div>
                          <div className="truncate text-xs text-muted-foreground">
                            {routerById.get(u.routerId)?.name}
                            {u.address && ` · ${u.address}`}
                          </div>
                        </div>
                        <div className="shrink-0 text-right text-xs">
                          <div className={u.connectedAt ? "tabular-nums" : "text-muted-foreground tabular-nums"}>{t.value}</div>
                          {u.lastEvent && (
                            <div className="flex items-center justify-end gap-1 text-muted-foreground">
                              <EventIcon type={u.lastEvent} className="size-3" />
                              {EVENT_META[u.lastEvent].label}
                            </div>
                          )}
                        </div>
                      </button>
                    </li>
                  )
                })}
              </ul>
            </Card>
          )}

          {/* Grid: kartu per user */}
          {view === "grid" && (
            <ul className="grid grid-cols-1 gap-2 min-[420px]:grid-cols-2 md:grid-cols-3 xl:grid-cols-4 2xl:grid-cols-5">
              {shown.map((u) => {
                const t = uptimeOrSeen(u, now)
                const tone = userTone(u)
                const label = metas[u.id]?.customerName ?? u.comment
                return (
                  <li key={u.id} className="min-w-0">
                    <button
                      type="button"
                      onClick={() => onOpenUser(u.id)}
                      className={cn(
                        "flex h-full w-full flex-col gap-1.5 rounded-xl border border-l-4 bg-card p-3 text-left transition-colors hover:bg-muted/40 focus-visible:ring-3 focus-visible:ring-ring/50 focus-visible:outline-none",
                        USER_TONE[tone].card
                      )}
                    >
                      <div className="flex items-start justify-between gap-2">
                        <span className="min-w-0 truncate font-mono text-[13px] font-medium" title={u.username}>
                          {u.username}
                        </span>
                        <UserToneBadge tone={tone} className="shrink-0" />
                      </div>
                      <div className="truncate text-xs text-muted-foreground">
                        {routerById.get(u.routerId)?.name} · {u.profile}
                        {label && ` · ${label}`}
                      </div>
                      <dl className="grid grid-cols-2 gap-x-2 text-xs">
                        <div className="min-w-0">
                          <dt className="text-muted-foreground">IP</dt>
                          <dd className="truncate font-mono">{u.address ?? "-"}</dd>
                        </div>
                        <div className="min-w-0">
                          <dt className="text-muted-foreground">{t.label || "Uptime"}</dt>
                          <dd className="truncate tabular-nums">{t.value}</dd>
                        </div>
                      </dl>
                      <div className="mt-auto flex items-center gap-1.5 border-t pt-1.5 text-xs">
                        {u.lastEvent && u.lastEventAt ? (
                          <>
                            <EventIcon type={u.lastEvent} className="size-3.5" />
                            <span className="truncate">{EVENT_META[u.lastEvent].label}</span>
                            <span className="ml-auto shrink-0 text-muted-foreground">
                              {formatRelative(u.lastEventAt, now)}
                            </span>
                          </>
                        ) : (
                          <span className="text-muted-foreground">Belum ada event</span>
                        )}
                      </div>
                    </button>
                  </li>
                )
              })}
            </ul>
          )}

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
