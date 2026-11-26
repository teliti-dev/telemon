import { ArrowUpRightIcon } from "lucide-react"

import { AiProposalCard } from "@/components/ai-proposal"
import { useT } from "@/lib/i18n"
import type { AiLink, AiTone, AiWidget } from "@/lib/types"
import { cn } from "@/lib/utils"

const DOT: Record<AiTone, string> = {
  good: "bg-good",
  warn: "bg-serious",
  bad: "bg-critical",
  muted: "bg-muted-foreground/40",
}

function Dot({ tone }: { tone?: AiTone }) {
  return <span className={cn("size-2 shrink-0 rounded-full", DOT[tone ?? "muted"])} aria-hidden />
}

/** Komponen visual di dalam jawaban AI: kartu angka, grafik batang, daftar, dan kartu user. */
export function AiWidgetView({ widget, onNavigate, onPatch }: { widget: AiWidget; onNavigate: (to: AiLink) => void; onPatch?: (patch: Partial<AiWidget>) => void }) {
  const t = useT()

  if (widget.kind === "proposal") return <AiProposalCard widget={widget} onPatch={(p) => onPatch?.(p)} />

  if (widget.kind === "stats") {
    return (
      <div className="grid grid-cols-2 gap-2">
        {widget.items.map((it) => (
          <div key={it.label} className="rounded-lg border bg-background p-2">
            <div className="text-[11px] text-muted-foreground">{it.label}</div>
            <div className="flex items-center gap-1.5 text-lg font-semibold tabular-nums">
              <Dot tone={it.tone} />
              {it.value}
            </div>
          </div>
        ))}
      </div>
    )
  }

  if (widget.kind === "bars") {
    const max = Math.max(1, ...widget.data.map((d) => d.value))
    const series = widget.data.every((d) => /^\d{2}:\d{2}$/.test(d.label))
    const empty = widget.data.every((d) => d.value === 0)
    return (
      <div className="rounded-lg border bg-background p-2">
        <div className="mb-1.5 text-xs font-medium">{widget.title}</div>
        {empty ? (
          <p className="py-2 text-xs text-muted-foreground">{t("Tidak ada data")}</p>
        ) : series ? (
          <>
            <div
              role="img"
              aria-label={`${widget.title}: ${widget.data.map((d) => `${d.label} ${d.value}`).join(", ")}`}
              className="flex h-20 items-end gap-0.5"
            >
              {widget.data.map((d) => (
                <div
                  key={d.label}
                  title={`${d.label}: ${d.value}`}
                  className={cn("min-w-0 flex-1 rounded-t-sm", d.value > 0 ? "bg-brand/70" : "bg-muted")}
                  style={{ height: d.value > 0 ? `${Math.max(8, (d.value / max) * 100)}%` : "4%" }}
                />
              ))}
            </div>
            <div className="mt-1 flex justify-between text-[10px] text-muted-foreground tabular-nums">
              <span>{widget.data[0].label}</span>
              <span>{widget.data[widget.data.length - 1].label}</span>
            </div>
          </>
        ) : (
          <ul className="grid gap-1" aria-label={widget.title}>
            {widget.data.map((d) => (
              <li key={d.label} className="grid grid-cols-[5.5rem_1fr_2rem] items-center gap-2 text-xs">
                <span className="truncate font-mono">{d.label}</span>
                <span className="h-2 overflow-hidden rounded-full bg-muted">
                  <span className="block h-full rounded-full bg-brand/70" style={{ width: `${(d.value / max) * 100}%` }} />
                </span>
                <span className="text-right tabular-nums">{d.value}</span>
              </li>
            ))}
          </ul>
        )}
      </div>
    )
  }

  if (widget.kind === "list") {
    return (
      <div className="overflow-hidden rounded-lg border bg-background">
        {widget.title && <div className="border-b px-2 py-1.5 text-xs font-medium">{widget.title}</div>}
        <ul className="divide-y">
          {widget.rows.map((r, i) => {
            const body = (
              <>
                <Dot tone={r.tone} />
                <div className="min-w-0 flex-1 text-left">
                  <div className="truncate text-[13px] font-medium">{r.primary}</div>
                  {r.secondary && <div className="truncate text-xs text-muted-foreground">{r.secondary}</div>}
                </div>
                {r.trailing && <span className="shrink-0 text-xs text-muted-foreground tabular-nums">{r.trailing}</span>}
              </>
            )
            return (
              <li key={`${r.primary}-${i}`}>
                {r.to ? (
                  <button
                    type="button"
                    onClick={() => onNavigate(r.to!)}
                    className="flex w-full items-center gap-2 px-2 py-1.5 outline-none hover:bg-muted/60 focus-visible:bg-muted"
                  >
                    {body}
                  </button>
                ) : (
                  <div className="flex items-center gap-2 px-2 py-1.5">{body}</div>
                )}
              </li>
            )
          })}
        </ul>
      </div>
    )
  }

  return (
    <div className="rounded-lg border bg-background p-2">
      <div className="flex items-center justify-between gap-2">
        <div className="min-w-0">
          <div className="truncate font-mono text-sm font-medium">{widget.username}</div>
          <div className="truncate text-xs text-muted-foreground">{widget.router}</div>
        </div>
        <span className="flex shrink-0 items-center gap-1.5 rounded-full border px-2 py-0.5 text-xs">
          <Dot tone={widget.tone} />
          {widget.status}
        </span>
      </div>
      <dl className="mt-2 grid grid-cols-2 gap-x-2 gap-y-1.5 text-xs">
        {widget.fields.map((f) => (
          <div key={f.label} className="min-w-0">
            <dt className="text-muted-foreground">{f.label}</dt>
            <dd className="truncate font-mono">{f.value}</dd>
          </div>
        ))}
      </dl>
      <button
        type="button"
        onClick={() => onNavigate(widget.to)}
        className="mt-2 inline-flex items-center gap-1 text-xs font-medium text-brand underline-offset-4 hover:underline"
      >
        {t("Buka")} <ArrowUpRightIcon className="size-3.5" />
      </button>
    </div>
  )
}
