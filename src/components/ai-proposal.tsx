import * as React from "react"
import { CheckIcon, Loader2Icon, LayoutDashboardIcon } from "lucide-react"
import { toast } from "sonner"

import { applyLayoutOps, describeOps } from "@/components/dashboard/ops"
import { WIDGET_BY_TYPE } from "@/components/dashboard/registry"
import { Button } from "@/components/ui/button"
import { useUpdateGate } from "@/hooks/use-write-gate"
import { DEFAULT_WIDGETS, notifyDashboardChanged, sameLayout } from "@/lib/dashboard"
import { useT } from "@/lib/i18n"
import { href } from "@/lib/route"
import { useStore } from "@/lib/store-context"
import type { AiWidget, DashboardWidget } from "@/lib/types"

type Proposal = Extract<AiWidget, { kind: "proposal" }>

const known = (ws: DashboardWidget[]) => ws.filter((w) => WIDGET_BY_TYPE.has(w.type))

/**
 * Usulan perubahan tata letak dari AI (ADR 0027). AI tidak mengubah apa pun: perubahan baru disimpan saat
 * pengguna menekan Terapkan, memakai izin Ubah Ringkasan miliknya sendiri, setelah UI memeriksa ulang usulannya.
 */
export function AiProposalCard({ widget, onPatch }: { widget: Proposal; onPatch: (patch: Partial<Proposal>) => void }) {
  const t = useT()
  const { api, routers } = useStore()
  const reason = useUpdateGate("ringkasan")()
  const [current, setCurrent] = React.useState<DashboardWidget[] | null>(null)
  const [busy, setBusy] = React.useState(false)
  const [error, setError] = React.useState<string | null>(null)
  const done = widget.state

  // Tata letak sekarang dipakai menamai widget yang dihapus atau diubah.
  React.useEffect(() => {
    let cancelled = false
    api
      .getDashboard()
      .then((l) => !cancelled && setCurrent(known(l ? l.widgets : DEFAULT_WIDGETS)))
      .catch(() => !cancelled && setCurrent(DEFAULT_WIDGETS))
    return () => {
      cancelled = true
    }
  }, [api])

  async function apply() {
    setBusy(true)
    setError(null)
    try {
      const stored = await api.getDashboard()
      const base = known(stored ? stored.widgets : DEFAULT_WIDGETS)
      const { widgets, problems } = applyLayoutOps(base, widget.ops, routers.map((r) => r.id))
      if (problems.length) throw new Error(t("Usulan tidak bisa diterapkan lagi: {reason}. Minta AI membuat usulan baru.", { reason: problems.join("; ") }))
      // Sama dengan susunan bawaan: hapus tata letak tersimpan supaya ikut bawaan.
      await api.saveDashboard(sameLayout(widgets, DEFAULT_WIDGETS) ? null : { version: 1, widgets })
      notifyDashboardChanged()
      onPatch({ state: "applied" })
      toast.success(t("Tata letak disimpan untuk semua akun"))
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
    } finally {
      setBusy(false)
    }
  }

  return (
    <div className="rounded-lg border bg-background p-2.5">
      <div className="mb-1.5 flex items-center gap-1.5 text-xs font-medium">
        <LayoutDashboardIcon className="size-3.5" aria-hidden /> {t("Usulan perubahan tata letak")}
      </div>
      <ul className="mb-2 list-disc space-y-0.5 pl-5 text-sm">
        {describeOps(widget.ops, current ?? [], routers).map((line, i) => (
          <li key={i}>{line}</li>
        ))}
      </ul>
      {done === "applied" ? (
        <p className="flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
          <span className="flex items-center gap-1 text-foreground">
            <CheckIcon className="size-3.5 text-good" aria-hidden /> {t("Sudah diterapkan")}
          </span>
          <a href={href("ringkasan")} className="underline underline-offset-2">
            {t("Buka Ringkasan")}
          </a>
        </p>
      ) : done === "dismissed" ? (
        <p className="text-xs text-muted-foreground">{t("Diabaikan")}</p>
      ) : (
        <>
          <p className="mb-2 text-xs text-muted-foreground">{reason ?? t("Belum ada yang berubah sampai kamu menekan Terapkan. Berlaku untuk semua akun.")}</p>
          {error && (
            <p role="alert" className="mb-2 text-xs text-destructive">
              {error}
            </p>
          )}
          <div className="flex gap-1.5">
            <Button type="button" size="sm" onClick={() => void apply()} disabled={busy || !!reason}>
              {busy && <Loader2Icon className="animate-spin" />}
              {t("Terapkan")}
            </Button>
            <Button type="button" size="sm" variant="ghost" onClick={() => onPatch({ state: "dismissed" })} disabled={busy}>
              {t("Abaikan")}
            </Button>
          </div>
        </>
      )}
    </div>
  )
}
