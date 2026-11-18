import * as React from "react"
import { LogOutIcon, MonitorSmartphoneIcon, ShieldCheckIcon, ShieldOffIcon } from "lucide-react"
import { toast } from "sonner"

import { RecoveryCodes } from "@/components/recovery-codes"
import { TotpSetup } from "@/components/totp-setup"
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
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { useNow } from "@/hooks/use-now"
import { useApi } from "@/lib/api-context"
import { useAuth } from "@/lib/auth-context"
import { formatDateTime, formatRelative } from "@/lib/format"
import { useStore } from "@/lib/store-context"
import type { SessionDevice, TwoFactorSetup } from "@/lib/types"

const errMsg = (e: unknown) => (e instanceof Error ? e.message : String(e))

export function AccountPage() {
  const { api } = useApi()
  const { session, setSession, logout } = useAuth()
  const { share } = useStore()

  if (!session) {
    return (
      <Alert className="max-w-lg">
        <ShieldOffIcon />
        <AlertDescription>
          Halaman Akun hanya untuk akses lewat browser. Aplikasi desktop tidak memakai login;
          kelola akun jarak jauh di halaman Berbagi.
        </AlertDescription>
      </Alert>
    )
  }

  return (
    <div className="grid items-start gap-3 xl:grid-cols-2">
      <ProfileCard
        username={session.username}
        displayName={session.displayName}
        onSave={async (name) => {
          setSession(await api.updateMyProfile({ displayName: name }))
          toast.success("Profil disimpan")
        }}
      />
      <PasswordCard />
      <TwoFactorCard
        enabled={session.twoFactor}
        required={!!share?.auth.require2fa}
        onChanged={async () => {
          const s = await api.session()
          if (s) setSession(s)
        }}
      />
      <SessionsCard />
      <div className="xl:col-span-2">
        <Button variant="outline" onClick={() => void logout()}>
          <LogOutIcon /> Keluar
        </Button>
      </div>
    </div>
  )
}

function ProfileCard({
  username,
  displayName,
  onSave,
}: {
  username: string
  displayName: string
  onSave: (name: string) => Promise<void>
}) {
  const [name, setName] = React.useState(displayName)
  const [busy, setBusy] = React.useState(false)
  const dirty = name.trim() !== displayName

  return (
    <Card>
      <CardHeader>
        <CardTitle>Profil</CardTitle>
        <CardDescription>Nama yang tampil di Telemon.</CardDescription>
      </CardHeader>
      <CardContent>
        <form
          className="grid gap-3"
          onSubmit={async (e) => {
            e.preventDefault()
            setBusy(true)
            try {
              await onSave(name)
            } catch (err) {
              toast.error(errMsg(err))
            } finally {
              setBusy(false)
            }
          }}
        >
          <div className="grid gap-1.5">
            <Label htmlFor="a-user">Username</Label>
            <Input id="a-user" value={username} readOnly disabled className="font-mono" />
          </div>
          <div className="grid gap-1.5">
            <Label htmlFor="a-name">Nama</Label>
            <Input id="a-name" value={name} onChange={(e) => setName(e.target.value)} maxLength={60} />
          </div>
          <Button type="submit" className="w-fit" disabled={!dirty || !name.trim() || busy}>
            Simpan
          </Button>
        </form>
      </CardContent>
    </Card>
  )
}

function PasswordCard() {
  const { api } = useApi()
  const [current, setCurrent] = React.useState("")
  const [next, setNext] = React.useState("")
  const [confirm, setConfirm] = React.useState("")
  const [busy, setBusy] = React.useState(false)
  const [error, setError] = React.useState<string | null>(null)
  const mismatch = confirm.length > 0 && next !== confirm
  const valid = current.length > 0 && next.length >= 8 && next === confirm

  return (
    <Card>
      <CardHeader>
        <CardTitle>Password</CardTitle>
        <CardDescription>Minimal 8 karakter. Sesi di perangkat lain tetap berjalan sampai Anda keluarkan.</CardDescription>
      </CardHeader>
      <CardContent>
        <form
          className="grid gap-3"
          onSubmit={async (e) => {
            e.preventDefault()
            setBusy(true)
            setError(null)
            try {
              await api.changeMyPassword(current, next)
              setCurrent("")
              setNext("")
              setConfirm("")
              toast.success("Password diganti")
            } catch (err) {
              setError(errMsg(err))
            } finally {
              setBusy(false)
            }
          }}
        >
          <div className="grid gap-1.5">
            <Label htmlFor="pw-cur">Password sekarang</Label>
            <Input id="pw-cur" type="password" value={current} onChange={(e) => setCurrent(e.target.value)} autoComplete="current-password" />
          </div>
          <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
            <div className="grid gap-1.5">
              <Label htmlFor="pw-new">Password baru</Label>
              <Input id="pw-new" type="password" value={next} onChange={(e) => setNext(e.target.value)} autoComplete="new-password" />
            </div>
            <div className="grid gap-1.5">
              <Label htmlFor="pw-conf">Ulangi password baru</Label>
              <Input id="pw-conf" type="password" value={confirm} onChange={(e) => setConfirm(e.target.value)} autoComplete="new-password" aria-invalid={mismatch} />
            </div>
          </div>
          {mismatch && <p className="-mt-1 text-xs text-destructive">Password tidak sama.</p>}
          {error && (
            <Alert variant="destructive">
              <AlertDescription>{error}</AlertDescription>
            </Alert>
          )}
          <Button type="submit" className="w-fit" disabled={!valid || busy}>
            Ganti password
          </Button>
        </form>
      </CardContent>
    </Card>
  )
}

