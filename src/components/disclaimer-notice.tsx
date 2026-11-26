import * as React from "react"
import { ShieldAlertIcon } from "lucide-react"

import { Button } from "@/components/ui/button"
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog"
import { DISCLAIMER_URL } from "@/lib/app-info"
import { useT } from "@/lib/i18n"

/** Naikkan nilainya bila isi penafian berubah supaya semua pemakai diminta membaca lagi. */
export const DISCLAIMER_VERSION = "2026-11"
const KEY = "telemon.disclaimer.accepted"

function accepted() {
  try {
    return localStorage.getItem(KEY) === DISCLAIMER_VERSION
  } catch {
    return false
  }
}

/**
 * Pemberitahuan penafian sekali per browser atau aplikasi (ADR 0030): tanpa jaminan dan tanpa tanggung jawab, risiko yang
 * khas Telemon, dan bahwa extension punya lisensinya sendiri. Harus ditutup dengan tombol; tidak bisa dilewati dengan Esc
 * atau klik di luar. Pilihan hanya disimpan di perangkat ini, jadi akun lain atau browser lain melihatnya sendiri.
 */
export function DisclaimerNotice() {
  const t = useT()
  const [open, setOpen] = React.useState(() => !accepted())
  if (!open) return null
  const accept = () => {
    try {
      localStorage.setItem(KEY, DISCLAIMER_VERSION)
    } catch {
      /* penyimpanan diblokir: pemberitahuan muncul lagi di kunjungan berikutnya */
    }
    setOpen(false)
  }
  return (
    <Dialog open onOpenChange={() => {}}>
      <DialogContent showCloseButton={false} className="sm:max-w-lg" onEscapeKeyDown={(e) => e.preventDefault()} onInteractOutside={(e) => e.preventDefault()}>
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <ShieldAlertIcon className="size-5 text-warning" /> {t("Sebelum memakai Telemon")}
          </DialogTitle>
          <DialogDescription>{t("Telemon disediakan apa adanya, tanpa jaminan. Baca ini sebelum melanjutkan.")}</DialogDescription>
        </DialogHeader>
        <ul className="grid gap-2 pl-5 text-sm [&>li]:list-disc">
          <li>{t("Sejauh diizinkan hukum, Teliti, penulis, dan kontributor tidak bertanggung jawab atas kerugian akibat pemakaian Telemon, misalnya data hilang, terhapus, atau bocor, pembobolan keamanan, salah konfigurasi router, atau gangguan layanan.")}</li>
          <li>{t("Telemon bisa mengubah router. Baca dialog tinjauan sebelum menerapkan, dan simpan cadanganmu sendiri.")}</li>
          <li>{t("Kamu bertanggung jawab mengamankan akses: kata sandi kuat, 2FA, dan jaringan. Data pelanggan bisa berupa data pribadi.")}</li>
          <li>{t("Asisten AI mengirim data yang kamu tanyakan ke penyedia yang kamu pilih, dan jawabannya bisa salah.")}</li>
          <li>{t("Setiap extension punya lisensinya sendiri dari penerbitnya; Telemon tidak menjual atau menegakkannya.")}</li>
        </ul>
        <DialogFooter className="items-center sm:justify-between">
          <a href={DISCLAIMER_URL} target="_blank" rel="noopener noreferrer" className="text-sm text-brand underline underline-offset-2">
            {t("Baca penafian lengkap")}
          </a>
          <Button onClick={accept}>{t("Saya mengerti")}</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
