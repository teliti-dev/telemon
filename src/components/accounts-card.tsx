import * as React from "react"
import {
  DicesIcon,
  KeyRoundIcon,
  MoreVerticalIcon,
  PlusIcon,
  ShieldCheckIcon,
  ShieldOffIcon,
  Trash2Icon,
  UserCogIcon,
  UserCheckIcon,
  UserXIcon,
} from "lucide-react"
import { toast } from "sonner"

import { ConfirmDialog } from "@/components/confirm-dialog"
import { Alert, AlertDescription } from "@/components/ui/alert"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Card, CardAction, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { Switch } from "@/components/ui/switch"
import { useNow } from "@/hooks/use-now"
import { useApi } from "@/lib/api-context"
import { formatRelative } from "@/lib/format"
import { randomPassword } from "@/lib/random"
import { useStore } from "@/lib/store-context"
import type { AccountSummary, Role } from "@/lib/types"

const errMsg = (e: unknown) => (e instanceof Error ? e.message : String(e))

type Dialog_ =
  | null
  | { kind: "add" }
  | { kind: "reset-password"; account: AccountSummary }
  | { kind: "role"; account: AccountSummary }
  | { kind: "reset-2fa" | "delete"; account: AccountSummary }

/** Akun untuk akses lewat browser (Serve LAN dan Publik). Izin mengikuti peran. */
export function AccountsCard({ readOnly }: { readOnly?: boolean }) {
  const { api } = useApi()
  const { share } = useStore()
  const now = useNow(60_000)
  const [accounts, setAccounts] = React.useState<AccountSummary[] | null>(null)
  const [roles, setRoles] = React.useState<Role[]>([])
  const [dialog, setDialog] = React.useState<Dialog_>(null)

  const load = React.useCallback(() => {
    Promise.all([api.listAccounts(), api.listRoles()])
      .then(([a, r]) => {
        setAccounts(a)
        setRoles(r)
      })
      .catch((e: unknown) => toast.error(errMsg(e)))
  }, [api])

  React.useEffect(() => {
    load()
  }, [load])

  const publicOn = share?.publik.status === "on"
  const require2fa = !!share?.auth.require2fa

  async function toggleDisabled(a: AccountSummary) {
    try {
      await api.setAccountDisabled(a.id, !a.disabled)
      toast.success(a.disabled ? `${a.username} diaktifkan` : `${a.username} dinonaktifkan`)
      load()
    } catch (e) {
      toast.error(errMsg(e))
    }
  }

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <KeyRoundIcon className="size-4" /> Akun
        </CardTitle>
        <CardDescription>
          Untuk masuk lewat browser (LAN dan publik). Apa yang boleh dilakukan akun mengikuti
          perannya. Aplikasi desktop tidak memakai login. Anda tidak bisa mengelola akun Anda sendiri atau akun
          berizin lebih tinggi dari sini.
        </CardDescription>
        <CardAction>
          <Button onClick={() => setDialog({ kind: "add" })} disabled={readOnly}>
            <PlusIcon /> <span className="hidden sm:inline">Tambah akun</span>
            <span className="sm:hidden">Tambah</span>
          </Button>
        </CardAction>
      </CardHeader>
      <CardContent className="grid gap-3">
        <div className="flex items-center justify-between gap-4 rounded-lg border p-2.5">
          <div className="grid gap-0.5">
            <Label htmlFor="req2fa">Wajib 2FA untuk semua akun</Label>
            <p className="text-xs text-muted-foreground">
              {publicOn
                ? "Selalu wajib selama akses publik aktif."
                : "Akun tanpa 2FA diminta mendaftarkannya saat masuk."}
            </p>
          </div>
          <Switch
            id="req2fa"
            checked={require2fa}
            disabled={publicOn || readOnly}
            onCheckedChange={async (v) => {
              try {
                await api.setRequire2fa(v)
              } catch (e) {
                toast.error(errMsg(e))
              }
            }}
          />
        </div>

        {accounts === null ? (
          <p className="text-sm text-muted-foreground">Memuat…</p>
        ) : accounts.length === 0 ? (
          <Alert>
            <AlertDescription>Belum ada akun. Buat satu akun supaya Serve ke browser bisa dipakai.</AlertDescription>
          </Alert>
        ) : (
          <ul className="divide-y rounded-lg border">
            {accounts.map((a) => (
              <li key={a.id} className="flex items-center gap-3 px-3 py-2 text-[13px]">
                <div className="min-w-0 flex-1">
                  <div className="flex flex-wrap items-center gap-x-2 gap-y-1">
                    <span className="font-mono font-medium">{a.username}</span>
                    <span className="text-muted-foreground">{a.displayName}</span>
                    <Badge variant="secondary">{a.roleName}</Badge>
                  </div>
                  <div className="mt-0.5 flex flex-wrap items-center gap-x-2 gap-y-1 text-xs text-muted-foreground">
                    {a.twoFactor ? (
                      <span className="inline-flex items-center gap-1">
                        <ShieldCheckIcon className="size-3.5 text-good" /> 2FA aktif
                      </span>
                    ) : (
                      <span className="inline-flex items-center gap-1">
                        <ShieldOffIcon className="size-3.5" /> Tanpa 2FA
                      </span>
                    )}
                    <span>· {a.lastLoginAt ? `masuk ${formatRelative(a.lastLoginAt, now)}` : "belum pernah masuk"}</span>
                  </div>
                </div>
                {a.disabled && <Badge variant="outline">Nonaktif</Badge>}
                <DropdownMenu>
                  <DropdownMenuTrigger asChild>
                    <Button variant="ghost" size="icon-sm" disabled={readOnly} aria-label={`Aksi ${a.username}`}>
                      <MoreVerticalIcon />
                    </Button>
                  </DropdownMenuTrigger>
                  <DropdownMenuContent align="end">
                    <DropdownMenuItem onSelect={() => setDialog({ kind: "role", account: a })}>
                      <UserCogIcon /> Ubah peran
                    </DropdownMenuItem>
                    <DropdownMenuItem onSelect={() => setDialog({ kind: "reset-password", account: a })}>
                      <KeyRoundIcon /> Reset password
                    </DropdownMenuItem>
                    <DropdownMenuItem disabled={!a.twoFactor} onSelect={() => setDialog({ kind: "reset-2fa", account: a })}>
                      <ShieldOffIcon /> Reset 2FA
                    </DropdownMenuItem>
                    <DropdownMenuItem onSelect={() => void toggleDisabled(a)}>
                      {a.disabled ? <UserCheckIcon /> : <UserXIcon />} {a.disabled ? "Aktifkan" : "Nonaktifkan"}
                    </DropdownMenuItem>
                    <DropdownMenuSeparator />
                    <DropdownMenuItem variant="destructive" onSelect={() => setDialog({ kind: "delete", account: a })}>
                      <Trash2Icon /> Hapus
                    </DropdownMenuItem>
                  </DropdownMenuContent>
                </DropdownMenu>
              </li>
            ))}
          </ul>
        )}
      </CardContent>

      <AddAccountDialog open={dialog?.kind === "add"} onOpenChange={(o) => !o && setDialog(null)} onDone={load} roles={roles} />
      <ChangeRoleDialog
        account={dialog?.kind === "role" ? dialog.account : null}
        roles={roles}
        onOpenChange={(o) => !o && setDialog(null)}
        onDone={load}
      />
      <ResetPasswordDialog
        account={dialog?.kind === "reset-password" ? dialog.account : null}
        onOpenChange={(o) => !o && setDialog(null)}
      />
      <ConfirmDialog
        open={dialog?.kind === "reset-2fa"}
        onOpenChange={(o) => !o && setDialog(null)}
        title={`Reset 2FA ${dialog && "account" in dialog ? dialog.account.username : ""}?`}
        description={<p>2FA dinonaktifkan dan semua sesi akun ini dikeluarkan. Akun harus mendaftarkan 2FA lagi bila diwajibkan.</p>}
        confirmLabel="Reset 2FA"
        destructive
        onConfirm={async () => {
          if (dialog && "account" in dialog) {
            await api.resetAccount2fa(dialog.account.id)
            toast.success("2FA direset")
            load()
          }
        }}
      />
      <ConfirmDialog
        open={dialog?.kind === "delete"}
        onOpenChange={(o) => !o && setDialog(null)}
        title={`Hapus akun ${dialog && "account" in dialog ? dialog.account.username : ""}?`}
        description={<p>Akun dan semua sesinya dihapus permanen. Data pemantauan tidak terpengaruh.</p>}
        confirmLabel="Hapus"
        destructive
        onConfirm={async () => {
          if (dialog && "account" in dialog) {
            await api.deleteAccount(dialog.account.id)
            toast.success("Akun dihapus")
            load()
          }
        }}
      />
    </Card>
  )
}

