import { StoreIcon, UploadIcon } from "lucide-react"

import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog"
import { useT } from "@/lib/i18n"

/** Pilihan sumber pasang: toko official atau berkas lokal (ADR 0028). */
export function InstallChooserDialog({ open, onOpenChange, onStore, onUpload }: { open: boolean; onOpenChange: (o: boolean) => void; onStore: () => void; onUpload: () => void }) {
  const t = useT()
  const option = "flex w-full items-start gap-3 rounded-xl border p-4 text-left transition-colors hover:bg-muted/50 focus-visible:ring-3 focus-visible:ring-ring/50 focus-visible:outline-none"
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>{t("Pasang extension")}</DialogTitle>
          <DialogDescription>{t("Pilih sumbernya.")}</DialogDescription>
        </DialogHeader>
        <div className="grid gap-2">
          <button type="button" className={option} onClick={onStore}>
            <StoreIcon className="mt-0.5 size-5 shrink-0 text-brand" />
            <span>
              <span className="block text-sm font-medium">{t("Dari toko")}</span>
              <span className="block text-xs text-muted-foreground">{t("Jelajahi extension official Teliti, cari, dan pasang. Dependensi ikut terpasang.")}</span>
            </span>
          </button>
          <button type="button" className={option} onClick={onUpload}>
            <UploadIcon className="mt-0.5 size-5 shrink-0 text-brand" />
            <span>
              <span className="block text-sm font-medium">{t("Unggah berkas lokal")}</span>
              <span className="block text-xs text-muted-foreground">{t("Pilih berkas .tmx dari komputer Anda. Tandatangannya diperiksa sebelum dipasang.")}</span>
            </span>
          </button>
        </div>
      </DialogContent>
    </Dialog>
  )
}
