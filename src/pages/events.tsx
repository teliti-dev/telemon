import * as React from "react"
import { PauseIcon, PlayIcon, SearchIcon } from "lucide-react"

import { FilterMulti, FilterSelect } from "@/components/filter-select"
import { EVENT_META, EVENT_TYPES, EventIcon } from "@/components/status"
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
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group"
import { formatDateTime, formatNumber, formatTime } from "@/lib/format"
import { useStore } from "@/lib/store-context"
import type { PppEvent, PppEventType } from "@/lib/types"

const PAGE_SIZE = 100

export function EventsPage({
  params,
  onOpenUser,
}: {
  params: URLSearchParams
  onOpenUser: (id: string) => void
}) {
  const { events, routers, routerById, users } = useStore()
  const initialTypes = params.get("type")?.split(",").filter(Boolean) as
    | PppEventType[]
    | undefined
  const [types, setTypes] = React.useState<PppEventType[]>(initialTypes ?? [])
  const [routerId, setRouterId] = React.useState(params.get("router") ?? "all")
  const [query, setQuery] = React.useState("")
  const [paused, setPaused] = React.useState<PppEvent[] | null>(null)
  const [limit, setLimit] = React.useState(PAGE_SIZE)

  const source = paused ?? events
  const newSincePause = paused ? events.length - paused.length : 0

  const userIds = React.useMemo(
    () => new Set(users.map((u) => u.id)),
    [users]
  )

  const filtered = React.useMemo(() => {
    const q = query.trim().toLowerCase()
    return source.filter(
      (e) =>
        (types.length === 0 || types.includes(e.type)) &&
        (routerId === "all" || e.routerId === routerId) &&
        (!q ||
          e.username?.toLowerCase().includes(q) ||
          e.message.toLowerCase().includes(q) ||
          e.callerId?.toLowerCase().includes(q))
    )
  }, [source, types, routerId, query])

  const shown = filtered.slice(0, limit)

  const routerOptions = [{ value: "all", label: "Semua router" }, ...routers.map((r) => ({ value: r.id, label: r.name }))]

  return (
    <div className="flex flex-col gap-3">
      <div className="flex flex-col gap-2">
        {/* Desktop: chip jenis event */}
        <div className="-mx-1 hidden overflow-x-auto px-1 md:block">
          <ToggleGroup
            type="multiple"
            variant="outline"
            spacing={1}
            value={types}
            onValueChange={(v) => setTypes(v as PppEventType[])}
            aria-label="Jenis event"
            className="flex-wrap justify-start"
          >
            {EVENT_TYPES.map((t) => (
              <ToggleGroupItem key={t} value={t} className="gap-1.5 px-2.5">
                <EventIcon type={t} className="size-3.5" />
                {EVENT_META[t].label}
              </ToggleGroupItem>
            ))}
          </ToggleGroup>
        </div>
        <div className="flex flex-col gap-2 md:flex-row md:items-center">
          <div className="relative md:w-72">
            <SearchIcon className="pointer-events-none absolute top-1/2 left-2.5 size-4 -translate-y-1/2 text-muted-foreground" />
            <Input
              type="search"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder="Cari username, pesan, MAC…"
              aria-label="Cari event"
              className="pl-8"
            />
          </div>

          {/* HP: jenis, router, dan jeda dalam satu baris */}
          <div className="flex gap-2 md:hidden">
            <FilterMulti
              label="Jenis"
              values={types}
              onChange={setTypes}
              options={EVENT_TYPES.map((t) => ({
                value: t,
                label: EVENT_META[t].label,
                icon: <EventIcon type={t} className="size-3.5" />,
              }))}
            />
            <FilterSelect
              label="Router"
              value={routerId}
              defaultValue="all"
              onChange={setRouterId}
              options={routerOptions}
            />
            <Button
              size="icon"
              variant={paused ? "default" : "outline"}
              aria-label={paused ? "Lanjutkan" : "Jeda"}
              title={paused ? "Lanjutkan" : "Jeda"}
              onClick={() => setPaused(paused ? null : events)}
            >
              {paused ? <PlayIcon /> : <PauseIcon />}
            </Button>
          </div>

          <div className="hidden gap-2 md:ml-auto md:flex">
            <Select value={routerId} onValueChange={setRouterId}>
              <SelectTrigger className="w-44" aria-label="Router">
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
            <Button variant={paused ? "default" : "outline"} onClick={() => setPaused(paused ? null : events)}>
              {paused ? <PlayIcon /> : <PauseIcon />}
              {paused ? "Lanjutkan" : "Jeda"}
            </Button>
          </div>
        </div>
      </div>

      <p className="-mb-1 text-xs text-muted-foreground" aria-live="polite">
        {formatNumber(filtered.length)} event
        {paused && newSincePause > 0 && (
          <>
            {" · "}
            <button
              type="button"
              className="font-medium text-foreground underline underline-offset-4"
              onClick={() => setPaused(null)}
            >
              {formatNumber(newSincePause)} event baru
            </button>
          </>
        )}
      </p>

      {filtered.length === 0 ? (
        <Card className="items-center py-12 text-sm text-muted-foreground">
          Tidak ada event yang cocok dengan filter.
        </Card>
      ) : (
        <Card className="gap-0 py-0">
          <ol className="divide-y">
            {shown.map((e) => {
              const uid = `${e.routerId}:${e.username}`
              const canOpen = !!e.username && userIds.has(uid)
              return (
                <li
                  key={e.id}
                  className="grid grid-cols-1 gap-x-3 gap-y-0.5 px-3 py-1.5 text-[13px] md:grid-cols-[4.5rem_8.5rem_minmax(9rem,13rem)_7rem_1fr] md:items-center"
                >
                  <time
                    className="order-2 pl-6 text-xs text-muted-foreground tabular-nums md:order-none md:pl-0 md:text-[13px]"
                    dateTime={new Date(e.at).toISOString()}
                    title={formatDateTime(e.at)}
                  >
                    {formatTime(e.at)}
                    <span className="md:hidden"> · {routerById.get(e.routerId)?.name}</span>
                  </time>
                  <span className="order-1 flex min-w-0 items-center gap-2 md:order-none">
                    <EventIcon type={e.type} />
                    <span className="shrink-0">{EVENT_META[e.type].label}</span>
                    {e.username && (
                      <button
                        type="button"
                        disabled={!canOpen}
                        onClick={() => onOpenUser(uid)}
                        className="truncate font-mono text-[13px] hover:underline disabled:no-underline md:hidden"
                      >
                        {e.username}
                      </button>
                    )}
                  </span>
                  <span className="hidden min-w-0 md:block">
                    {e.username ? (
                      <button
                        type="button"
                        disabled={!canOpen}
                        onClick={() => onOpenUser(uid)}
                        className="block max-w-full truncate font-mono text-[13px] hover:underline disabled:no-underline"
                      >
                        {e.username}
                      </button>
                    ) : (
                      <span className="text-muted-foreground">-</span>
                    )}
                  </span>
                  <span className="hidden truncate md:block">
                    {routerById.get(e.routerId)?.name}
                  </span>
                  <span className="order-3 truncate pl-6 font-mono text-xs text-muted-foreground md:order-none md:pl-0">
                    {e.message}
                  </span>
                </li>
              )
            })}
          </ol>
        </Card>
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
    </div>
  )
}