type TfaDialog =
  | null
  | { kind: "setup"; setup: TwoFactorSetup }
  | { kind: "codes"; codes: string[] }
  | { kind: "password"; action: "disable" | "regenerate" }

function TwoFactorCard({
  enabled,
  required,
  onChanged,
}: {
  enabled: boolean
  required: boolean
  onChanged: () => Promise<void>
}) {
  const { api } = useApi()
  const [dialog, setDialog] = React.useState<TfaDialog>(null)
  const [busy, setBusy] = React.useState(false)

  async function startSetup() {
    setBusy(true)
    try {
      setDialog({ kind: "setup", setup: await api.start2faSetup() })
    } catch (e) {
      toast.error(errMsg(e))
    } finally {
      setBusy(false)
    }
  }

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          Verifikasi dua langkah (2FA)
          {enabled ? (
            <Badge variant="outline" className="gap-1.5 font-normal">
              <ShieldCheckIcon className="size-3.5 text-good" /> Aktif
            </Badge>
          ) : (
            <Badge variant="outline" className="gap-1.5 font-normal">
              <ShieldOffIcon className="size-3.5 text-muted-foreground" /> Belum aktif
            </Badge>
          )}
        </CardTitle>
        <CardDescription>Kode 6 digit dari aplikasi authenticator saat masuk.</CardDescription>
        <CardAction />
      </CardHeader>
      <CardContent className="grid gap-3">
        {required && (
          <Alert>
            <ShieldCheckIcon />
            <AlertDescription>
              Pemilik Telemon mewajibkan 2FA (otomatis saat akses publik aktif), jadi tidak bisa dinonaktifkan.
            </AlertDescription>
          </Alert>
        )}
        <div className="flex flex-wrap gap-2">
          {enabled ? (
            <>
              <Button variant="outline" onClick={() => setDialog({ kind: "password", action: "regenerate" })}>
                Buat ulang kode pemulihan
              </Button>
              <Button variant="outline" disabled={required} onClick={() => setDialog({ kind: "password", action: "disable" })}>
                Nonaktifkan
              </Button>
            </>
          ) : (
            <Button onClick={startSetup} disabled={busy}>
              Aktifkan 2FA
            </Button>
          )}
        </div>
      </CardContent>

      <Dialog open={!!dialog} onOpenChange={(o) => !o && setDialog(null)}>
        <DialogContent className="max-h-[92svh] overflow-y-auto sm:max-w-md">
          {dialog?.kind === "setup" && (
            <>
              <DialogHeader>
                <DialogTitle>Aktifkan 2FA</DialogTitle>
                <DialogDescription>Daftarkan Telemon di aplikasi authenticator Anda.</DialogDescription>
              </DialogHeader>
              <TotpSetup
                secret={dialog.setup.secret}
                otpauthUrl={dialog.setup.otpauthUrl}
                submitLabel="Aktifkan"
                onSubmit={async (code) => {
                  const r = await api.confirm2faSetup(code)
                  await onChanged()
                  setDialog({ kind: "codes", codes: r.recoveryCodes })
                }}
                onCancel={() => setDialog(null)}
              />
            </>
          )}
          {dialog?.kind === "codes" && (
            <>
              <DialogHeader>
                <DialogTitle>Kode pemulihan</DialogTitle>
                <DialogDescription>Kode lama tidak berlaku lagi.</DialogDescription>
              </DialogHeader>
              <RecoveryCodes codes={dialog.codes} />
              <DialogFooter>
                <Button onClick={() => setDialog(null)}>Selesai</Button>
              </DialogFooter>
            </>
          )}
          {dialog?.kind === "password" && (
            <PasswordPrompt
              title={dialog.action === "disable" ? "Nonaktifkan 2FA?" : "Buat ulang kode pemulihan?"}
              description={
                dialog.action === "disable"
                  ? "Akun Anda hanya akan dilindungi password."
                  : "Kode pemulihan lama langsung tidak berlaku."
              }
              confirmLabel={dialog.action === "disable" ? "Nonaktifkan" : "Buat ulang"}
              destructive={dialog.action === "disable"}
              onCancel={() => setDialog(null)}
              onConfirm={async (password) => {
                if (dialog.action === "disable") {
                  await api.disable2fa(password)
                  await onChanged()
                  setDialog(null)
                  toast.success("2FA dinonaktifkan")
                } else {
                  setDialog({ kind: "codes", codes: await api.regenerateRecoveryCodes(password) })
                }
              }}
            />
          )}
        </DialogContent>
      </Dialog>
    </Card>
  )
}

