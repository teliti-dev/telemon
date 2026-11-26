import * as React from "react"
import { ExternalLinkIcon, KeyRoundIcon, Loader2Icon, PuzzleIcon, ShieldAlertIcon, Trash2Icon, UploadIcon } from "lucide-react"
import { toast } from "sonner"

import { ConfirmDialog } from "@/components/confirm-dialog"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import { Checkbox } from "@/components/ui/checkbox"
import { Label } from "@/components/ui/label"
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog"
import { Skeleton } from "@/components/ui/skeleton"
import { Switch } from "@/components/ui/switch"
import { useUpdateGate } from "@/hooks/use-write-gate"
import { reloadExtensions } from "@/lib/extensions/loader"
import { tk, useT } from "@/lib/i18n"
import { useStore } from "@/lib/store-context"
import { pathHref } from "@/lib/route"
import type { ExtensionAccess, ExtensionInfo, ExtensionInspect, ExtensionLevel, ExtensionList, ExtensionRequirement } from "@/lib/types"

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

const LEVELS: ExtensionLevel[] = ["none", "read", "update"]
const LEVEL_LABEL = { none: tk("Tanpa akses"), read: tk("Lihat"), update: tk("Ubah") } as const
const rank = (l: ExtensionLevel) => LEVELS.indexOf(l)

