import * as React from "react"
import { CheckCircle2Icon, CircleAlertIcon, CircleDashedIcon, Loader2Icon, PencilIcon, PlugZapIcon, PlusIcon, StarIcon, Trash2Icon } from "lucide-react"
import { toast } from "sonner"

import { AiProviderDialog, PROVIDER_KIND_LABEL } from "@/components/ai-provider-dialog"
import { ConfirmDialog } from "@/components/confirm-dialog"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Card, CardAction, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Skeleton } from "@/components/ui/skeleton"
import { Switch } from "@/components/ui/switch"
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group"
import { useNow } from "@/hooks/use-now"
import { useIsDesktop } from "@/hooks/use-media-query"
import { useUpdateGate } from "@/hooks/use-write-gate"
import { formatCompact, formatDateTime, formatNumber, formatRelative, formatUsd } from "@/lib/format"
import { useT } from "@/lib/i18n"
import { navigatePath } from "@/lib/route"
import { useStore } from "@/lib/store-context"
import type { AiDashboard, AiProvider, AiUsageDay, AiUsageRow } from "@/lib/types"
import { cn } from "@/lib/utils"

const errMsg = (e: unknown) => (e instanceof Error ? e.message : String(e))
const dayLabel = (ts: number) => new Date(ts).toLocaleDateString(undefined, { day: "numeric", month: "short" })

function Tile({ label, value, hint, children }: { label: string; value: React.ReactNode; hint?: React.ReactNode; children?: React.ReactNode }) {
  return (
    <div className="min-w-0 rounded-lg border p-3">
      <div className="text-xs text-muted-foreground">{label}</div>
      <div className="mt-0.5 truncate text-xl font-semibold tabular-nums">{value}</div>
      {hint && <div className="mt-0.5 text-xs text-muted-foreground">{hint}</div>}
      {children}
    </div>
  )
}

/** Meter anggaran: brand normal, kuning dengan label saat mendekati batas, merah saat lewat. */
function BudgetMeter({ spent, limit, alert }: { spent: number; limit: number; alert: number }) {
  const t = useT()
  const pct = (spent / limit) * 100
  const state = pct >= 100 ? "over" : pct >= alert ? "near" : "ok"
  return (
    <div className="mt-2">
      <div
        role="progressbar"
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={Math.min(100, Math.round(pct))}
        aria-label={t("Pemakaian anggaran bulan ini")}
        className="h-1.5 overflow-hidden rounded-full bg-muted"
      >
        <div
          className={cn("h-full rounded-full", state === "ok" ? "bg-brand" : state === "near" ? "bg-warning" : "bg-critical")}
          style={{ width: `${Math.min(100, pct)}%` }}
        />
      </div>
      <div className="mt-1 text-xs text-muted-foreground">
        {state === "over" ? t("Batas terlampaui") : state === "near" ? t("Mendekati batas") : t("{pct}% dari batas", { pct: Math.round(pct) })}
      </div>
    </div>
  )
}

