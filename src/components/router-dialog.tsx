import * as React from "react"
import { toast } from "sonner"

import { FormFooter, FormHeader } from "@/components/form-layout"
import { Button } from "@/components/ui/button"
import { Dialog, DialogContent } from "@/components/ui/dialog"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Switch } from "@/components/ui/switch"
import { useT } from "@/lib/i18n"
import { useStore } from "@/lib/store-context"
import type { RouterInfo } from "@/lib/types"

export function RouterDialog({
  open,
  onOpenChange,
  router,
}: {
  open: boolean
  onOpenChange: (o: boolean) => void
  /** Kosong = router baru. */
  router?: RouterInfo
}) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-h-[92svh] overflow-y-auto sm:max-w-md">
        {open && <RouterForm router={router} onClose={() => onOpenChange(false)} />}
      </DialogContent>
    </Dialog>
  )
}

export function RouterForm({ router, onClose }: { router?: RouterInfo; onClose: () => void }) {
  const { api } = useStore()
  const t = useT()
  const editing = !!router
  const [tls, setTls] = React.useState(router?.tls ?? true)
  const [allowWrite, setAllowWrite] = React.useState(router?.canWrite ?? false)
  const [port, setPort] = React.useState(String(router?.port ?? 8729))
  const [busy, setBusy] = React.useState(false)

  async function onSubmit(e: React.FormEvent<HTMLFormElement>) {
    e.preventDefault()
    const f = new FormData(e.currentTarget)
    const username = String(f.get("username")).trim()
    const password = String(f.get("password"))
    setBusy(true)
    try {
      const base = {
        name: String(f.get("name")).trim(),
        host: String(f.get("host")).trim(),
        port: Number(port),
        tls,
        allowWrite,
      }
      if (router) {
        const r = await api.updateRouter(router.id, { ...base, username: username || undefined, password: password || undefined })
        toast.success(t("{name} diperbarui", { name: r.name }))
      } else {
        const r = await api.addRouter({ ...base, username, password })
        toast.success(t("{name} ditambahkan", { name: r.name }))
      }
      onClose()
    } catch (err) {
      toast.error(err instanceof Error ? err.message : String(err))
    } finally {
      setBusy(false)
    }
  }

  return (
    <form onSubmit={onSubmit} className="grid gap-4">
      <FormHeader
        title={editing ? t("Edit {name}", { name: router?.name ?? "" }) : t("Tambah router")}
        description={
          <>
            {t("Pakai akun API khusus. Grup")} <code>read</code> {t("cukup untuk memantau; tambah hak")} <code>write</code>{" "}
            {t("bila user dan profil PPP ingin diedit dari Telemon. Password disimpan di keyring sistem operasi")}
            {editing && t("; kosongkan username dan password bila tidak diubah")}.
          </>
        }
      />
      <div className="grid gap-2">
        <Label htmlFor="r-name">{t("Nama")}</Label>
        <Input id="r-name" name="name" required defaultValue={router?.name} placeholder={t("RB-Utara")} />
      </div>
      <div className="grid grid-cols-[1fr_6rem] gap-3">
        <div className="grid gap-2">
          <Label htmlFor="r-host">{t("Host / IP")}</Label>
          <Input id="r-host" name="host" required defaultValue={router?.host} placeholder="10.10.5.1" inputMode="url" />
        </div>
        <div className="grid gap-2">
          <Label htmlFor="r-port">{t("Port")}</Label>
          <Input
            id="r-port"
            required
            inputMode="numeric"
            pattern="[0-9]{1,5}"
            value={port}
            onChange={(e) => setPort(e.target.value)}
          />
        </div>
      </div>
      <div className="flex items-center justify-between gap-4 rounded-lg border p-3">
        <div className="grid gap-0.5">
          <Label htmlFor="r-tls">API-SSL (TLS)</Label>
          <p className="text-xs text-muted-foreground">{t("Disarankan. Port bawaan 8729; tanpa TLS 8728.")}</p>
        </div>
        <Switch
          id="r-tls"
          checked={tls}
          onCheckedChange={(v) => {
            setTls(v)
            setPort((p) => (p === "8729" || p === "8728" ? (v ? "8729" : "8728") : p))
          }}
        />
      </div>
      <div className="flex items-center justify-between gap-4 rounded-lg border p-3">
        <div className="grid gap-0.5">
          <Label htmlFor="r-write">{t("Izinkan edit user dan profil PPP")}</Label>
          <p className="text-xs text-muted-foreground">
            {t("Nyalakan hanya bila akun API di bawah punya hak write. Setiap tulis ke router tetap meminta konfirmasi.")}
          </p>
        </div>
        <Switch id="r-write" checked={allowWrite} onCheckedChange={setAllowWrite} />
      </div>
      <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
        <div className="grid gap-2">
          <Label htmlFor="r-user">{t("Username API")}</Label>
          <Input id="r-user" name="username" required={!editing} autoComplete="off" placeholder={editing ? t("tetap") : "telemon"} />
        </div>
        <div className="grid gap-2">
          <Label htmlFor="r-pass">{t("Password")}</Label>
          <Input id="r-pass" name="password" type="password" required={!editing} autoComplete="new-password" placeholder={editing ? t("tetap") : undefined} />
        </div>
      </div>
      <FormFooter>
        <Button type="button" variant="outline" onClick={onClose} disabled={busy}>
          {t("Batal")}
        </Button>
        <Button type="submit" disabled={busy}>
          {busy ? t("Menghubungkan…") : t("Simpan")}
        </Button>
      </FormFooter>
    </form>
  )
}
