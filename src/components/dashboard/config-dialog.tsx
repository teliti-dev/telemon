import * as React from "react"

import type { ConfigProps, WidgetConfig, WidgetDef } from "@/components/dashboard/widget-types"
import { Button } from "@/components/ui/button"
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog"
import { useT } from "@/lib/i18n"
import type { DashboardWidget } from "@/lib/types"

/** Pengaturan satu widget (sumber data, rentang). Perubahan baru berlaku setelah Terapkan, lalu ikut disimpan bersama tata letak. */
export function WidgetConfigDialog({ widget, def, onClose, onApply }: { widget: DashboardWidget | null; def: WidgetDef | null; onClose: () => void; onApply: (id: string, config: WidgetConfig) => void }) {
  return (
    <Dialog open={!!widget && !!def?.Config} onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="sm:max-w-md">{widget && def?.Config && <Body key={widget.id} widget={widget} def={def} Form={def.Config} onClose={onClose} onApply={onApply} />}</DialogContent>
    </Dialog>
  )
}

/** Isi dialog; `key` per widget membuat draf mulai dari pengaturan widget yang dibuka. */
function Body({ widget, def, Form, onClose, onApply }: { widget: DashboardWidget; def: WidgetDef; Form: React.ComponentType<ConfigProps>; onClose: () => void; onApply: (id: string, config: WidgetConfig) => void }) {
  const t = useT()
  const [draft, setDraft] = React.useState<WidgetConfig>(() => ({ ...(def.defaultConfig ?? {}), ...(widget.config ?? {}) }))
  return (
    <>
      <DialogHeader>
        <DialogTitle>{t(def.title) /* i18n-ignore: kunci tk */}</DialogTitle>
        <DialogDescription>{t("Sumber data dan tampilan widget ini.")}</DialogDescription>
      </DialogHeader>
      <Form config={draft} onChange={setDraft} />
      <DialogFooter>
        <Button type="button" variant="outline" onClick={onClose}>
          {t("Batal")}
        </Button>
        <Button
          type="button"
          onClick={() => {
            onApply(widget.id, draft)
            onClose()
          }}
        >
          {t("Terapkan")}
        </Button>
      </DialogFooter>
    </>
  )
}
