import { PlusIcon } from "lucide-react"

import { WIDGETS } from "@/components/dashboard/registry"
import { Button } from "@/components/ui/button"
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog"
import { useT } from "@/lib/i18n"
import type { DashboardWidget } from "@/lib/types"

/** Daftar widget yang bisa ditambahkan; widget yang sama boleh ditambahkan lebih dari sekali. */
export function AddWidgetDialog({
  open,
  onOpenChange,
  widgets,
  full,
  onAdd,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  widgets: DashboardWidget[]
  full: boolean
  onAdd: (type: string) => void
}) {
  const t = useT()
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-h-[88svh] overflow-y-auto sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>{t("Tambah widget")}</DialogTitle>
          <DialogDescription>{full ? t("Jumlah widget sudah mencapai batas.") : t("Widget baru muncul di bagian bawah; geser ke tempat yang diinginkan.")}</DialogDescription>
        </DialogHeader>
        <ul className="grid gap-2">
          {WIDGETS.map((d) => {
            const n = widgets.filter((w) => w.type === d.type).length
            return (
              <li key={d.type} className="flex items-center gap-3 rounded-lg border p-3">
                <d.icon className="size-5 shrink-0 text-muted-foreground" />
                <div className="min-w-0 flex-1">
                  <div className="truncate text-sm font-medium">
                    {/* i18n-ignore: kunci tk */}
                    {t(d.title)}
                    {n > 0 && <span className="ml-2 text-xs font-normal text-muted-foreground">{t("{n} terpasang", { n })}</span>}
                  </div>
                  {/* i18n-ignore: kunci tk */}
                  <p className="text-xs text-muted-foreground">{t(d.description)}</p>
                </div>
                <Button type="button" size="sm" variant="outline" disabled={full} onClick={() => onAdd(d.type)}>
                  <PlusIcon /> {t("Tambah")}
                </Button>
              </li>
            )
          })}
        </ul>
      </DialogContent>
    </Dialog>
  )
}
