import * as React from "react"
import { LayoutDashboardIcon, Loader2Icon, PlusIcon, RotateCcwIcon } from "lucide-react"
import { toast } from "sonner"

import { AddWidgetDialog } from "@/components/dashboard/add-widget-dialog"
import { DashboardGrid, DashboardStack } from "@/components/dashboard/grid"
import { WIDGET_BY_TYPE } from "@/components/dashboard/registry"
import { PageSkeleton } from "@/components/skeletons"
import { Button } from "@/components/ui/button"
import { useDashboard } from "@/hooks/use-dashboard"
import { useIsDesktop } from "@/hooks/use-media-query"
import { useUpdateGate } from "@/hooks/use-write-gate"
import { DEFAULT_WIDGETS, MAX_WIDGETS, newWidgetId, nextY, sameLayout } from "@/lib/dashboard"
import { useT } from "@/lib/i18n"
import type { DashboardWidget } from "@/lib/types"

/**
 * Ringkasan = kumpulan widget dengan satu tata letak untuk semua akun. Akun berizin Ubah Ringkasan bisa
 * menggeser, mengubah ukuran, menambah, dan menghapus widget (hanya di layar md ke atas); lainnya hanya melihat.
 */
export function OverviewPage({ onOpenUser }: { onOpenUser: (id: string) => void }) {
  const t = useT()
  const isDesktop = useIsDesktop()
  const editReason = useUpdateGate("ringkasan")()
  const { widgets, save, reset } = useDashboard()
  const [draft, setDraft] = React.useState<DashboardWidget[] | null>(null)
  const [adding, setAdding] = React.useState(false)
  const [busy, setBusy] = React.useState(false)

  const canEdit = isDesktop && !editReason
  const editing = canEdit && draft !== null

  if (!widgets) return <PageSkeleton page="ringkasan" />

  async function onSave() {
    if (!draft) return
    setBusy(true)
    try {
      // Sama dengan susunan bawaan: hapus tata letak tersimpan supaya ikut bawaan.
      if (sameLayout(draft, DEFAULT_WIDGETS)) await reset()
      else await save(draft)
      toast.success(t("Tata letak disimpan untuk semua akun"))
      setDraft(null)
    } catch (e) {
      toast.error(e instanceof Error ? e.message : String(e))
    } finally {
      setBusy(false)
    }
  }

  function addWidget(type: string) {
    const def = WIDGET_BY_TYPE.get(type)
    if (!def) return
    setDraft((d) => (d ? [...d, { id: newWidgetId(type), type, x: 0, y: nextY(d), w: def.w, h: def.h, ...(def.defaultConfig ? { config: { ...def.defaultConfig } } : {}) }] : d))
    setAdding(false)
    toast.success(t("Widget ditambahkan di bagian bawah"))
  }

  return (
    <div className="flex flex-col gap-3">
      {canEdit &&
        (editing ? (
          <div className="sticky top-12 z-20 flex flex-wrap items-center gap-2 rounded-xl border bg-background/95 p-2 shadow-sm backdrop-blur md:top-9">
            <p className="min-w-0 flex-1 text-sm text-muted-foreground">
              {t("Mode ubah: geser widget, tarik sudut kanan bawah untuk mengubah ukuran. Berlaku untuk semua akun.")}
            </p>
            <Button type="button" variant="outline" size="sm" onClick={() => setAdding(true)} disabled={busy}>
              <PlusIcon /> {t("Tambah widget")}
            </Button>
            <Button type="button" variant="outline" size="sm" onClick={() => setDraft(DEFAULT_WIDGETS)} disabled={busy}>
              <RotateCcwIcon /> {t("Susunan bawaan")}
            </Button>
            <Button type="button" variant="ghost" size="sm" onClick={() => setDraft(null)} disabled={busy}>
              {t("Batal")}
            </Button>
            <Button type="button" size="sm" onClick={() => void onSave()} disabled={busy}>
              {busy && <Loader2Icon className="animate-spin" />}
              {t("Simpan")}
            </Button>
          </div>
        ) : (
          <div className="flex justify-end">
            <Button type="button" variant="outline" size="sm" onClick={() => setDraft(widgets)}>
              <LayoutDashboardIcon /> {t("Ubah tata letak")}
            </Button>
          </div>
        ))}

      {isDesktop ? (
        <DashboardGrid
          widgets={editing ? draft : widgets}
          editing={editing}
          onChange={setDraft}
          onRemove={(id) => setDraft((d) => (d ? d.filter((w) => w.id !== id) : d))}
          onConfigure={(id, config) => setDraft((d) => (d ? d.map((w) => (w.id === id ? { ...w, config } : w)) : d))}
          onOpenUser={onOpenUser}
        />
      ) : (
        <DashboardStack widgets={widgets} onOpenUser={onOpenUser} />
      )}

      {editing && draft.length === 0 && <p className="py-8 text-center text-sm text-muted-foreground">{t("Belum ada widget. Tambahkan dari tombol di atas, atau pakai susunan bawaan.")}</p>}

      <AddWidgetDialog open={adding} onOpenChange={setAdding} widgets={draft ?? []} full={(draft?.length ?? 0) >= MAX_WIDGETS} onAdd={addWidget} />
    </div>
  )
}