function PasswordPrompt({
  title,
  description,
  confirmLabel,
  destructive,
  onConfirm,
  onCancel,
}: {
  title: string
  description: string
  confirmLabel: string
  destructive?: boolean
  onConfirm: (password: string) => Promise<void>
  onCancel: () => void
}) {
  const [password, setPassword] = React.useState("")
  const [busy, setBusy] = React.useState(false)
  const [error, setError] = React.useState<string | null>(null)

  return (
    <form
      className="grid gap-3"
      onSubmit={async (e) => {
        e.preventDefault()
        setBusy(true)
        setError(null)
        try {
          await onConfirm(password)
        } catch (err) {
          setError(errMsg(err))
          setBusy(false)
        }
      }}
    >
      <DialogHeader>
        <DialogTitle>{title}</DialogTitle>
        <DialogDescription>{description}</DialogDescription>
      </DialogHeader>
      <div className="grid gap-1.5">
        <Label htmlFor="pp-pass">Masukkan password Anda untuk melanjutkan</Label>
        <Input id="pp-pass" type="password" value={password} onChange={(e) => setPassword(e.target.value)} autoComplete="current-password" autoFocus />
      </div>
      {error && (
        <Alert variant="destructive">
          <AlertDescription>{error}</AlertDescription>
        </Alert>
      )}
      <DialogFooter>
        <Button type="button" variant="outline" onClick={onCancel} disabled={busy}>
          Batal
        </Button>
        <Button type="submit" variant={destructive ? "destructive" : "default"} disabled={!password || busy}>
          {busy ? "Memproses…" : confirmLabel}
        </Button>
      </DialogFooter>
    </form>
  )
}

function SessionsCard() {
  const { api } = useApi()
  const now = useNow(30_000)
  const [items, setItems] = React.useState<SessionDevice[] | null>(null)

  const load = React.useCallback(() => {
    api
      .listMySessions()
      .then(setItems)
      .catch((e: unknown) => toast.error(errMsg(e)))
  }, [api])

  React.useEffect(() => {
    load()
  }, [load])

  return (
    <Card className="gap-0 py-0">
      <CardHeader className="border-b px-3 py-3 [.border-b]:pb-3">
        <CardTitle className="flex items-center gap-2">
          <MonitorSmartphoneIcon className="size-4" /> Sesi aktif
        </CardTitle>
        <CardDescription>Perangkat yang sedang masuk dengan akun ini.</CardDescription>
      </CardHeader>
      <CardContent className="px-0">
        {items === null ? (
          <p className="px-3 py-4 text-sm text-muted-foreground">Memuat…</p>
        ) : (
          <ul className="divide-y">
            {items.map((s) => (
              <li key={s.id} className="flex items-center gap-3 px-3 py-2 text-[13px]">
                <div className="min-w-0 flex-1">
                  <div className="flex items-center gap-2">
                    <span className="truncate font-medium">{s.device}</span>
                    {s.current && <Badge variant="secondary">Sesi ini</Badge>}
                  </div>
                  <div className="truncate text-xs text-muted-foreground">
                    {s.ip} · masuk {formatDateTime(s.createdAt)} · aktif {formatRelative(s.lastSeenAt, now)}
                  </div>
                </div>
                {!s.current && (
                  <Button
                    variant="outline"
                    size="sm"
                    onClick={async () => {
                      try {
                        await api.revokeSession(s.id)
                        toast.success("Sesi dikeluarkan")
                        load()
                      } catch (e) {
                        toast.error(errMsg(e))
                      }
                    }}
                  >
                    Keluarkan
                  </Button>
                )}
              </li>
            ))}
          </ul>
        )}
      </CardContent>
    </Card>
  )
}
