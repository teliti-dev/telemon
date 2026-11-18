import * as React from "react"
import { DownloadIcon, LayoutGridIcon, ListIcon, PlusIcon, SearchIcon } from "lucide-react"

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
import { FilterSelect } from "@/components/filter-select"
import { ImportDialog } from "@/components/import-dialog"
import { LoadMore } from "@/components/skeletons"
import { UserFormDialog } from "@/components/user-form-dialog"
import { UsersSectionNav } from "@/components/users-section-nav"
import { useInfiniteList } from "@/hooks/use-infinite-list"
import { useUpdateGate } from "@/hooks/use-write-gate"
import { useStore } from "@/lib/store-context"
import { tk, useT } from "@/lib/i18n"
import { cn } from "@/lib/utils"
import type { PppUser, UserStatus } from "@/lib/types"

const PAGE_SIZE = 60
type StatusFilter = UserStatus | "all" | "deleted"
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
  { value: "all", label: tk("Semua") },
  { value: "active", label: tk("Aktif") },
  { value: "inactive", label: tk("Tidak aktif") },
  { value: "disabled", label: tk("Dinonaktifkan") },
  { value: "deleted", label: tk("Terhapus") },
]

function uptimeOrSeen(u: PppUser, now: number) {
  if (u.connectedAt) return { label: tk("Uptime"), value: formatDuration(now - u.connectedAt) }
  if (u.lastSeenAt) return { label: tk("Terlihat"), value: formatRelative(u.lastSeenAt, now) }
  return { label: "", value: "-" }
}

