import * as React from "react"
import { DownloadIcon, Loader2Icon, RefreshCwIcon } from "lucide-react"
import { toast } from "sonner"

import { InstallUpdateDialog, RELEASES_URL, useUpdateStatus } from "@/components/update-banner"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import { Label } from "@/components/ui/label"
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select"
import { useNow } from "@/hooks/use-now"
import { useApi } from "@/lib/api-context"
import { formatRelative } from "@/lib/format"
import { useT } from "@/lib/i18n"
import type { UpdateChannel } from "@/lib/types"

/** Kartu "Pembaruan" di Pengaturan (hanya aplikasi desktop): versi, kanal, periksa sekarang. */
export function UpdateCard() {
  const t = useT()
  const { api } = useApi()
  const [status, setStatus] = useUpdateStatus()
  const [busy, setBusy] = React.useState(false)
  const [open, setOpen] = React.useState(false)
  const now = useNow(60_000)
  if (!status?.supported) return null

  async function run(fn: () => Promise<typeof status>) {
    setBusy(true)
    try {
      setStatus(await fn())
    } catch (e) {
      toast.error(e instanceof Error ? e.message : String(e))
      setStatus(await api.getUpdateStatus!().catch(() => status))
    } finally {
      setBusy(false)
    }
  }

  const av = status.available
  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <DownloadIcon className="size-4" /> {t("Pembaruan")}
        </CardTitle>
        <CardDescription>
          {t("Versi terpasang {current}. Aplikasi memeriksa pembaruan saat dibuka lalu tiap 24 jam; kamu yang memutuskan memasangnya.", { current: status.currentVersion ?? "-" })}
        </CardDescription>
      </CardHeader>
      <CardContent className="grid gap-4">
        <div className="grid gap-2 sm:max-w-xs">
          <Label htmlFor="upd-channel">{t("Kanal")}</Label>
          <Select value={status.channel} disabled={busy} onValueChange={(v) => void run(() => api.setUpdateChannel!(v as UpdateChannel))}>
            <SelectTrigger id="upd-channel" className="w-full">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="stable">{t("Stabil (disarankan)")}</SelectItem>
              <SelectItem value="rc">{t("Uji (termasuk pra-rilis)")}</SelectItem>
            </SelectContent>
          </Select>
        </div>

        <div className="rounded-lg border p-3 text-sm">
          {av ? (
            <div className="grid gap-1">
              <div className="font-medium">{t("Telemon {version} tersedia", { version: av.version })}</div>
              {av.notes && <p className="text-xs whitespace-pre-line text-muted-foreground">{av.notes}</p>}
              {!status.canInstall && <p className="text-xs text-muted-foreground">{t("Versi ini perlu dipasang manual (paket .deb).")}</p>}
            </div>
          ) : status.error ? (
            <span className="text-destructive">{t("Gagal memeriksa pembaruan: {error}", { error: status.error })}</span>
          ) : status.checkedAt ? (
            <span className="text-muted-foreground">{t("Sudah versi terbaru ({when}).", { when: formatRelative(status.checkedAt, now) })}</span>
          ) : (
            <span className="text-muted-foreground">{t("Belum diperiksa.")}</span>
          )}
        </div>

        <div className="flex flex-wrap gap-2">
          <Button variant="outline" size="sm" disabled={busy} onClick={() => void run(() => api.checkUpdate!())}>
            {busy ? <Loader2Icon className="animate-spin" /> : <RefreshCwIcon />} {t("Periksa sekarang")}
          </Button>
          {av &&
            (status.canInstall ? (
              <Button size="sm" disabled={busy} onClick={() => setOpen(true)}>
                {t("Perbarui ke {version}", { version: av.version })}
              </Button>
            ) : (
              <Button size="sm" asChild>
                <a href={RELEASES_URL} target="_blank" rel="noreferrer noopener">
                  {t("Unduh versi baru")}
                </a>
              </Button>
            ))}
        </div>
      </CardContent>
      {av && <InstallUpdateDialog version={av.version} open={open} onOpenChange={setOpen} />}
    </Card>
  )
}
