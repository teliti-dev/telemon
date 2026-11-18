import * as React from "react"
import {
  LockIcon,
  LockOpenIcon,
  MoreVerticalIcon,
  PlusIcon,
  TriangleAlertIcon,
  UsersIcon,
  Trash2Icon,
  DownloadIcon,
  PencilIcon,
} from "lucide-react"
import { toast } from "sonner"

import { ImportDialog } from "@/components/import-dialog"
import { RouterStatusBadge, UserMixBar } from "@/components/status"
import { Alert, AlertDescription } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import {
  Card,
  CardAction,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card"
import {
  Dialog,
  DialogClose,
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
import { Switch } from "@/components/ui/switch"
import { useNow } from "@/hooks/use-now"
import { formatRelative } from "@/lib/format"
import { useUpdateGate } from "@/hooks/use-write-gate"
import { navigate } from "@/lib/route"
import { useStore } from "@/lib/store-context"
import type { RouterInfo } from "@/lib/types"

export function RoutersPage() {
  const { routers, liveUsers: users, api } = useStore()
  const importReason = useUpdateGate("router")()
  const [editing, setEditing] = React.useState<RouterInfo | null>(null)
  const now = useNow(10_000)
  const [adding, setAdding] = React.useState(false)
  const [removing, setRemoving] = React.useState<RouterInfo | null>(null)
  const [importFor, setImportFor] = React.useState<RouterInfo | null>(null)

  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-center justify-between gap-3">
        <p className="text-sm text-muted-foreground">
          Mikrotik adalah source of truth; Telemon membaca dan (bila diizinkan) menulis lewat RouterOS API.
        </p>
        <Button onClick={() => setAdding(true)} disabled={!!importReason} title={importReason ?? undefined}>
          <PlusIcon />
          <span className="hidden sm:inline">Tambah router</span>
          <span className="sm:hidden">Tambah</span>
        </Button>
      </div>

      <div className="grid gap-2 md:grid-cols-2 md:gap-3 xl:grid-cols-3">
        {routers.map((r) => {
          const mine = users.filter((u) => u.routerId === r.id)
          const c = { active: 0, inactive: 0, disabled: 0 }
          for (const u of mine) c[u.status]++
          const facts: [string, React.ReactNode][] = [
            ["Model", r.model],
            ["RouterOS", r.version],
            [
              "Koneksi",
              <span className="inline-flex items-center gap-1.5" key="tls">
                {r.tls ? <LockIcon className="size-3.5" /> : <LockOpenIcon className="size-3.5" />}
                {r.tls ? "API-SSL" : "API tanpa TLS"}
              </span>,
            ],
            ["Polling", r.lastPollAt ? formatRelative(r.lastPollAt, now) : "Belum pernah"],
            ["Izin API", r.canWrite ? "Baca & tulis" : "Hanya baca"],
            ["Import terakhir", r.lastImportAt ? formatRelative(r.lastImportAt, now) : "Belum pernah"],
          ]
          return (
            <Card key={r.id} className="gap-2.5">
              <CardHeader>
                <CardTitle className="flex items-center gap-2">
                  <span className="truncate">{r.name}</span>
                  <RouterStatusBadge status={r.status} />
                </CardTitle>
                <CardDescription className="font-mono text-xs">
                  {r.host}:{r.port}
                </CardDescription>
                <CardAction>
                  <DropdownMenu>
                    <DropdownMenuTrigger asChild>
                      <Button variant="ghost" size="icon" aria-label={`Aksi ${r.name}`}>
                        <MoreVerticalIcon />
                      </Button>
                    </DropdownMenuTrigger>
                    <DropdownMenuContent align="end">
                      <DropdownMenuItem onSelect={() => navigate("user", { router: r.id })}>
                        <UsersIcon /> Lihat user
                      </DropdownMenuItem>
                      <DropdownMenuItem disabled={!!importReason} onSelect={() => setEditing(r)}>
                        <PencilIcon /> Edit router
                      </DropdownMenuItem>
                      <DropdownMenuItem
                        disabled={r.status !== "online" || !!importReason}
                        onSelect={() => setImportFor(r)}
                      >
                        <DownloadIcon /> Import dari Mikrotik
                      </DropdownMenuItem>
                      <DropdownMenuSeparator />
                      <DropdownMenuItem variant="destructive" disabled={!!importReason} onSelect={() => setRemoving(r)}>
                        <Trash2Icon /> Hapus dari Telemon
                      </DropdownMenuItem>
                    </DropdownMenuContent>
                  </DropdownMenu>
                </CardAction>
              </CardHeader>
              <CardContent className="space-y-2.5">
                {r.error && (
                  <Alert variant="destructive">
                    <TriangleAlertIcon />
                    <AlertDescription>{r.error}</AlertDescription>
                  </Alert>
                )}
                <dl className="grid grid-cols-2 gap-x-3 gap-y-1.5 text-[13px] sm:grid-cols-3">
                  {facts.map(([k, v]) => (
                    <div key={k} className="min-w-0">
                      <dt className="text-xs text-muted-foreground">{k}</dt>
                      <dd className="truncate">{v}</dd>
                    </div>
                  ))}
                </dl>
                <div className="space-y-1.5">
                  <UserMixBar active={c.active} inactive={c.inactive} disabled={c.disabled} />
                  <p className="text-xs text-muted-foreground">
                    <span className="font-medium text-foreground tabular-nums">{c.active}</span> aktif ·{" "}
                    <span className="font-medium text-foreground tabular-nums">{c.inactive}</span> tidak aktif ·{" "}
                    <span className="font-medium text-foreground tabular-nums">{c.disabled}</span> dinonaktifkan
                  </p>
                </div>
              </CardContent>
            </Card>
          )
        })}
      </div>

      <RouterDialog open={adding} onOpenChange={setAdding} />
      <RouterDialog open={!!editing} onOpenChange={(o) => !o && setEditing(null)} router={editing ?? undefined} />
      <ImportDialog
        open={!!importFor}
        onOpenChange={(o) => !o && setImportFor(null)}
        defaultRouterId={importFor?.id}
      />

      <Dialog open={!!removing} onOpenChange={(o) => !o && setRemoving(null)}>
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle>Hapus {removing?.name}?</DialogTitle>
            <DialogDescription>
              Router berhenti dipantau dan data user serta event-nya dihapus dari
              Telemon. Konfigurasi di router tidak diubah.
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <DialogClose asChild>
              <Button variant="outline">Batal</Button>
            </DialogClose>
            <Button
              variant="destructive"
              onClick={() => {
                const r = removing
                setRemoving(null)
                if (r)
                  api
                    .removeRouter(r.id)
                    .then(() => toast.success(`${r.name} dihapus`))
                    .catch((e: Error) => toast.error(e.message))
              }}
            >
              Hapus
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  )
}

function RouterDialog({
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

function RouterForm({ router, onClose }: { router?: RouterInfo; onClose: () => void }) {
  const { api } = useStore()
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
        toast.success(`${r.name} diperbarui`)
      } else {
        const r = await api.addRouter({ ...base, username, password })
        toast.success(`${r.name} ditambahkan`)
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
      <DialogHeader>
        <DialogTitle>{editing ? `Edit ${router?.name}` : "Tambah router"}</DialogTitle>
        <DialogDescription>
          Pakai akun API khusus. Grup <code>read</code> cukup untuk memantau; tambah hak <code>write</code> bila user dan
          profil PPP ingin diedit dari Telemon. Password disimpan di keyring sistem operasi
          {editing && "; kosongkan username dan password bila tidak diubah"}.
        </DialogDescription>
      </DialogHeader>
      <div className="grid gap-2">
        <Label htmlFor="r-name">Nama</Label>
        <Input id="r-name" name="name" required defaultValue={router?.name} placeholder="RB-Utara" />
      </div>
      <div className="grid grid-cols-[1fr_6rem] gap-3">
        <div className="grid gap-2">
          <Label htmlFor="r-host">Host / IP</Label>
          <Input id="r-host" name="host" required defaultValue={router?.host} placeholder="10.10.5.1" inputMode="url" />
        </div>
        <div className="grid gap-2">
          <Label htmlFor="r-port">Port</Label>
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
          <p className="text-xs text-muted-foreground">Disarankan. Port bawaan 8729; tanpa TLS 8728.</p>
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
          <Label htmlFor="r-write">Izinkan edit user dan profil PPP</Label>
          <p className="text-xs text-muted-foreground">
            Nyalakan hanya bila akun API di bawah punya hak write. Setiap tulis ke router tetap meminta konfirmasi.
          </p>
        </div>
        <Switch id="r-write" checked={allowWrite} onCheckedChange={setAllowWrite} />
      </div>
      <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
        <div className="grid gap-2">
          <Label htmlFor="r-user">Username API</Label>
          <Input id="r-user" name="username" required={!editing} autoComplete="off" placeholder={editing ? "tetap" : "telemon"} />
        </div>
        <div className="grid gap-2">
          <Label htmlFor="r-pass">Password</Label>
          <Input id="r-pass" name="password" type="password" required={!editing} autoComplete="new-password" placeholder={editing ? "tetap" : undefined} />
        </div>
      </div>
      <DialogFooter>
        <Button type="button" variant="outline" onClick={onClose} disabled={busy}>
          Batal
        </Button>
        <Button type="submit" disabled={busy}>
          {busy ? "Menghubungkan…" : "Simpan"}
        </Button>
      </DialogFooter>
    </form>
  )
}