export function UsersPage({
  params,
  onOpenUser,
}: {
  params: URLSearchParams
  onOpenUser: (id: string) => void
}) {
  const t = useT()
  const { users, routers, routerById, metas } = useStore()
  const userReason = useUpdateGate("user")()
  const importReason = useUpdateGate("router")()
  const now = useNow(15_000)

  const [query, setQuery] = React.useState(params.get("q") ?? "")
  const [status, setStatus] = React.useState<StatusFilter>(
    (params.get("status") as StatusFilter) ?? "all"
  )
  const [routerId, setRouterId] = React.useState(params.get("router") ?? "all")
  const [sort, setSort] = React.useState<SortKey>("recent")
  const [view, setView] = React.useState<ViewMode>(loadView)

  const filtered = React.useMemo(() => {
    const q = query.trim().toLowerCase()
    const list = users.filter(
      (u) =>
        (status === "deleted" ? !!u.deletedAt : !u.deletedAt && (status === "all" || u.status === status)) &&
        (routerId === "all" || u.routerId === routerId) &&
        (!q ||
          u.username.toLowerCase().includes(q) ||
          u.address?.includes(q) ||
          u.callerId?.toLowerCase().includes(q) ||
          u.comment?.toLowerCase().includes(q) ||
          metas[u.id]?.customerName?.toLowerCase().includes(q) ||
          metas[u.id]?.phone?.includes(q) ||
          metas[u.id]?.tags.some((tag) => tag.toLowerCase().includes(q)))
    )
    const sorters: Record<SortKey, (a: PppUser, b: PppUser) => number> = {
      recent: (a, b) => (b.lastEventAt ?? 0) - (a.lastEventAt ?? 0),
      username: (a, b) => a.username.localeCompare(b.username),
      uptime: (a, b) =>
        (a.connectedAt ?? Number.MAX_SAFE_INTEGER) - (b.connectedAt ?? Number.MAX_SAFE_INTEGER),
    }
    return list.sort(sorters[sort])
  }, [users, metas, query, status, routerId, sort])

  const { count, hasMore, sentinelRef } = useInfiniteList(filtered.length, PAGE_SIZE, `${query}|${status}|${routerId}|${sort}|${view}`)
  const shown = filtered.slice(0, count)
  const [adding, setAdding] = React.useState(false)
  const [importing, setImporting] = React.useState(false)
  const canWriteAny = !userReason && routers.some((r) => r.canWrite)

  const onStatus = (v: string) => {
    setStatus(v as StatusFilter)
  }
  const onRouter = (v: string) => {
    setRouterId(v)
  }
  // i18n-ignore: label bertanda tk()
  const statusOptions = STATUS_OPTIONS.map((o) => ({ value: o.value, label: o.value === "all" ? t("Semua status") : t(o.label) }))
  const routerOptions = [{ value: "all", label: t("Semua router") }, ...routers.map((r) => ({ value: r.id, label: r.name }))]
  const sortOptions = [
    { value: "recent", label: t("Event terbaru"), short: t("Terbaru") },
    { value: "username", label: t("Username A-Z"), short: t("A-Z") },
    { value: "uptime", label: t("Uptime terlama"), short: t("Uptime") },
  ]
  const viewToggle = (
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
      aria-label={t("Tampilan")}
      className="shrink-0"
    >
      <ToggleGroupItem value="list" aria-label={t("Tampilan list")} title={t("List")}>
        <ListIcon />
      </ToggleGroupItem>
      <ToggleGroupItem value="grid" aria-label={t("Tampilan grid")} title={t("Grid")}>
        <LayoutGridIcon />
      </ToggleGroupItem>
    </ToggleGroup>
  )

  return (
    <div className="flex flex-col gap-3">
      <div className="flex flex-wrap items-center gap-2">
        <UsersSectionNav page="user" />
        <div className="ml-auto flex gap-2">
          <Button variant="outline" onClick={() => setImporting(true)} disabled={!!importReason} title={importReason ?? undefined}>
            <DownloadIcon />
            <span className="hidden sm:inline">{t("Import dari Mikrotik")}</span>
            <span className="sm:hidden">{t("Import")}</span>
          </Button>
          <Button
            onClick={() => setAdding(true)}
            disabled={!canWriteAny}
            title={canWriteAny ? undefined : (userReason ?? t("Tidak ada router dengan hak write"))}
          >
            <PlusIcon />
            <span className="hidden sm:inline">{t("Tambah user")}</span>
            <span className="sm:hidden">{t("Tambah")}</span>
          </Button>
        </div>
      </div>

      <div className="flex flex-col gap-2 lg:flex-row lg:items-center">
        {/* HP: baris 1 = cari + tampilan; baris 2 = tiga dropdown. Desktop: satu baris. */}
        <div className="flex gap-2 lg:contents">
          <div className="relative min-w-0 flex-1 lg:w-72 lg:flex-none">
            <SearchIcon className="pointer-events-none absolute top-1/2 left-2.5 size-4 -translate-y-1/2 text-muted-foreground" />
            <Input
              type="search"
              value={query}
              onChange={(e) => {
                setQuery(e.target.value)
              }}
              placeholder={t("Cari username, IP, MAC…")}
              aria-label={t("Cari user")}
              className="pl-8"
            />
          </div>
          <div className="lg:hidden">{viewToggle}</div>
        </div>

        <div className="flex gap-2 lg:hidden">
          <FilterSelect label={t("Status")} value={status} defaultValue="all" options={statusOptions} onChange={onStatus} className="flex-[1.3]" />
          <FilterSelect label={t("Router")} value={routerId} defaultValue="all" options={routerOptions} onChange={onRouter} />
          <FilterSelect label={t("Urutkan")} value={sort} defaultValue="recent" options={sortOptions} onChange={(v) => setSort(v as SortKey)} />
        </div>

        <div className="-mx-1 hidden overflow-x-auto px-1 lg:block">
          <ToggleGroup type="single" variant="outline" value={status} onValueChange={(v) => v && onStatus(v)} aria-label={t("Filter status")}>
            {STATUS_OPTIONS.map((o) => (
              <ToggleGroupItem key={o.value} value={o.value} className="px-3">
                {/* i18n-ignore: label bertanda tk() */}
                {t(o.label)}
              </ToggleGroupItem>
            ))}
          </ToggleGroup>
        </div>
        <div className="hidden gap-2 lg:ml-auto lg:flex">
          <Select value={routerId} onValueChange={onRouter}>
            <SelectTrigger className="w-44" aria-label={t("Router")}>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {routerOptions.map((o) => (
                <SelectItem key={o.value} value={o.value}>
                  {o.label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          <Select value={sort} onValueChange={(v) => setSort(v as SortKey)}>
            <SelectTrigger className="w-40" aria-label={t("Urutkan")}>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {sortOptions.map((o) => (
                <SelectItem key={o.value} value={o.value}>
                  {o.label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          {viewToggle}
        </div>
      </div>

      <div className="-mb-1 flex flex-wrap items-center gap-x-3 gap-y-1 text-xs text-muted-foreground">
        <p aria-live="polite">
          {t("{n} user", { n: formatNumber(filtered.length) })}
          {filtered.length > shown.length && `, ${t("menampilkan {n}", { n: formatNumber(shown.length) })}`}
        </p>
        <ul className="flex flex-wrap gap-x-3 gap-y-1 sm:ml-auto" aria-label={t("Keterangan warna")}>
          {USER_TONES.filter((tn) => tn !== "deleted" || status === "deleted").map((tn) => (
            <li key={tn} className="flex items-center gap-1.5">
              <span className={cn("size-2 rounded-full", USER_TONE[tn].dot)} />
              {/* i18n-ignore: label bertanda tk() */}
              {t(USER_TONE[tn].label)}
            </li>
          ))}
        </ul>
      </div>

      {filtered.length === 0 ? (
        <Card className="items-center py-12 text-sm text-muted-foreground">
          {t("Tidak ada user yang cocok dengan filter.")}
        </Card>
      ) : (
        <>
          {/* Desktop: tabel */}
          <Card className={view === "list" ? "hidden py-0 md:block" : "hidden"}>
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead className="pl-3">{t("Username")}</TableHead>
                  <TableHead className="hidden lg:table-cell">{t("Profil")}</TableHead>
                  <TableHead>{t("Status")}</TableHead>
                  <TableHead>{t("Router")}</TableHead>
                  <TableHead>IP</TableHead>
                  <TableHead className="hidden xl:table-cell">MAC</TableHead>
                  <TableHead>{t("Uptime / terlihat")}</TableHead>
                  <TableHead className="pr-3">{t("Event terakhir")}</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {shown.map((u) => {
                  const ut = uptimeOrSeen(u, now)
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
                      <TableCell className={cn("pl-3 font-mono text-[13px]", tone.bar, u.deletedAt && "line-through")} title={label}>
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
                        {ut.value}
                      </TableCell>
                      <TableCell className="pr-3">
                        {u.lastEvent && u.lastEventAt ? (
                          <span className="flex items-center gap-2">
                            <EventIcon type={u.lastEvent} />
                            {/* i18n-ignore: label bertanda tk() */}
                            <span>{t(EVENT_META[u.lastEvent].label)}</span>
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
                  const ut = uptimeOrSeen(u, now)
                  const tone = USER_TONE[userTone(u)]
                  return (
                    <li key={u.id}>
                      <button
                        type="button"
                        onClick={() => onOpenUser(u.id)}
                        className={cn("flex w-full items-center gap-2.5 px-3 py-2 text-left active:bg-muted/50", tone.row, tone.bar)}
                      >
                        {/* i18n-ignore: label bertanda tk() */}
                        <span className={cn("size-2 shrink-0 rounded-full", tone.dot)} aria-label={t(tone.label)} />
                        <div className="min-w-0 flex-1">
                          <div className="truncate font-mono text-[13px]">{u.username}</div>
                          <div className="truncate text-xs text-muted-foreground">
                            {routerById.get(u.routerId)?.name}
                            {u.address && ` · ${u.address}`}
                          </div>
                        </div>
                        <div className="shrink-0 text-right text-xs">
                          <div className={u.connectedAt ? "tabular-nums" : "text-muted-foreground tabular-nums"}>{ut.value}</div>
                          {u.lastEvent && (
                            <div className="flex items-center justify-end gap-1 text-muted-foreground">
                              <EventIcon type={u.lastEvent} className="size-3" />
                              {/* i18n-ignore: label bertanda tk() */}
                              {t(EVENT_META[u.lastEvent].label)}
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
                const ut = uptimeOrSeen(u, now)
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
                        <span className={cn("min-w-0 truncate font-mono text-[13px] font-medium", u.deletedAt && "line-through")} title={u.username}>
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
                          {/* i18n-ignore: label bertanda tk() */}
                          <dt className="text-muted-foreground">{t(ut.label || "Uptime")}</dt>
                          <dd className="truncate tabular-nums">{ut.value}</dd>
                        </div>
                      </dl>
                      <div className="mt-auto flex items-center gap-1.5 border-t pt-1.5 text-xs">
                        {u.lastEvent && u.lastEventAt ? (
                          <>
                            <EventIcon type={u.lastEvent} className="size-3.5" />
                            {/* i18n-ignore: label bertanda tk() */}
                            <span className="truncate">{t(EVENT_META[u.lastEvent].label)}</span>
                            <span className="ml-auto shrink-0 text-muted-foreground">
                              {formatRelative(u.lastEventAt, now)}
                            </span>
                          </>
                        ) : (
                          <span className="text-muted-foreground">{t("Belum ada event")}</span>
                        )}
                      </div>
                    </button>
                  </li>
                )
              })}
            </ul>
          )}

          {hasMore && <LoadMore sentinelRef={sentinelRef} variant={view === "grid" ? "cards" : "rows"} />}
        </>
      )}

      <UserFormDialog
        open={adding}
        onOpenChange={setAdding}
        mode="create"
        defaultRouterId={routerId !== "all" ? routerId : undefined}
      />
      <ImportDialog
        open={importing}
        onOpenChange={setImporting}
        defaultRouterId={routerId !== "all" ? routerId : undefined}
      />
    </div>
  )
}
