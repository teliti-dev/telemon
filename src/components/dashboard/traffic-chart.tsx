import * as React from "react"

import { formatBps } from "@/lib/format"
import { useT } from "@/lib/i18n"
import { niceMax } from "@/lib/traffic"
import { cn } from "@/lib/utils"

export interface ChartPoint {
  at: number
  a: number
  b: number
}

interface Props {
  points: ChartPoint[]
  from: number
  to: number
  bucketSeconds: number
  aLabel: string
  bLabel: string
  /** Tanpa sumbu dan petunjuk (dipakai sebagai garis kecil di dalam kartu). */
  compact?: boolean
  className?: string
}

const PAD = { left: 52, right: 8, top: 8, bottom: 20 }

function timeLabel(at: number, span: number) {
  const d = new Date(at)
  return span > 36 * 3600_000
    ? d.toLocaleDateString(undefined, { day: "numeric", month: "short" })
    : d.toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" })
}

/** Garis dengan jeda: titik yang berjarak lebih dari 2,5 ember dianggap data hilang, garis tidak menyambungkannya. */
function segments(pts: { x: number; y: number; at: number }[], maxGap: number) {
  const out: { x: number; y: number }[][] = []
  let cur: { x: number; y: number }[] = []
  let last: number | null = null
  for (const p of pts) {
    if (last !== null && p.at - last > maxGap && cur.length) {
      out.push(cur)
      cur = []
    }
    cur.push(p)
    last = p.at
  }
  if (cur.length) out.push(cur)
  return out
}

const line = (s: { x: number; y: number }[]) => s.map((p, i) => `${i === 0 ? "M" : "L"}${p.x.toFixed(1)},${p.y.toFixed(1)}`).join("")

/**
 * Grafik area ringan (SVG): deret `a` solid dengan isi, deret `b` putus-putus. Dua deret dibedakan bentuk garisnya,
 * bukan hanya warna. Arahkan penunjuk untuk melihat nilai di satu titik.
 */