function PasswordField({
  id,
  value,
  onChange,
}: {
  id: string
  value: string
  onChange: (v: string) => void
}) {
  return (
    <div className="flex gap-1.5">
      <Input
        id={id}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        autoComplete="off"
        spellCheck={false}
        className="font-mono"
        aria-invalid={value.length > 0 && value.length < 8}
      />
      <Button type="button" variant="outline" size="icon" aria-label="Buat password acak" title="Buat password acak" onClick={() => onChange(randomPassword())}>
        <DicesIcon />
      </Button>
    </div>
  )
}

function RoleSelect({ id, roles, value, onChange }: { id: string; roles: Role[]; value: string; onChange: (v: string) => void }) {
  return (
    <Select value={value} onValueChange={onChange}>
      <SelectTrigger id={id} className="w-full">
        <SelectValue placeholder="Pilih peran" />
      </SelectTrigger>
      <SelectContent>
        {roles.map((r) => (
          <SelectItem key={r.id} value={r.id}>
            {r.name}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  )
}

function AddAccountDialog({
  open,
  onOpenChange,
  onDone,
  roles,
}: {
  open: boolean
  onOpenChange: (o: boolean) => void
  onDone: () => void
  roles: Role[]
}) {
  const { api } = useApi()
  const [roleId, setRoleId] = React.useState("")
  const [username, setUsername] = React.useState("")
  const [displayName, setDisplayName] = React.useState("")
  const [password, setPassword] = React.useState("")
  const [busy, setBusy] = React.useState(false)
  const [error, setError] = React.useState<string | null>(null)

  return (
    <Dialog
      open={open}
      onOpenChange={(o) => {
        if (!o) {
          setUsername("")
          setDisplayName("")
          setPassword("")
          setRoleId("")
          setError(null)
        }
        onOpenChange(o)
      }}
    >
      <DialogContent className="sm:max-w-md">
        <form
          className="grid gap-3"
          onSubmit={async (e) => {
            e.preventDefault()
            setBusy(true)
            setError(null)
            try {
              const a = await api.createAccount({ username, displayName, password, roleId })
              toast.success(`Akun ${a.username} dibuat`)
              onDone()
              onOpenChange(false)
            } catch (err) {
              setError(errMsg(err))
            } finally {
              setBusy(false)
            }
          }}
        >
          <DialogHeader>
            <DialogTitle>Tambah akun</DialogTitle>
            <DialogDescription>
              Berikan username dan password ini ke orangnya lewat jalur aman.
            </DialogDescription>
          </DialogHeader>
          <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
            <div className="grid gap-1.5">
              <Label htmlFor="ac-user">Username</Label>
              <Input id="ac-user" value={username} onChange={(e) => setUsername(e.target.value.toLowerCase())} autoComplete="off" spellCheck={false} className="font-mono" placeholder="noc.budi" autoFocus />
            </div>
            <div className="grid gap-1.5">
              <Label htmlFor="ac-name">Nama</Label>
              <Input id="ac-name" value={displayName} onChange={(e) => setDisplayName(e.target.value)} maxLength={60} placeholder="Budi (NOC)" />
            </div>
          </div>
          <div className="grid gap-1.5">
            <Label htmlFor="ac-role">Peran</Label>
            <RoleSelect id="ac-role" roles={roles} value={roleId} onChange={setRoleId} />
            <p className="text-xs text-muted-foreground">Wajib dipilih; tanpa peran akun tidak punya akses. Atur izin peran di tab Peran.</p>
          </div>
          <div className="grid gap-1.5">
            <Label htmlFor="ac-pass">Password awal</Label>
            <PasswordField id="ac-pass" value={password} onChange={setPassword} />
            <p className="text-xs text-muted-foreground">Minimal 8 karakter. Orangnya bisa menggantinya sendiri setelah masuk.</p>
          </div>
          {error && (
            <Alert variant="destructive">
              <AlertDescription>{error}</AlertDescription>
            </Alert>
          )}
          <DialogFooter>
            <Button type="button" variant="outline" onClick={() => onOpenChange(false)} disabled={busy}>
              Batal
            </Button>
            <Button type="submit" disabled={busy || !username.trim() || !displayName.trim() || password.length < 8 || !roleId}>
              {busy ? "Membuat…" : "Buat akun"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}

function ResetPasswordDialog({
  account,
  onOpenChange,
}: {
  account: AccountSummary | null
  onOpenChange: (o: boolean) => void
}) {
  const { api } = useApi()
  const [password, setPassword] = React.useState("")
  const [busy, setBusy] = React.useState(false)
  const [error, setError] = React.useState<string | null>(null)

  return (
    <Dialog
      open={!!account}
      onOpenChange={(o) => {
        if (!o) {
          setPassword("")
          setError(null)
        }
        onOpenChange(o)
      }}
    >
      <DialogContent className="sm:max-w-md">
        <form
          className="grid gap-3"
          onSubmit={async (e) => {
            e.preventDefault()
            if (!account) return
            setBusy(true)
            setError(null)
            try {
              await api.resetAccountPassword(account.id, password)
              toast.success("Password direset; sesi akun dikeluarkan")
              onOpenChange(false)
            } catch (err) {
              setError(errMsg(err))
            } finally {
              setBusy(false)
            }
          }}
        >
          <DialogHeader>
            <DialogTitle>Reset password {account?.username}</DialogTitle>
            <DialogDescription>Semua sesi akun ini dikeluarkan.</DialogDescription>
          </DialogHeader>
          <div className="grid gap-1.5">
            <Label htmlFor="rp-pass">Password baru</Label>
            <PasswordField id="rp-pass" value={password} onChange={setPassword} />
          </div>
          {error && (
            <Alert variant="destructive">
              <AlertDescription>{error}</AlertDescription>
            </Alert>
          )}
          <DialogFooter>
            <Button type="button" variant="outline" onClick={() => onOpenChange(false)} disabled={busy}>
              Batal
            </Button>
            <Button type="submit" disabled={busy || password.length < 8}>
              {busy ? "Menyimpan…" : "Reset"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}

function ChangeRoleDialog({
  account,
  roles,
  onOpenChange,
  onDone,
}: {
  account: AccountSummary | null
  roles: Role[]
  onOpenChange: (o: boolean) => void
  onDone: () => void
}) {
  const { api } = useApi()
  return (
    <Dialog open={!!account} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        {account && <ChangeRoleForm key={account.id} account={account} roles={roles} onClose={() => onOpenChange(false)} onDone={onDone} api={api} />}
      </DialogContent>
    </Dialog>
  )
}

function ChangeRoleForm({
  account,
  roles,
  onClose,
  onDone,
  api,
}: {
  account: AccountSummary
  roles: Role[]
  onClose: () => void
  onDone: () => void
  api: ReturnType<typeof useApi>["api"]
}) {
  const [roleId, setRoleId] = React.useState(account.roleId)
  const [busy, setBusy] = React.useState(false)
  const [error, setError] = React.useState<string | null>(null)
  const role = roles.find((r) => r.id === roleId)

  return (
    <form
      className="grid gap-3"
      onSubmit={async (e) => {
        e.preventDefault()
        setBusy(true)
        setError(null)
        try {
          await api.setAccountRole(account.id, roleId)
          toast.success(`Peran ${account.username} diubah`)
          onDone()
          onClose()
        } catch (err) {
          setError(errMsg(err))
        } finally {
          setBusy(false)
        }
      }}
    >
      <DialogHeader>
        <DialogTitle>Ubah peran {account.username}</DialogTitle>
        <DialogDescription>Berlaku pada sesi berikutnya; sesi yang sedang berjalan mengikuti peran saat masuk.</DialogDescription>
      </DialogHeader>
      <div className="grid gap-1.5">
        <Label htmlFor="cr-role">Peran</Label>
        <RoleSelect id="cr-role" roles={roles} value={roleId} onChange={setRoleId} />
        {role?.description && <p className="text-xs text-muted-foreground">{role.description}</p>}
      </div>
      {error && (
        <Alert variant="destructive">
          <AlertDescription>{error}</AlertDescription>
        </Alert>
      )}
      <DialogFooter>
        <Button type="button" variant="outline" onClick={onClose} disabled={busy}>
          Batal
        </Button>
        <Button type="submit" disabled={busy || roleId === account.roleId}>
          {busy ? "Menyimpan…" : "Simpan"}
        </Button>
      </DialogFooter>
    </form>
  )
}
