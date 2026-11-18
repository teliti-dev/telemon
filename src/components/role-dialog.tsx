import * as React from "react"
import { RotateCcwIcon, ShieldAlertIcon } from "lucide-react"
import { toast } from "sonner"

import { FormFooter, FormHeader } from "@/components/form-layout"
import { PermissionMatrix } from "@/components/permission-matrix"
import { Alert, AlertDescription } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import { Dialog, DialogContent } from "@/components/ui/dialog"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { useApi } from "@/lib/api-context"
import { DEFAULT_ROLES } from "@/lib/defaults"
import { useT } from "@/lib/i18n"
import { hasUpdate } from "@/lib/permissions"
import type { Permissions, Role } from "@/lib/types"

interface Props {
  open: boolean
  onOpenChange: (open: boolean) => void
  /** Kosong = peran baru. */
  role?: Role
  onDone: () => void
}

export function RoleDialog({ open, onOpenChange, ...rest }: Props) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-h-[92svh] overflow-y-auto sm:max-w-lg">
        {open && <RoleForm onClose={() => onOpenChange(false)} {...rest} />}
      </DialogContent>
    </Dialog>
  )
}

export function RoleForm({ role, onDone, onClose }: { role?: Role; onDone: () => void; onClose: () => void }) {
  const t = useT()
  const { api } = useApi()
  const def = role?.builtin ? DEFAULT_ROLES.find((d) => d.id === role.id) : undefined
  // Peran baru: tanpa akses sampai diberi centang.
  const none: Permissions = { ringkasan: "none", user: "none", profile: "none", event: "none", router: "none", admin: "none", pengaturan: "none", ai: "none" }
  const [name, setName] = React.useState(role?.name ?? "")
  const [description, setDescription] = React.useState(role?.description ?? "")
  const [perms, setPerms] = React.useState<Permissions>(role?.permissions ?? none)
  const [busy, setBusy] = React.useState(false)
  const [error, setError] = React.useState<string | null>(null)

  const isDefault =
    !!def && name === def.name && description === def.description && JSON.stringify(perms) === JSON.stringify(def.permissions)

  async function submit(e: React.FormEvent) {
    e.preventDefault()
    setBusy(true)
    setError(null)
    try {
      const input = { name, description, permissions: perms }
      if (role) await api.updateRole(role.id, input)
      else await api.createRole(input)
      toast.success(role ? t("Peran disimpan") : t("Peran dibuat"))
      onDone()
      onClose()
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setBusy(false)
    }
  }

  return (
    <form onSubmit={submit} className="grid gap-3">
      <FormHeader
        title={role ? t("Edit peran {name}", { name: role.name }) : t("Tambah peran")}
        description={t("Bawaan tanpa akses: centang menu yang boleh dilihat atau diubah akun dengan peran ini. Menu Berbagi dan izin tulis lewat akses publik hanya dari aplikasi desktop. Halaman Akun saya (profil sendiri, password, 2FA, sesi) tidak memerlukan izin.")}
      />

      <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
        <div className="grid gap-1.5">
          <Label htmlFor="role-name">{t("Nama")}</Label>
          <Input id="role-name" value={name} onChange={(e) => setName(e.target.value)} maxLength={30} placeholder={t("NOC Shift")} autoFocus={!role} />
        </div>
        <div className="grid gap-1.5">
          <Label htmlFor="role-desc">{t("Keterangan")}</Label>
          <Input id="role-desc" value={description} onChange={(e) => setDescription(e.target.value)} maxLength={120} placeholder={t("opsional")} />
        </div>
      </div>

      <PermissionMatrix value={perms} onChange={setPerms} />

      {hasUpdate(perms) && (
        <Alert>
          <ShieldAlertIcon />
          <AlertDescription>
            {t("Izin")} <b>{t("Ubah")}</b>{" "}
            {t("berarti akun bisa mengubah data lewat browser: Users, Profil PPP, dan Router menulis ke router (tetap ada tinjauan sebelum menulis); Admin dan Pengaturan mengubah akun, peran, dan pengaturan. Akun dengan peran ini wajib 2FA, dan tulis lewat akses publik mengikuti Pengaturan.")}
          </AlertDescription>
        </Alert>
      )}

      {error && (
        <Alert variant="destructive">
          <AlertDescription>{error}</AlertDescription>
        </Alert>
      )}

      <FormFooter className="sm:justify-between">
        {def ? (
          <Button
            type="button"
            variant="ghost"
            disabled={isDefault || busy}
            onClick={() => {
              setName(def.name)
              setDescription(def.description)
              setPerms({ ...def.permissions })
            }}
          >
            <RotateCcwIcon /> {t("Kembalikan ke bawaan")}
          </Button>
        ) : (
          <span />
        )}
        <div className="flex gap-2">
          <Button type="button" variant="outline" onClick={onClose} disabled={busy}>
            {t("Batal")}
          </Button>
          <Button type="submit" disabled={busy || name.trim().length < 2}>
            {busy ? t("Menyimpan…") : t("Simpan")}
          </Button>
        </div>
      </FormFooter>
    </form>
  )
}
