import * as React from "react"
import { DownloadIcon, XIcon } from "lucide-react"
import { toast } from "sonner"

import { ConfirmDialog } from "@/components/confirm-dialog"
import { Button } from "@/components/ui/button"
import { useApi } from "@/lib/api-context"
import { useT } from "@/lib/i18n"
import type { UpdateStatus } from "@/lib/types"

export const RELEASES_URL = "https://github.com/teliti-dev/telemon/releases/latest"
const SEEN_KEY = "telemon.update.dismissed"

const read = () => {
  try {
    return localStorage.getItem(SEEN_KEY)
  } catch {
    return null
  }
}

/** Hook status pembaruan (hanya aplikasi desktop). Memuat saat dibuka dan tiap 10 menit (server memeriksa tiap 24 jam). */
export function useUpdateStatus() {
  const { api, access } = useApi()
  const [status, setStatus] = React.useState<UpdateStatus | null>(null)
  React.useEffect(() => {
    if (access !== "desktop" || !api.getUpdateStatus) return
    let cancelled = false
    const load = () => api.getUpdateStatus!().then((s) => !cancelled && setStatus(s)).catch(() => undefined)
    void load()
    const id = setInterval(load, 10 * 60_000)
    return () => {
      cancelled = true
      clearInterval(id)
    }
  }, [api, access])
  return [status, setStatus] as const
}

/** Pasang pembaruan setelah konfirmasi. Dipakai banner dan kartu Pengaturan. */
export function InstallUpdateDialog({ version, open, onOpenChange }: { version: string; open: boolean; onOpenChange: (o: boolean) => void }) {
  const t = useT()
  const { api } = useApi()
  return (
    <ConfirmDialog
      open={open}
      onOpenChange={onOpenChange}
      title={t("Pasang Telemon {version}?", { version })}
      description={
        <p>
          {t("Basis data dicadangkan dulu, lalu pembaruan diunduh dan diverifikasi tanda tangannya. Aplikasi akan memulai ulang; pemantauan terhenti sebentar dan server Serve/Publik ikut berhenti.")}
        </p>
      }
      confirmLabel={t("Perbarui sekarang")}
      onConfirm={async () => {
        const r = await api.installUpdate!()
        toast.success(r.restarting ? t("Aplikasi sedang dimulai ulang…") : t("Pembaruan dipasang. Aplikasi akan dimulai ulang."))
      }}
    />
  )
}

/** Banner di atas halaman bila ada versi baru. Pengguna yang memutuskan; tidak pernah memasang otomatis. */
export function UpdateBanner() {
  const t = useT()
  const [status] = useUpdateStatus()
  const [dismissed, setDismissed] = React.useState<string | null>(read)
  const [open, setOpen] = React.useState(false)
  const v = status?.available?.version
  if (!status || !v || dismissed === v) return null
  const dismiss = () => {
    setDismissed(v)
    try {
      localStorage.setItem(SEEN_KEY, v)
    } catch {
      /* abaikan */
    }
  }
  return (
    <div role="status" className="border-b bg-brand/5">
      <div className="mx-auto flex max-w-7xl flex-wrap items-center gap-x-3 gap-y-1 px-4 py-2 text-sm">
        <DownloadIcon className="size-4 shrink-0 text-brand" />
        <span className="min-w-0 flex-1">
          {t("Telemon {version} tersedia", { version: v })}
          <span className="text-muted-foreground"> · {t("kamu memakai {current}", { current: status.currentVersion ?? "-" })}</span>
        </span>
        {status.canInstall ? (
          <Button size="sm" onClick={() => setOpen(true)}>
            {t("Perbarui")}
          </Button>
        ) : (
          <Button size="sm" variant="outline" asChild>
            <a href={RELEASES_URL} target="_blank" rel="noreferrer noopener">
              {t("Unduh")}
            </a>
          </Button>
        )}
        <Button size="icon-sm" variant="ghost" onClick={dismiss} aria-label={t("Sembunyikan")}>
          <XIcon />
        </Button>
      </div>
      <InstallUpdateDialog version={v} open={open} onOpenChange={setOpen} />
    </div>
  )
}