/** Matriks akses per peran untuk satu extension: level menu dan aksi yang dideklarasikan manifest (ADR 0028). */
function AccessDialog({ ext, locked, onClose }: { ext: ExtensionInfo | null; locked: boolean; onClose: () => void }) {
  const t = useT()
  const { api } = useStore()
  const [data, setData] = React.useState<ExtensionAccess | null>(null)
  const [error, setError] = React.useState<string | null>(null)
  const [busy, setBusy] = React.useState<string | null>(null)
  const id = ext?.id

  React.useEffect(() => {
    if (!id) return
    let cancelled = false
    api
      .getExtensionAccess(id)
      .then((d) => !cancelled && (setData(d), setError(null)))
      .catch((e) => !cancelled && setError(errMsg(e)))
    return () => {
      cancelled = true
    }
  }, [api, id])

  async function save(roleId: string, level: ExtensionLevel, actions: string[]) {
    if (!id) return
    setBusy(roleId)
    try {
      await api.setExtensionAccess(id, roleId, { level, actions })
      setData((d) => (d ? { ...d, roles: d.roles.map((r) => (r.id === roleId ? { ...r, level, actions } : r)) } : d))
      await reloadExtensions(() => api.getActiveExtensions())
    } catch (e) {
      toast.error(errMsg(e))
    } finally {
      setBusy(null)
    }
  }

  return (
    <Dialog open={!!ext} onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-h-[88svh] overflow-y-auto sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle>{t("Akses {name}", { name: ext?.name ?? "" })}</DialogTitle>
          <DialogDescription>{t("Atur peran mana yang boleh membuka extension ini dan aksi apa yang boleh dilakukan. Anda tidak bisa memberi lebih daripada akses Anda sendiri atau mengubah peran Anda sendiri.")}</DialogDescription>
        </DialogHeader>
        {error && (
          <Alert variant="destructive">
            <AlertDescription>{error}</AlertDescription>
          </Alert>
        )}
        {!data && !error && <Skeleton className="h-24" />}
        {data && (
          <ul className="grid gap-3">
            {data.roles.map((r) => {
              const mine = rank(data.me.level)
              const off = locked || r.own || busy === r.id || rank(r.level) > mine
              return (
                <li key={r.id} className="grid gap-2 rounded-lg border p-3">
                  <div className="flex flex-wrap items-center gap-2">
                    <span className="text-sm font-medium">{r.name}</span>
                    {r.own && <Badge variant="outline">{t("Peran Anda")}</Badge>}
                    <select
                      aria-label={t("Level akses {name}", { name: r.name })}
                      className="ml-auto h-9 rounded-md border bg-background px-2 text-sm md:h-7"
                      value={r.level}
                      disabled={off}
                      onChange={(e) => {
                        const level = e.target.value as ExtensionLevel
                        const keep = r.actions.filter((a) => rank(data.actions.find((d) => d.id === a)?.level ?? "read") <= rank(level))
                        void save(r.id, level, level === "none" ? [] : keep)
                      }}
                    >
                      {LEVELS.filter((l) => rank(l) <= mine).map((l) => (
                        <option key={l} value={l}>
                          {t(LEVEL_LABEL[l] /* i18n-ignore: kunci tk */)}
                        </option>
                      ))}
                    </select>
                  </div>
                  {data.actions.length > 0 && r.level !== "none" && (
                    <div className="grid gap-1.5">
                      {data.actions.map((a) => {
                        const need = rank(a.level) > rank(r.level) || !data.me.actions.includes(a.id)
                        return (
                          <div key={a.id} className="flex items-start gap-2">
                            <Checkbox
                              id={`${r.id}-${a.id}`}
                              checked={r.actions.includes(a.id)}
                              disabled={off || need}
                              onCheckedChange={(v) => void save(r.id, r.level, v === true ? [...r.actions, a.id] : r.actions.filter((x) => x !== a.id))}
                            />
                            <Label htmlFor={`${r.id}-${a.id}`} className="text-sm font-normal">
                              {a.label}
                              {a.level === "update" && <span className="ml-2 text-xs text-muted-foreground">({t("butuh Ubah")})</span>}
                              {a.description && <span className="block text-xs text-muted-foreground">{a.description}</span>}
                            </Label>
                          </div>
                        )
                      })}
                    </div>
                  )}
                </li>
              )
            })}
          </ul>
        )}
        <DialogFooter>
          <Button variant="outline" onClick={onClose}>
            {t("Tutup")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}

function ExtensionCard({ ext, locked, onChanged, onRemove, onAccess }: { ext: ExtensionInfo; locked: boolean; onChanged: (e: ExtensionInfo) => void; onRemove: () => void; onAccess: () => void }) {
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
        <div className="flex flex-wrap justify-end gap-2">
          {ext.enabled && ext.status === "ok" && (ext.pages ?? []).map((pg) => (
            <Button key={pg.id} variant="outline" size="sm" asChild>
              <a href={pathHref("ext", [ext.id, pg.id])}>
                <ExternalLinkIcon /> {pg.title}
              </a>
            </Button>
          ))}
          <Button variant="outline" size="sm" onClick={onAccess}>
            <KeyRoundIcon /> {t("Akses")}
          </Button>
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
  const [deleteData, setDeleteData] = React.useState(false)
  const [accessFor, setAccessFor] = React.useState<ExtensionInfo | null>(null)
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
        list.extensions.map((e) => <ExtensionCard key={e.id} ext={e} locked={locked} onChanged={upsert} onRemove={() => setRemoving(e)} onAccess={() => setAccessFor(e)} />)
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
      <AccessDialog ext={accessFor} locked={locked} onClose={() => setAccessFor(null)} />
      <ConfirmDialog
        open={!!removing}
        onOpenChange={(o) => !o && setRemoving(null)}
        title={t("Hapus extension?")}
        description={
          removing ? (
            <div className="grid gap-3">
              <p>{t("{name} dan pengaturannya dihapus. Widget-nya hilang dari Ringkasan.", { name: removing.name })}</p>
              {removing.hasData && (
                <div className="flex items-start gap-2">
                  <Checkbox id="ext-del-data" checked={deleteData} onCheckedChange={(v) => setDeleteData(v === true)} />
                  <Label htmlFor="ext-del-data" className="text-sm font-normal">
                    {t("Hapus juga data extension ini (tabel di database). Tanpa centang, data disimpan dan dipakai lagi bila dipasang ulang.")}
                  </Label>
                </div>
              )}
            </div>
          ) : (
            ""
          )
        }
        confirmLabel={t("Hapus")}
        destructive
        onConfirm={async () => {
          if (!removing) return
          await api.removeExtension(removing.id, { deleteData: removing.hasData && deleteData })
          setDeleteData(false)
          setList((l) => (l ? { ...l, extensions: l.extensions.filter((x) => x.id !== removing.id) } : l))
          await reloadExtensions(() => api.getActiveExtensions())
        }}
      />
    </div>
  )
}
