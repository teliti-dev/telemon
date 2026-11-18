import * as React from "react"
import { RotateCcwIcon, ShieldAlertIcon } from "lucide-react"
import { toast } from "sonner"

import { PermissionMatrix } from "@/components/permission-matrix"
import { Alert, AlertDescription } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { useApi } from "@/lib/api-context"
import { DEFAULT_NEW_ACCOUNT_ROLE_ID, DEFAULT_ROLES } from "@/lib/defaults"
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

function RoleForm({ role, onDone, onClose }: Omit<Props, "open" | "onOpenChange"> & { onClose: () => void }) {
  const { api } = useApi()
  const def = role?.builtin ? DEFAULT_ROLES.find((d) => d.id === role.id) : undefined
  const template = DEFAULT_ROLES.find((d) => d.id === DEFAULT_NEW_ACCOUNT_ROLE_ID)!.permissions
  const [name, setName] = React.useState(role?.name ?? "")
  const [description, setDescription] = React.useState(role?.description ?? "")
  const [perms, setPerms] = React.useState<Permissions>(role?.permissions ?? { ...template })
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
      toast.success(role ? "Peran disimpan" : "Peran dibuat")
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
      <DialogHeader>
        <DialogTitle>{role ? `Edit peran ${role.name}` : "Tambah peran"}</DialogTitle>
        <DialogDescription>
          Pilih apa yang boleh dilakukan akun dengan peran ini di tiap menu. Menu Berbagi, Admin, dan Pengaturan
          selalu hanya di aplikasi desktop.
        </DialogDescription>
      </DialogHeader>

      <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
        <div className="grid gap-1.5">
          <Label htmlFor="role-name">Nama</Label>
          <Input id="role-name" value={name} onChange={(e) => setName(e.target.value)} maxLength={30} placeholder="NOC Shift" autoFocus={!role} />
        </div>
        <div className="grid gap-1.5">
          <Label htmlFor="role-desc">Keterangan</Label>
          <Input id="role-desc" value={description} onChange={(e) => setDescription(e.target.value)} maxLength={120} placeholder="opsional" />
        </div>
      </div>

      <PermissionMatrix value={perms} onChange={setPerms} />

      {hasUpdate(perms) && (
        <Alert>
          <ShieldAlertIcon />
          <AlertDescription>
            Izin <b>Ubah</b> berarti akun bisa menulis ke router lewat browser (tetap ada tinjauan sebelum menulis).
            Akun dengan peran ini wajib memakai 2FA. Tulis lewat akses publik mengikuti Pengaturan.
          </AlertDescription>
        </Alert>
      )}

      {error && (
        <Alert variant="destructive">
          <AlertDescription>{error}</AlertDescription>
        </Alert>
      )}

      <DialogFooter className="sm:justify-between">
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
            <RotateCcwIcon /> Kembalikan ke bawaan
          </Button>
        ) : (
          <span />
        )}
        <div className="flex gap-2">
          <Button type="button" variant="outline" onClick={onClose} disabled={busy}>
            Batal
          </Button>
          <Button type="submit" disabled={busy || name.trim().length < 2}>
            {busy ? "Menyimpan…" : "Simpan"}
          </Button>
        </div>
      </DialogFooter>
    </form>
  )
}
