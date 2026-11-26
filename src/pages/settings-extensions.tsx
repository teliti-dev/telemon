import * as React from "react"
import { Loader2Icon, PuzzleIcon, ShieldAlertIcon, Trash2Icon, UploadIcon } from "lucide-react"
import { toast } from "sonner"

import { ConfirmDialog } from "@/components/confirm-dialog"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog"
import { Skeleton } from "@/components/ui/skeleton"
import { Switch } from "@/components/ui/switch"
import { useUpdateGate } from "@/hooks/use-write-gate"
import { reloadExtensions } from "@/lib/extensions/loader"
import { useT } from "@/lib/i18n"
import { useStore } from "@/lib/store-context"
import type { ExtensionInfo, ExtensionInspect, ExtensionList, ExtensionRequirement } from "@/lib/types"

const errMsg = (e: unknown) => (e instanceof Error ? e.message : String(e))

function Permissions({
  perms,
  granted,
  locked,
  onToggle,
}: {
  perms: { id: string; required: boolean; sensitive: boolean; description: string }[]
  granted: (id: string) => boolean
  locked: boolean
  onToggle: (id: string, on: boolean) => void
}) {
  const t = useT()
  if (perms.length === 0) return <p className="text-sm text-muted-foreground">{t("Tidak meminta izin apa pun.")}</p>
  return (
    <ul className="divide-y rounded-lg border">
      {perms.map((p) => (
        <li key={p.id} className="flex items-center gap-3 p-3">
          <div className="min-w-0 flex-1">
            <div className="flex flex-wrap items-center gap-2 text-sm font-medium">
              <code className="text-xs">{p.id}</code>
              {p.required && <Badge variant="secondary">{t("Wajib")}</Badge>}
              {p.sensitive && (
                <Badge variant="outline" className="gap-1">
                  <ShieldAlertIcon className="size-3" /> {t("Sensitif")}
                </Badge>
              )}
            </div>
            {p.description && <p className="text-xs text-muted-foreground">{p.description}</p>}
          </div>
          <Switch checked={granted(p.id)} disabled={locked} aria-label={p.id} onCheckedChange={(v) => onToggle(p.id, v)} />
        </li>
      ))}
    </ul>
  )
}

const STATUS = { disabled: "Mati", limited: "Terbatas", incompatible: "Tidak cocok", ok: "Aktif", broken: "Rusak" } as const

function Requires({ list }: { list: ExtensionRequirement[] }) {
  const t = useT()
  if (list.length === 0) return null
  return (
    <ul className="grid gap-1 text-xs">
      {list.map((r) => (
        <li key={r.id} className="flex flex-wrap items-center gap-2">
          <code>{r.id}</code>
          <span className="text-muted-foreground">{r.needs}</span>
          {r.optional && <Badge variant="outline">{t("Opsional")}</Badge>}
          <Badge variant={r.state === "ok" ? "secondary" : "destructive"}>
            {r.state === "ok" ? t("Terpasang v{v}", { v: r.installed ?? "" }) : r.state === "missing" ? t("Belum terpasang") : t("Versi tidak cocok (v{v})", { v: r.installed ?? "" })}
          </Badge>
        </li>
      ))}
    </ul>
  )
}