export function TrafficChart({ points, from, to, bucketSeconds, aLabel, bLabel, compact, className }: Props) {
  const t = useT()
  const ref = React.useRef<HTMLDivElement>(null)
  const [size, setSize] = React.useState({ w: 0, h: 0 })
  const [hover, setHover] = React.useState<number | null>(null)

  React.useEffect(() => {
    const el = ref.current
    if (!el) return
    const ro = new ResizeObserver(([e]) => setSize({ w: Math.floor(e.contentRect.width), h: Math.floor(e.contentRect.height) }))
    ro.observe(el)
    return () => ro.disconnect()
  }, [])

  const pad = compact ? { left: 2, right: 2, top: 2, bottom: 2 } : PAD
  const { w, h } = size
  const iw = Math.max(1, w - pad.left - pad.right)
  const ih = Math.max(1, h - pad.top - pad.bottom)
  const span = Math.max(1, to - from)
  const max = niceMax(Math.max(0, ...points.map((p) => Math.max(p.a, p.b))) * 1.08)
  const x = (at: number) => pad.left + ((at - from) / span) * iw
  const y = (v: number) => pad.top + ih - (v / max) * ih
  const maxGap = bucketSeconds * 1000 * 2.5
  const aPts = points.map((p) => ({ at: p.at, x: x(p.at), y: y(p.a) }))
  const bPts = points.map((p) => ({ at: p.at, x: x(p.at), y: y(p.b) }))
  const aSegs = segments(aPts, maxGap)
  const bSegs = segments(bPts, maxGap)
  const base = pad.top + ih

  const peak = points.reduce((m, p) => Math.max(m, p.a, p.b), 0)
  const last = points[points.length - 1]
  const summary = last ? t("{a} {av}, {b} {bv}, puncak {peak}", { a: aLabel, av: formatBps(last.a), b: bLabel, bv: formatBps(last.b), peak: formatBps(peak) }) : t("Tidak ada data")

  function onMove(e: React.PointerEvent<SVGSVGElement>) {
    if (compact || points.length === 0) return
    const rect = e.currentTarget.getBoundingClientRect()
    const px = e.clientX - rect.left
    let best = 0
    let bd = Infinity
    points.forEach((p, i) => {
      const d = Math.abs(x(p.at) - px)
      if (d < bd) {
        bd = d
        best = i
      }
    })
    setHover(best)
  }

  const hp = hover !== null ? points[hover] : null
  const ticks = compact ? [] : [0, 0.25, 0.5, 0.75, 1]
  const xTicks = compact ? [] : [0, 1 / 3, 2 / 3, 1]

  return (
    <div ref={ref} className={cn("relative min-h-0 w-full", className)}>
      {w > 0 && h > 0 && (
        <svg width={w} height={h} role="img" aria-label={summary} onPointerMove={onMove} onPointerLeave={() => setHover(null)} className="block touch-pan-y select-none">
          {ticks.map((f) => (
            <g key={f}>
              <line x1={pad.left} x2={w - pad.right} y1={y(max * f)} y2={y(max * f)} className="stroke-border" strokeWidth={1} strokeDasharray={f === 0 ? undefined : "2 3"} />
              <text x={pad.left - 6} y={y(max * f) + 3} textAnchor="end" className="fill-muted-foreground text-[10px] tabular-nums">
                {formatBps(max * f)}
              </text>
            </g>
          ))}
          {xTicks.map((f, i) => (
            <text key={f} x={pad.left + iw * f} y={h - 5} textAnchor={i === 0 ? "start" : i === xTicks.length - 1 ? "end" : "middle"} className="fill-muted-foreground text-[10px] tabular-nums">
              {timeLabel(from + span * f, span)}
            </text>
          ))}
          {aSegs.map((s, i) => (
            <g key={i}>
              {s.length > 1 && <path d={`${line(s)}L${s[s.length - 1].x.toFixed(1)},${base}L${s[0].x.toFixed(1)},${base}Z`} className="fill-brand/15" />}
              <path d={line(s)} className="stroke-brand" fill="none" strokeWidth={1.75} strokeLinejoin="round" />
              {s.length === 1 && <circle cx={s[0].x} cy={s[0].y} r={2} className="fill-brand" />}
            </g>
          ))}
          {bSegs.map((s, i) => (
            <path key={i} d={line(s)} className="stroke-foreground/60" fill="none" strokeWidth={1.5} strokeDasharray="4 3" strokeLinejoin="round" />
          ))}
          {hp && (
            <g pointerEvents="none">
              <line x1={x(hp.at)} x2={x(hp.at)} y1={pad.top} y2={base} className="stroke-foreground/30" />
              <circle cx={x(hp.at)} cy={y(hp.a)} r={3.5} className="fill-brand stroke-background" strokeWidth={1.5} />
              <circle cx={x(hp.at)} cy={y(hp.b)} r={3.5} className="fill-foreground/70 stroke-background" strokeWidth={1.5} />
            </g>
          )}
        </svg>
      )}
      {hp && !compact && (
        <div
          className="pointer-events-none absolute top-1 z-10 rounded-md border bg-popover px-2 py-1 text-xs shadow-md"
          style={{ left: Math.min(Math.max(x(hp.at) + 10, pad.left), Math.max(pad.left, w - 150)) }}
        >
          <div className="text-muted-foreground tabular-nums">{new Date(hp.at).toLocaleString()}</div>
          <div className="flex items-center gap-1.5 tabular-nums">
            <span className="inline-block h-0.5 w-3 bg-brand" /> {aLabel}: <span className="font-medium">{formatBps(hp.a)}</span>
          </div>
          <div className="flex items-center gap-1.5 tabular-nums">
            <span className="inline-block w-3 border-t-2 border-dashed border-foreground/60" /> {bLabel}: <span className="font-medium">{formatBps(hp.b)}</span>
          </div>
        </div>
      )}
    </div>
  )
}