function TokenChart({ usage }: { usage: AiUsageDay[] }) {
  const t = useT()
  const [active, setActive] = React.useState<number | null>(null)
  const [view, setView] = React.useState<"chart" | "table">("chart")
  const max = Math.max(1, ...usage.map((u) => u.inputTokens + u.outputTokens))
  const sel = active !== null ? usage[active] : null
  const total = usage.reduce((s, u) => s + u.inputTokens + u.outputTokens, 0)
  return (
    <div className="grid gap-2">
      <div className="flex items-center justify-between gap-2">
        <div className="min-w-0 text-sm" aria-live="polite">
          {sel ? (
            <>
              <span className="font-medium">{dayLabel(sel.day)}</span>
              <span className="text-muted-foreground">
                {" · "}
                {t("{n} token", { n: formatCompact(sel.inputTokens + sel.outputTokens) })}
                {" · "}
                {formatUsd(sel.costUsd)}
                {" · "}
                {t("{n} permintaan", { n: formatNumber(sel.requests) })}
              </span>
            </>
          ) : (
            <span className="text-muted-foreground">{t("Total {n} token; arahkan ke batang untuk rincian.", { n: formatCompact(total) })}</span>
          )}
        </div>
        <ToggleGroup type="single" size="sm" value={view} onValueChange={(v) => v && setView(v as "chart" | "table")} aria-label={t("Tampilan")}>
          <ToggleGroupItem value="chart">{t("Grafik")}</ToggleGroupItem>
          <ToggleGroupItem value="table">{t("Tabel")}</ToggleGroupItem>
        </ToggleGroup>
      </div>
      {view === "chart" ? (
        <div className="relative">
          <div className="pointer-events-none absolute inset-x-0 top-0 flex items-center gap-2 text-[11px] text-muted-foreground">
            <span className="rounded bg-background/80 pr-1">{formatCompact(max)}</span>
            <span className="h-px flex-1 border-t border-dashed" />
          </div>
          <div className="flex h-40 items-end gap-0.5 border-b pt-4" onMouseLeave={() => setActive(null)}>
            {usage.map((u, i) => {
              const v = u.inputTokens + u.outputTokens
              return (
                <button
                  key={u.day}
                  type="button"
                  aria-label={t("{day}: {n} token, {cost}", { day: dayLabel(u.day), n: formatCompact(v), cost: formatUsd(u.costUsd) })}
                  onMouseEnter={() => setActive(i)}
                  onFocus={() => setActive(i)}
                  onBlur={() => setActive(null)}
                  className="group flex h-full min-w-0 flex-1 items-end outline-none"
                >
                  <span
                    className={cn("block w-full rounded-t-sm bg-brand/70 group-hover:bg-brand group-focus-visible:bg-brand", active === i && "bg-brand")}
                    style={{ height: `${Math.max(2, (v / max) * 100)}%` }}
                  />
                </button>
              )
            })}
          </div>
          <div className="mt-1 flex justify-between text-[11px] text-muted-foreground">
            <span>{dayLabel(usage[0].day)}</span>
            <span>{dayLabel(usage[usage.length - 1].day)}</span>
          </div>
        </div>
      ) : (
        <div className="max-h-64 overflow-auto rounded-lg border">
          <table className="w-full text-[13px] tabular-nums">
            <thead className="sticky top-0 bg-muted text-xs text-muted-foreground">
              <tr>
                <th className="px-2 py-1 text-left font-medium">{t("Tanggal")}</th>
                <th className="px-2 py-1 text-right font-medium">{t("Masuk")}</th>
                <th className="px-2 py-1 text-right font-medium">{t("Keluar")}</th>
                <th className="px-2 py-1 text-right font-medium">{t("Permintaan")}</th>
                <th className="px-2 py-1 text-right font-medium">{t("Biaya")}</th>
              </tr>
            </thead>
            <tbody>
              {[...usage].reverse().map((u) => (
                <tr key={u.day} className="border-t">
                  <td className="px-2 py-1">{dayLabel(u.day)}</td>
                  <td className="px-2 py-1 text-right">{formatCompact(u.inputTokens)}</td>
                  <td className="px-2 py-1 text-right">{formatCompact(u.outputTokens)}</td>
                  <td className="px-2 py-1 text-right">{formatNumber(u.requests)}</td>
                  <td className="px-2 py-1 text-right">{formatUsd(u.costUsd)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  )
}

function Breakdown({ title, rows }: { title: string; rows: AiUsageRow[] }) {
  const t = useT()
  const max = Math.max(1, ...rows.map((r) => r.inputTokens + r.outputTokens))
  return (
    <div className="grid gap-2">
      <h4 className="text-xs font-medium text-muted-foreground">{title}</h4>
      <ul className="grid gap-2">
        {rows.map((r) => {
          const v = r.inputTokens + r.outputTokens
          return (
            <li key={r.key} className="grid gap-1 text-[13px]">
              <div className="flex items-baseline justify-between gap-2">
                <span className="min-w-0 truncate font-mono">{r.label}</span>
                <span className="shrink-0 text-xs text-muted-foreground tabular-nums">
                  {t("{n} token", { n: formatCompact(v) })} · {formatUsd(r.costUsd)}
                </span>
              </div>
              <div className="h-1.5 overflow-hidden rounded-full bg-muted">
                <div className="h-full rounded-full bg-brand/70" style={{ width: `${(v / max) * 100}%` }} />
              </div>
            </li>
          )
        })}
      </ul>
    </div>
  )
}

function ProviderStatus({ p }: { p: AiProvider }) {
  const t = useT()
  if (p.status === "ok") {
    return (
      <span className="inline-flex items-center gap-1 text-xs">
        <CheckCircle2Icon className="size-3.5 text-good" /> {t("Terhubung")}
        {p.latencyMs ? <span className="text-muted-foreground">· {formatNumber(p.latencyMs)} ms</span> : null}
      </span>
    )
  }
  if (p.status === "error") {
    return (
      <span className="inline-flex items-center gap-1 text-xs">
        <CircleAlertIcon className="size-3.5 text-critical" /> {t("Error")}
      </span>
    )
  }
  return (
    <span className="inline-flex items-center gap-1 text-xs text-muted-foreground">
      <CircleDashedIcon className="size-3.5" /> {t("Belum diuji")}
    </span>
  )
}

export function AiDashboard() {
  const t = useT()
  const { api } = useStore()
  const isDesktop = useIsDesktop()
  const now = useNow(60_000)
  const reason = useUpdateGate("pengaturan")()
  const [days, setDays] = React.useState<7 | 30>(30)
  const [data, setData] = React.useState<AiDashboard | null>(null)
  const [error, setError] = React.useState<string | null>(null)
  const [busyId, setBusyId] = React.useState<string | null>(null)
  const [dialog, setDialog] = React.useState<{ provider?: AiProvider } | null>(null)
  const [removing, setRemoving] = React.useState<AiProvider | null>(null)
  const [limit, setLimit] = React.useState("")
  const [alertPct, setAlertPct] = React.useState("80")
  const [hardStop, setHardStop] = React.useState(false)
  const [savingBudget, setSavingBudget] = React.useState(false)

  const load = React.useCallback(async () => {
    try {
      const d = await api.getAiDashboard(days)
      setData(d)
      setError(null)
      setLimit(d.budget.monthlyUsd === null ? "" : String(d.budget.monthlyUsd))
      setAlertPct(String(d.budget.alertPercent))
      setHardStop(d.budget.hardStop)
    } catch (e) {
      setError(errMsg(e))
    }
  }, [api, days])

  React.useEffect(() => {
    // eslint-disable-next-line react-hooks/set-state-in-effect -- memuat data dari adapter
    void load()
  }, [load])

  async function run(id: string, fn: () => Promise<unknown>, ok?: string) {
    setBusyId(id)
    try {
      await fn()
      if (ok) toast.success(ok)
      await load()
    } catch (e) {
      toast.error(errMsg(e))
    } finally {
      setBusyId(null)
    }
  }

  const openForm = (provider?: AiProvider) =>
    isDesktop ? setDialog({ provider }) : navigatePath("pengaturan", ["ai-provider", ...(provider ? [provider.id, "edit"] : ["new"])])

  if (error) {
    return (
      <Card>
        <CardContent className="flex items-center justify-between gap-2 text-sm text-destructive">
          <span>{error}</span>
          <Button variant="outline" size="sm" onClick={() => void load()}>{t("Coba lagi")}</Button>
        </CardContent>
      </Card>
    )
  }
  if (!data) {
    return (
      <Card>
        <CardHeader><Skeleton className="h-5 w-40" /></CardHeader>
        <CardContent className="grid gap-3">
          <div className="grid grid-cols-2 gap-3 lg:grid-cols-4">{[0, 1, 2, 3].map((i) => <Skeleton key={i} className="h-20" />)}</div>
          <Skeleton className="h-44" />
        </CardContent>
      </Card>
    )
  }

  const { budget, providers, usage } = data
  const tokens = usage.reduce((s, u) => s + u.inputTokens + u.outputTokens, 0)
  const requests = usage.reduce((s, u) => s + u.requests, 0)
  const cost = usage.reduce((s, u) => s + u.costUsd, 0)
  const def = providers.find((p) => p.isDefault)
  const budgetDirty =
    (budget.monthlyUsd === null ? "" : String(budget.monthlyUsd)) !== limit.trim() || String(budget.alertPercent) !== alertPct || budget.hardStop !== hardStop
  const limitNum = limit.trim() === "" ? null : Number(limit)
  const alertNum = Number(alertPct)
  const budgetValid = (limitNum === null || (limitNum > 0 && limitNum <= 100000)) && Number.isInteger(alertNum) && alertNum >= 10 && alertNum <= 100

  return (
    <Card>
      <CardHeader>
        <CardTitle>{t("Dasbor AI")}</CardTitle>
        <CardDescription>{t("Provider, saldo, pemakaian token, dan anggaran asisten AI. Angka contoh sampai server tersambung ke provider.")}</CardDescription>
        <CardAction>
          <ToggleGroup type="single" size="sm" value={String(days)} onValueChange={(v) => v && setDays(Number(v) as 7 | 30)} aria-label={t("Rentang waktu")}>
            <ToggleGroupItem value="7">{t("7 hari")}</ToggleGroupItem>
            <ToggleGroupItem value="30">{t("30 hari")}</ToggleGroupItem>
          </ToggleGroup>
        </CardAction>
      </CardHeader>
      <CardContent className="grid gap-5">
        <div className="grid grid-cols-2 gap-3 lg:grid-cols-4">
          <Tile
            label={t("Saldo provider bawaan")}
            value={def?.balanceUsd == null ? "-" : formatUsd(def.balanceUsd)}
            hint={def ? (def.balanceUsd == null ? t("{name} tidak melaporkan saldo", { name: def.name }) : def.name) : t("Belum ada provider bawaan")}
          />
          <Tile label={t("Biaya bulan ini")} value={formatUsd(budget.spentMonthUsd)} hint={budget.monthlyUsd === null ? t("Tanpa batas") : t("dari {limit}", { limit: formatUsd(budget.monthlyUsd) })}>
            {budget.monthlyUsd !== null && <BudgetMeter spent={budget.spentMonthUsd} limit={budget.monthlyUsd} alert={budget.alertPercent} />}
          </Tile>
          <Tile label={t("Token ({n} hari)", { n: days })} value={formatCompact(tokens)} hint={`${formatUsd(cost)}`} />
          <Tile
            label={t("Permintaan ({n} hari)", { n: days })}
            value={formatNumber(requests)}
            hint={t("galat {pct}% · median {ms} ms", { pct: (data.errorRate * 100).toFixed(1), ms: formatNumber(data.p50Ms) })}
          />
        </div>

        <section className="grid gap-2" aria-label={t("Token per hari")}>
          <h3 className="text-sm font-medium">{t("Token per hari")}</h3>
          <TokenChart usage={usage} />
        </section>

        <div className="grid gap-5 md:grid-cols-2">
          <Breakdown title={t("Per model")} rows={data.byModel} />
          <Breakdown title={t("Per akun")} rows={data.byAccount} />
        </div>

        <section className="grid gap-2" aria-label={t("Provider")}>
          <div className="flex items-center justify-between gap-2">
            <h3 className="text-sm font-medium">{t("Provider")}</h3>
            <Button size="sm" variant="outline" disabled={!!reason} title={reason ?? undefined} onClick={() => openForm()}>
              <PlusIcon /> {t("Tambah provider")}
            </Button>
          </div>
          {providers.length === 0 && <p className="text-sm text-muted-foreground">{t("Belum ada provider. Tambahkan satu agar AI chat bisa memakai model sungguhan.")}</p>}
          <ul className="grid gap-2">
            {providers.map((p) => (
              <li key={p.id} className="grid gap-2 rounded-lg border p-3">
                <div className="flex flex-wrap items-center gap-x-2 gap-y-1">
                  <span className="font-medium">{p.name}</span>
                  {p.isDefault && <Badge variant="outline"><StarIcon /> {t("Bawaan")}</Badge>}
                  <ProviderStatus p={p} />
                </div>
                <dl className="grid grid-cols-2 gap-x-3 gap-y-1.5 text-[13px] sm:grid-cols-4">
                  <div className="min-w-0">
                    <dt className="text-xs text-muted-foreground">{t("Jenis")}</dt>
                    <dd className="truncate">{t(PROVIDER_KIND_LABEL[p.kind]) /* i18n-ignore: kunci tk */}</dd>
                  </div>
                  <div className="min-w-0">
                    <dt className="text-xs text-muted-foreground">{t("Model")}</dt>
                    <dd className="truncate font-mono text-xs">{p.model}</dd>
                  </div>
                  <div className="min-w-0">
                    <dt className="text-xs text-muted-foreground">{t("Kunci API")}</dt>
                    <dd className="truncate font-mono text-xs">{p.keyHint ?? "-"}</dd>
                  </div>
                  <div className="min-w-0">
                    <dt className="text-xs text-muted-foreground">{t("Saldo")}</dt>
                    <dd className="tabular-nums">{p.balanceUsd === null ? "-" : formatUsd(p.balanceUsd)}</dd>
                  </div>
                </dl>
                {p.status === "error" && p.error && <p className="text-xs text-destructive">{p.error}</p>}
                {p.checkedAt && <p className="text-xs text-muted-foreground" title={formatDateTime(p.checkedAt)}>{t("Terakhir diuji {when}", { when: formatRelative(p.checkedAt, now) })}</p>}
                <div className="flex flex-wrap gap-1.5">
                  <Button size="sm" variant="outline" disabled={busyId === p.id} onClick={() => void run(p.id, () => api.testAiProvider(p.id), t("Pengujian selesai"))}>
                    {busyId === p.id ? <Loader2Icon className="animate-spin" /> : <PlugZapIcon />} {t("Uji koneksi")}
                  </Button>
                  {!p.isDefault && (
                    <Button size="sm" variant="outline" disabled={!!reason || busyId === p.id} title={reason ?? undefined} onClick={() => void run(p.id, () => api.setDefaultAiProvider(p.id), t("Provider bawaan diganti"))}>
                      <StarIcon /> {t("Jadikan bawaan")}
                    </Button>
                  )}
                  <Button size="sm" variant="outline" disabled={!!reason} title={reason ?? undefined} onClick={() => openForm(p)}>
                    <PencilIcon /> {t("Edit")}
                  </Button>
                  <Button size="sm" variant="ghost" disabled={!!reason} title={reason ?? undefined} onClick={() => setRemoving(p)} aria-label={t("Hapus {name}", { name: p.name })}>
                    <Trash2Icon />
                  </Button>
                </div>
              </li>
            ))}
          </ul>
        </section>

        <section className="grid gap-3 rounded-lg border p-3" aria-label={t("Anggaran")}>
          <div>
            <h3 className="text-sm font-medium">{t("Anggaran bulanan")}</h3>
            <p className="text-xs text-muted-foreground">{t("Dihitung dari biaya token; direset {when}.", { when: formatDateTime(budget.resetAt) })}</p>
          </div>
          <div className="grid grid-cols-2 gap-3 sm:max-w-md">
            <div className="grid gap-1.5">
              <Label htmlFor="bg-limit">{t("Batas (USD)")}</Label>
              <Input id="bg-limit" inputMode="decimal" value={limit} placeholder={t("Tanpa batas")} disabled={!!reason} onChange={(e) => setLimit(e.target.value)} aria-invalid={limitNum !== null && !(limitNum > 0 && limitNum <= 100000)} />
            </div>
            <div className="grid gap-1.5">
              <Label htmlFor="bg-alert">{t("Peringatan di (%)")}</Label>
              <Input id="bg-alert" inputMode="numeric" value={alertPct} disabled={!!reason} onChange={(e) => setAlertPct(e.target.value)} aria-invalid={!(alertNum >= 10 && alertNum <= 100)} />
            </div>
          </div>
          <div className="flex items-center justify-between gap-4">
            <div className="grid gap-0.5">
              <Label htmlFor="bg-stop">{t("Hentikan AI saat batas tercapai")}</Label>
              <p className="text-xs text-muted-foreground">{t("Bila mati, AI tetap menjawab dan hanya memberi peringatan.")}</p>
            </div>
            <Switch id="bg-stop" checked={hardStop} disabled={!!reason} onCheckedChange={setHardStop} />
          </div>
          <div className="flex gap-2">
            <Button
              size="sm"
              disabled={!!reason || !budgetDirty || !budgetValid || savingBudget}
              title={reason ?? undefined}
              onClick={async () => {
                setSavingBudget(true)
                try {
                  await api.saveAiBudget({ monthlyUsd: limitNum, alertPercent: alertNum, hardStop })
                  toast.success(t("Anggaran disimpan"))
                  await load()
                } catch (e) {
                  toast.error(errMsg(e))
                } finally {
                  setSavingBudget(false)
                }
              }}
            >
              {savingBudget && <Loader2Icon className="animate-spin" />} {t("Simpan anggaran")}
            </Button>
            {budgetDirty && (
              <Button size="sm" variant="ghost" onClick={() => void load()}>{t("Batalkan")}</Button>
            )}
          </div>
        </section>
      </CardContent>

      <AiProviderDialog open={!!dialog} onOpenChange={(o) => !o && setDialog(null)} provider={dialog?.provider} onSaved={() => void load()} />
      <ConfirmDialog
        open={!!removing}
        onOpenChange={(o) => !o && setRemoving(null)}
        title={t("Hapus provider?")}
        description={removing ? t("{name} dan kunci API-nya dihapus dari Telemon. AI chat memakai provider bawaan lain.", { name: removing.name }) : ""}
        confirmLabel={t("Hapus")}
        destructive
        onConfirm={async () => {
          if (removing) await run(removing.id, () => api.removeAiProvider(removing.id), t("Provider dihapus"))
          setRemoving(null)
        }}
      />
    </Card>
  )
}