function ExtensionCard({ ext, locked, onChanged, onRemove }: { ext: ExtensionInfo; locked: boolean; onChanged: (e: ExtensionInfo) => void; onRemove: () => void }) {
  const t = useT()
  const { api } = useStore()
  const [busy, setBusy] = React.useState(false)

  async function patch(p: { enabled?: boolean; perms?: Record<string, boolean> }) {
    setBusy(true)
    try {
      const next = await api.updateExtension(ext.id, p)
      onChanged(next)
      await reloadExtensions(() => api.getActiveExtensions())
    } catch (e) {
      toast.error(errMsg(e))
    } finally {
      setBusy(false)
    }
  }

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex flex-wrap items-center gap-2">
          <PuzzleIcon className="size-4" /> {ext.name}
          <span className="text-xs font-normal text-muted-foreground">v{ext.version}</span>
          <Badge variant={ext.tier === "official" ? "default" : "outline"}>{ext.tier === "official" ? t("Official") : t("Publik")}</Badge>
          {/* i18n-ignore: kunci ada di STATUS */}
          <Badge variant={ext.status === "ok" ? "secondary" : "outline"}>{t(STATUS[ext.status])}</Badge>
        </CardTitle>
        <CardDescription>
          {ext.publisher}
          {ext.description ? ` · ${ext.description}` : ""}
        </CardDescription>
      </CardHeader>
      <CardContent className="grid gap-4">
        <div className="flex items-center justify-between gap-3 rounded-lg border p-3">
          <div>
            <div className="text-sm font-medium">{t("Aktifkan extension")}</div>
            <p className="text-xs text-muted-foreground">{t("Yang mati tidak dimuat dan tidak disajikan.")}</p>
          </div>
          <Switch checked={ext.enabled} disabled={locked || busy} aria-label={t("Aktifkan extension")} onCheckedChange={(v) => patch({ enabled: v })} />
        </div>
        {ext.status === "incompatible" && (
          <Alert variant="destructive">
            <ShieldAlertIcon />
            <AlertDescription>{t("Versi Telemon ini di luar rentang yang didukung extension ({range}), jadi tidak dimuat. Perbarui extension.", { range: ext.telemon ?? "" })}</AlertDescription>
          </Alert>
        )}
        {ext.status === "limited" && (
          <Alert>
            <ShieldAlertIcon />
            <AlertDescription>
              {ext.blockedBy.length > 0
                ? t("Butuh extension lain yang aktif dan cocok versinya: {list}.", { list: ext.blockedBy.join(", ") })
                : t("Ada izin wajib yang dimatikan, jadi extension ini tidak dimuat sampai izinnya dinyalakan.")}
            </AlertDescription>
          </Alert>
        )}
        {(ext.requires.length > 0 || ext.telemon) && (
          <div className="grid gap-2">
            <div className="text-sm font-medium">{t("Kompatibilitas")}</div>
            {ext.telemon && <p className="text-xs text-muted-foreground">{t("Versi Telemon: {range}", { range: ext.telemon })}</p>}
            <Requires list={ext.requires} />
          </div>
        )}
        {ext.dependents.length > 0 && (
          <p className="text-xs text-muted-foreground">{t("Dibutuhkan oleh: {list}", { list: ext.dependents.join(", ") })}</p>
        )}
        {ext.widgets.length > 0 && (
          <p className="text-xs text-muted-foreground">
            {t("Menambah widget Ringkasan")}: {ext.widgets.map((w) => w.title).join(", ")}
          </p>
        )}
        <div className="grid gap-2">
          <div className="text-sm font-medium">{t("Izin")}</div>
          <Permissions
            perms={ext.permissions}
            granted={(id) => ext.permissions.find((p) => p.id === id)?.granted ?? false}
            locked={locked || busy}
            onToggle={(id, on) => patch({ perms: { [id]: on } })}
          />
        </div>
        <div className="flex justify-end">
          <Button variant="outline" size="sm" disabled={locked || busy || ext.dependents.length > 0} title={ext.dependents.length > 0 ? t("Masih dibutuhkan extension lain") : undefined} onClick={onRemove}>
            <Trash2Icon /> {t("Hapus")}
          </Button>
        </div>
      </CardContent>
    </Card>
  )
}

