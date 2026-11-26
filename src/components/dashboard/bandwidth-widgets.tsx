import * as React from "react"

import { TrafficChart } from "@/components/dashboard/traffic-chart"
import type { ConfigProps, WidgetConfig, WidgetProps } from "@/components/dashboard/widget-types"
import { Card, CardAction, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import { Label } from "@/components/ui/label"
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select"
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group"
import { formatBps, formatNumber } from "@/lib/format"
import { useT } from "@/lib/i18n"
import { href } from "@/lib/route"
import { useStore } from "@/lib/store-context"
import { PPPOE, RANGES, directions, ifaceLabel, rangeLabel, stats, useTrafficSeries } from "@/lib/traffic"
import type { TrafficInterface, TrafficPoint, TrafficRange } from "@/lib/types"

// ---------------- pengaturan widget ----------------

const str = (c: WidgetConfig | undefined, k: string, d: string) => (typeof c?.[k] === "string" ? (c[k] as string) : d)
const range = (c: WidgetConfig | undefined, d: TrafficRange): TrafficRange => (RANGES.includes(c?.range as TrafficRange) ? (c?.range as TrafficRange) : d)

export const BANDWIDTH_CHART_DEFAULT: WidgetConfig = { router: "all", iface: PPPOE, range: "6h" }
export const TOTAL_DEFAULT: WidgetConfig = { range: "6h" }
export const PER_USER_DEFAULT: WidgetConfig = { router: "all", range: "6h" }

function RouterSelect({ id, value, onChange }: { id: string; value: string; onChange: (v: string) => void }) {
  const t = useT()
  const { routers } = useStore()
  const known = value === "all" || routers.some((r) => r.id === value)
  return (
    <Select value={value} onValueChange={onChange}>
      <SelectTrigger id={id} className="w-full">
        <SelectValue />
      </SelectTrigger>
      <SelectContent>
        <SelectItem value="all">{t("Semua router")}</SelectItem>
        {routers.map((r) => (
          <SelectItem key={r.id} value={r.id}>
            {r.name}
          </SelectItem>
        ))}
        {!known && <SelectItem value={value}>{t("Router tidak ditemukan")}</SelectItem>}
      </SelectContent>
    </Select>
  )
}

function RangeSelect({ id, value, onChange }: { id: string; value: TrafficRange; onChange: (v: TrafficRange) => void }) {
  return (
    <Select value={value} onValueChange={(v) => onChange(v as TrafficRange)}>
      <SelectTrigger id={id} className="w-full">
        <SelectValue />
      </SelectTrigger>
      <SelectContent>
        {RANGES.map((r) => (
          <SelectItem key={r} value={r}>
            {rangeLabel(r)}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  )
}

export function BandwidthChartConfig({ config, onChange }: ConfigProps) {
  const t = useT()
  const { api } = useStore()
  const router = str(config, "router", "all")
  const iface = str(config, "iface", PPPOE)
  const [ifaces, setIfaces] = React.useState<TrafficInterface[] | null>(null)
  const [enabled, setEnabled] = React.useState(true)

  React.useEffect(() => {
    if (router === "all") return
    let cancelled = false
    api
      .getTrafficInterfaces(router)
      .then((r) => {
        if (cancelled) return
        setIfaces(r.interfaces)
        setEnabled(r.enabled)
      })
      .catch(() => !cancelled && setIfaces([]))
    return () => {
      cancelled = true
    }
  }, [api, router])

  const options = router === "all" ? [{ name: PPPOE }] : (ifaces ?? []).map((i) => ({ name: i.name }))
  if (!options.some((o) => o.name === iface)) options.push({ name: iface })
  const loading = router !== "all" && ifaces === null

  return (
    <div className="grid gap-4">
      <div className="grid gap-1.5">
        <Label htmlFor="bw-router">{t("Router")}</Label>
        <RouterSelect
          id="bw-router"
          value={router}
          onChange={(v) => {
            setIfaces(null)
            onChange({ ...config, router: v, iface: PPPOE })
          }}
        />
        {router === "all" && <p className="text-xs text-muted-foreground">{t("Total semua router hanya tersedia untuk antarmuka PPPoE (pelanggan); antarmuka fisik akan terhitung ganda.")}</p>}
      </div>
      <div className="grid gap-1.5">
        <Label htmlFor="bw-iface">{t("Antarmuka")}</Label>
        <Select value={iface} onValueChange={(v) => onChange({ ...config, iface: v })} disabled={loading || router === "all"}>
          <SelectTrigger id="bw-iface" className="w-full font-mono">
            <SelectValue placeholder={t("Memuat…")} />
          </SelectTrigger>
          <SelectContent>
            {options.map((o) => (
              <SelectItem key={o.name} value={o.name} className="font-mono">
                {ifaceLabel(o.name)}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
        {!enabled && <p className="text-xs text-muted-foreground">{t("Pengumpulan bandwidth sedang mati; nyalakan di Pengaturan > Bandwidth.")}</p>}
        {ifaces && ifaces.length === 0 && router !== "all" && <p className="text-xs text-muted-foreground">{t("Belum ada antarmuka terbaca untuk router ini.")}</p>}
      </div>
      <div className="grid gap-1.5">
        <Label htmlFor="bw-range">{t("Rentang waktu")}</Label>
        <RangeSelect id="bw-range" value={range(config, "6h")} onChange={(v) => onChange({ ...config, range: v })} />
      </div>
    </div>
  )
}

export function TotalBandwidthConfig({ config, onChange }: ConfigProps) {
  const t = useT()
  return (
    <div className="grid gap-1.5">
      <Label htmlFor="tb-range">{t("Rentang waktu")}</Label>
      <RangeSelect id="tb-range" value={range(config, "6h")} onChange={(v) => onChange({ ...config, range: v })} />
    </div>
  )
}

export function PerUserConfig({ config, onChange }: ConfigProps) {
  const t = useT()
  return (
    <div className="grid gap-4">
      <div className="grid gap-1.5">
        <Label htmlFor="pu-router">{t("Router")}</Label>
        <RouterSelect id="pu-router" value={str(config, "router", "all")} onChange={(v) => onChange({ ...config, router: v })} />
      </div>
      <div className="grid gap-1.5">
        <Label htmlFor="pu-range">{t("Rentang waktu")}</Label>
        <RangeSelect id="pu-range" value={range(config, "6h")} onChange={(v) => onChange({ ...config, range: v })} />
      </div>
    </div>
  )
}

// ---------------- keadaan kosong, galat, dimuat ----------------

function Notice({ children }: { children: React.ReactNode }) {
  return <p className="flex min-h-0 flex-1 items-center justify-center px-3 py-4 text-center text-sm text-muted-foreground">{children}</p>
}

/** Pesan pengganti grafik bila belum ada yang bisa digambar; null bila ada data. */
function useNotice(state: ReturnType<typeof useTrafficSeries>): React.ReactNode | null {
  const t = useT()
  if (state.error) return state.error
  if (state.loading || !state.series) return t("Memuat…")
  if (!state.series.enabled) {
    return (
      <>
        {t("Pengumpulan bandwidth mati.")}{" "}
        <a href={href("pengaturan")} className="underline underline-offset-2">
          {t("Nyalakan di Pengaturan")}
        </a>
      </>
    )
  }
  if (state.series.points.length === 0) return t("Belum ada data. Sampel pertama muncul dalam {n} detik.", { n: state.series.intervalSeconds })
  return null
}

function routerName(routers: { id: string; name: string }[], router: string, all: string, missing: string) {
  return router === "all" ? all : (routers.find((r) => r.id === router)?.name ?? missing)
}

function Stat({ label, values, solid }: { label: string; values: number[]; solid: boolean }) {
  const t = useT()
  const s = stats(values)
  return (
    <div className="min-w-0">
      <div className="flex items-center gap-1.5 text-xs text-muted-foreground">
        {solid ? <span className="inline-block h-0.5 w-3 bg-brand" aria-hidden /> : <span className="inline-block w-3 border-t-2 border-dashed border-foreground/60" aria-hidden />}
        {label}
      </div>
      <div className="truncate text-lg font-semibold tabular-nums">{formatBps(s.now)}</div>
      <div className="truncate text-xs text-muted-foreground tabular-nums">{t("rata-rata {avg} · puncak {peak}", { avg: formatBps(s.avg), peak: formatBps(s.peak) })}</div>
    </div>
  )
}

// ---------------- widget ----------------

export function BandwidthChartWidget({ widget }: WidgetProps) {
  const t = useT()
  const { routers } = useStore()
  const router = str(widget.config, "router", "all")
  const iface = str(widget.config, "iface", PPPOE)
  const cfgRange = range(widget.config, "6h")
  // Pilihan rentang di kartu hanya untuk pengamat ini; berubahnya pengaturan widget menggantikannya.
  const [pick, setPick] = React.useState<{ base: TrafficRange; sel: TrafficRange } | null>(null)
  const rangeSel = pick && pick.base === cfgRange ? pick.sel : cfgRange
  const setRangeSel = (sel: TrafficRange) => setPick({ base: cfgRange, sel })
  const state = useTrafficSeries({ router, iface, range: rangeSel })
  const notice = useNotice(state)
  const dir = directions(iface)
  const pts = (state.series?.points ?? []).map((p) => ({ at: p.at, ...dir.pick(p) }))
  return (
    <Card className="h-full min-h-0 gap-2 py-2.5">
      <CardHeader className="px-3">
        <CardTitle>{t("Bandwidth")}</CardTitle>
        <CardDescription className="truncate">
          {routerName(routers, router, t("Semua router"), t("Router tidak ditemukan"))} · <span className="font-mono">{ifaceLabel(iface)}</span>
        </CardDescription>
        <CardAction>
          <ToggleGroup type="single" size="sm" value={rangeSel} onValueChange={(v) => v && setRangeSel(v as TrafficRange)} aria-label={t("Rentang waktu")}>
            {(["1h", "6h", "24h", "7d"] as const).map((r) => (
              <ToggleGroupItem key={r} value={r}>
                {r}
              </ToggleGroupItem>
            ))}
          </ToggleGroup>
        </CardAction>
      </CardHeader>
      <CardContent className="flex min-h-0 flex-1 flex-col gap-2 px-3">
        {notice ? (
          <Notice>{notice}</Notice>
        ) : (
          <>
            <div className="grid grid-cols-2 gap-3">
              <Stat label={dir.a} values={pts.map((p) => p.a)} solid />
              <Stat label={dir.b} values={pts.map((p) => p.b)} solid={false} />
            </div>
            <TrafficChart className="min-h-[72px] flex-1" points={pts} from={state.series!.from} to={state.series!.to} bucketSeconds={state.series!.bucketSeconds} aLabel={dir.a} bLabel={dir.b} />
          </>
        )}
      </CardContent>
    </Card>
  )
}

export function TotalBandwidthWidget({ widget }: WidgetProps) {
  const t = useT()
  const r = range(widget.config, "6h")
  const state = useTrafficSeries({ router: "all", iface: PPPOE, range: r })
  const notice = useNotice(state)
  const dir = directions(PPPOE)
  const pts = (state.series?.points ?? []).map((p) => ({ at: p.at, ...dir.pick(p) }))
  const last: TrafficPoint | undefined = state.series?.points[state.series.points.length - 1]
  return (
    <Card className="h-full min-h-0 gap-2 py-2.5">
      <CardHeader className="px-3">
        <CardTitle>{t("Total bandwidth pelanggan")}</CardTitle>
        <CardDescription>
          {t("Semua router")} · {rangeLabel(r)}
          {last?.sessions !== undefined && ` · ${t("{n} sesi", { n: formatNumber(Math.round(last.sessions)) })}`}
        </CardDescription>
      </CardHeader>
      <CardContent className="flex min-h-0 flex-1 flex-col gap-2 px-3">
        {notice ? (
          <Notice>{notice}</Notice>
        ) : (
          <>
            <div className="grid grid-cols-2 gap-3">
              <Stat label={dir.a} values={pts.map((p) => p.a)} solid />
              <Stat label={dir.b} values={pts.map((p) => p.b)} solid={false} />
            </div>
            <TrafficChart compact className="min-h-[40px] flex-1" points={pts} from={state.series!.from} to={state.series!.to} bucketSeconds={state.series!.bucketSeconds} aLabel={dir.a} bLabel={dir.b} />
          </>
        )}
      </CardContent>
    </Card>
  )
}

export function AvgPerUserWidget({ widget }: WidgetProps) {
  const t = useT()
  const { routers } = useStore()
  const router = str(widget.config, "router", "all")
  const r = range(widget.config, "6h")
  const state = useTrafficSeries({ router, iface: PPPOE, range: r })
  const notice = useNotice(state)
  const dir = directions(PPPOE)
  // Per user = laju total sesi dibagi jumlah sesi pada ember yang sama; ember tanpa sesi dilewati.
  const pts = (state.series?.points ?? [])
    .filter((p) => (p.sessions ?? 0) > 0)
    .map((p) => {
      const d = dir.pick(p)
      const n = p.sessions as number
      return { at: p.at, a: d.a / n, b: d.b / n }
    })
  const last = state.series?.points[state.series.points.length - 1]
  return (
    <Card className="h-full min-h-0 gap-2 py-2.5">
      <CardHeader className="px-3">
        <CardTitle>{t("Rata-rata per user")}</CardTitle>
        <CardDescription className="truncate">
          {routerName(routers, router, t("Semua router"), t("Router tidak ditemukan"))} · {rangeLabel(r)}
          {last?.sessions !== undefined && ` · ${t("{n} sesi", { n: formatNumber(Math.round(last.sessions)) })}`}
        </CardDescription>
      </CardHeader>
      <CardContent className="flex min-h-0 flex-1 flex-col gap-2 px-3">
        {notice ? (
          <Notice>{notice}</Notice>
        ) : pts.length === 0 ? (
          <Notice>{t("Belum ada sesi PPPoE aktif pada rentang ini.")}</Notice>
        ) : (
          <>
            <div className="grid grid-cols-2 gap-3">
              <Stat label={dir.a} values={pts.map((p) => p.a)} solid />
              <Stat label={dir.b} values={pts.map((p) => p.b)} solid={false} />
            </div>
            <TrafficChart compact className="min-h-[36px] flex-1" points={pts} from={state.series!.from} to={state.series!.to} bucketSeconds={state.series!.bucketSeconds} aLabel={dir.a} bLabel={dir.b} />
          </>
        )}
      </CardContent>
    </Card>
  )
}