function InstallDialog({ inspect, file, onClose, onInstalled }: { inspect: ExtensionInspect | null; file: File | null; onClose: () => void; onInstalled: (e: ExtensionInfo) => void }) {
  const t = useT()
  const { api } = useStore()
  const [busy, setBusy] = React.useState(false)
  const p = inspect?.package

  async function install() {
    if (!file) return
    setBusy(true)
    try {
      const e = await api.installExtension(file)
      toast.success(t("Extension dipasang. Nyalakan untuk memakainya."))
      onInstalled(e)
      onClose()
    } catch (e) {
      toast.error(errMsg(e))
    } finally {
      setBusy(false)
    }
  }

  return (
    <Dialog open={!!inspect} onOpenChange={(o) => !o && !busy && onClose()}>
      <DialogContent className="max-h-[88svh] overflow-y-auto sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>{inspect?.installedVersion ? t("Perbarui extension") : t("Pasang extension")}</DialogTitle>
          <DialogDescription>{t("Periksa penerbit dan izin yang diminta. Izin sensitif mulai mati dan bisa diubah kapan saja.")}</DialogDescription>
        </DialogHeader>
        {p && (
          <div className="grid gap-3 text-sm">
            <div>
              <div className="font-medium">
                {p.name} <span className="text-xs font-normal text-muted-foreground">v{p.version}</span>
              </div>
              <div className="text-xs text-muted-foreground">
                {p.publisher} · {p.id}
              </div>
              {inspect?.installedVersion && <div className="text-xs text-muted-foreground">{t("Terpasang: v{v}", { v: inspect.installedVersion })}</div>}
            </div>
            <div className="flex gap-2">
              <Badge>{p.tier === "official" ? t("Official") : t("Publik")}</Badge>
              <Badge variant="secondary">{p.signed ? t("Tanda tangan valid") : t("Tanpa tanda tangan")}</Badge>
            </div>
            {inspect && inspect.requires.length > 0 && (
              <div className="grid gap-1">
                <div className="font-medium">{t("Membutuhkan")}</div>
                <Requires list={inspect.requires} />
              </div>
            )}
            {inspect && inspect.problemTexts.length > 0 && (
              <Alert variant="destructive">
                <AlertTitle>{t("Tidak bisa dipasang")}</AlertTitle>
                <AlertDescription>
                  <ul className="list-disc pl-4">
                    {inspect.problemTexts.map((x) => (
                      <li key={x}>{x}</li>
                    ))}
                  </ul>
                </AlertDescription>
              </Alert>
            )}
            <Permissions perms={p.permissions} granted={(id) => !p.permissions.find((x) => x.id === id)?.sensitive} locked onToggle={() => {}} />
          </div>
        )}
        <DialogFooter>
          <Button variant="outline" onClick={onClose} disabled={busy}>
            {t("Batal")}
          </Button>
          <Button onClick={install} disabled={busy || (inspect?.problemTexts.length ?? 0) > 0}>
            {busy && <Loader2Icon className="animate-spin" />} {inspect?.installedVersion ? t("Perbarui") : t("Pasang")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}

/** Pengaturan > Extension (`#/pengaturan/extension`): pasang, aktifkan, dan atur izin extension (ADR 0028). */
export function SettingsExtensionsPage() {
  const t = useT()
  const { api, access } = useStore()
  const reason = useUpdateGate("admin")()
  const [list, setList] = React.useState<ExtensionList | null>(null)
  const [error, setError] = React.useState<string | null>(null)
  const [inspect, setInspect] = React.useState<ExtensionInspect | null>(null)
  const [file, setFile] = React.useState<File | null>(null)
  const [removing, setRemoving] = React.useState<ExtensionInfo | null>(null)
  const input = React.useRef<HTMLInputElement>(null)

  const load = React.useCallback(() => {
    api
      .getExtensions()
      .then((l) => {
        setList(l)
        setError(null)
      })
      .catch((e) => setError(errMsg(e)))
  }, [api])
  React.useEffect(load, [load])

  async function pick(f: File | undefined) {
    if (!f) return
    try {
      const i = await api.inspectExtension(f)
      setFile(f)
      setInspect(i)
    } catch (e) {
      toast.error(errMsg(e))
    } finally {
      if (input.current) input.current.value = ""
    }
  }

  const upsert = (e: ExtensionInfo) => setList((l) => (l ? { ...l, extensions: [...l.extensions.filter((x) => x.id !== e.id), e].sort((a, b) => a.id.localeCompare(b.id)) } : l))

  if (error) {
    return (
      <Alert variant="destructive" className="max-w-lg">
        <AlertTitle>{t("Gagal memuat extension")}</AlertTitle>
        <AlertDescription>{error}</AlertDescription>
      </Alert>
    )
  }
  if (!list) return <Skeleton className="h-40 max-w-3xl" />

  if (!list.available) {
    return (
      <Alert className="max-w-2xl">
        <PuzzleIcon />
        <AlertTitle>{t("Extension mati di server ini")}</AlertTitle>
        <AlertDescription>
          <p>{t("Nyalakan dengan variabel lingkungan TELEMON_EXTENSIONS=1 lalu mulai ulang server. Setelah itu extension dipasang dan diatur dari halaman ini.")}</p>
        </AlertDescription>
      </Alert>
    )
  }

  const locked = !!reason || access === "remote" ? !!reason : false
  return (
    <div className="grid max-w-3xl gap-4">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <div>
          <h2 className="text-lg font-semibold">{t("Extension")}</h2>
          <p className="text-sm text-muted-foreground">{t("Tambahan fitur dari pihak Teliti. Hanya extension official yang bisa dipasang pada versi ini.")}</p>
        </div>
        <div>
          <input ref={input} type="file" accept=".tmx" className="hidden" onChange={(e) => pick(e.target.files?.[0])} />
          <Button disabled={locked} title={reason ?? undefined} onClick={() => input.current?.click()}>
            <UploadIcon /> {t("Pasang dari berkas")}
          </Button>
        </div>
      </div>
      {reason && (
        <Alert>
          <AlertDescription>{t("{reason}. Anda hanya bisa melihat extension.", { reason })}</AlertDescription>
        </Alert>
      )}
      {list.extensions.length === 0 ? (
        <p className="text-sm text-muted-foreground">{t("Belum ada extension terpasang.")}</p>
      ) : (
        list.extensions.map((e) => <ExtensionCard key={e.id} ext={e} locked={locked} onChanged={upsert} onRemove={() => setRemoving(e)} />)
      )}
      <InstallDialog
        inspect={inspect}
        file={file}
        onClose={() => {
          setInspect(null)
          setFile(null)
        }}
        onInstalled={(e) => {
          upsert(e)
          load()
        }}
      />
      <ConfirmDialog
        open={!!removing}
        onOpenChange={(o) => !o && setRemoving(null)}
        title={t("Hapus extension?")}
        description={removing ? t("{name} dan pengaturannya dihapus. Widget-nya hilang dari Ringkasan.", { name: removing.name }) : ""}
        confirmLabel={t("Hapus")}
        destructive
        onConfirm={async () => {
          if (!removing) return
          await api.removeExtension(removing.id)
          setList((l) => (l ? { ...l, extensions: l.extensions.filter((x) => x.id !== removing.id) } : l))
          await reloadExtensions(() => api.getActiveExtensions())
        }}
      />
    </div>
  )
}
