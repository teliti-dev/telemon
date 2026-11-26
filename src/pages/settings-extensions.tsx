import * as React from "react"
import { LayoutGridIcon, ListIcon, Loader2Icon, PlusIcon, PuzzleIcon, SearchIcon } from "lucide-react"
import { toast } from "sonner"

import { InstallChooserDialog } from "@/components/extensions/install-chooser"
import { LicenseInfo, LicenseNote, Permissions, Requires, StatusBadge, TierBadge, ToggleExtensionDialog, errMsg, useExtensionPatch } from "@/components/extensions/parts"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Card } from "@/components/ui/card"
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog"
import { Input } from "@/components/ui/input"
import { Skeleton } from "@/components/ui/skeleton"
import { Switch } from "@/components/ui/switch"
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table"
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group"
import { useUpdateGate } from "@/hooks/use-write-gate"
import { useT } from "@/lib/i18n"
import { navigatePath, pathHref } from "@/lib/route"
import { useStore } from "@/lib/store-context"
import type { ExtensionInfo, ExtensionInspect, ExtensionList } from "@/lib/types"
import { cn } from "@/lib/utils"

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
            <div className="grid gap-1">
              <div className="font-medium">{t("Lisensi")}</div>
              <LicenseInfo license={p.license} />
              <LicenseNote />
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

type ViewMode = "grid" | "list"
const VIEW_KEY = "telemon.extensions.view"
function loadView(): ViewMode {
  try {
    return localStorage.getItem(VIEW_KEY) === "list" ? "list" : "grid"
  } catch {
    return "grid"
  }
}
function saveView(v: ViewMode) {
  try {
    localStorage.setItem(VIEW_KEY, v)
  } catch {
    /* penyimpanan browser tidak tersedia; pilihan hanya berlaku sesi ini */
  }
}

/** Ringkasan isi extension untuk kartu dan baris: berapa widget, halaman, dan apakah punya backend. */
function Contributes({ ext }: { ext: ExtensionInfo }) {
  const t = useT()
  const bits = [
    ext.widgets.length > 0 && t("{n} widget", { n: ext.widgets.length }),
    (ext.pages ?? []).length > 0 && t("{n} halaman", { n: ext.pages.length }),
    ext.hasBackend && t("Backend"),
  ].filter(Boolean)
  return <span className="truncate text-xs text-muted-foreground">{bits.length ? bits.join(" · ") : t("Hanya tampilan")}</span>
}

/** Pengaturan > Extension (`#/pengaturan/extension`): toko, daftar (grid atau list seperti Users), dan pasang dari berkas (ADR 0028). */
export function SettingsExtensionsPage() {
  const t = useT()
  const { api } = useStore()
  const reason = useUpdateGate("admin")()
  const [list, setList] = React.useState<ExtensionList | null>(null)
  const [error, setError] = React.useState<string | null>(null)
  const [inspect, setInspect] = React.useState<ExtensionInspect | null>(null)
  const [file, setFile] = React.useState<File | null>(null)
  const [query, setQuery] = React.useState("")
  const [view, setView] = React.useState<ViewMode>(loadView)
  const [toggle, setToggle] = React.useState<{ ext: ExtensionInfo; enable: boolean } | null>(null)
  const [choosing, setChoosing] = React.useState(false)
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

  const upsert = React.useCallback((e: ExtensionInfo) => setList((l) => (l ? { ...l, extensions: [...l.extensions.filter((x) => x.id !== e.id), e].sort((a, b) => a.id.localeCompare(b.id)) } : l)), [])
  const patch = useExtensionPatch(upsert)
  const filtered = React.useMemo(() => {
    const q = query.trim().toLowerCase()
    return (list?.extensions ?? []).filter((e) => !q || `${e.name} ${e.id} ${e.publisher} ${e.description}`.toLowerCase().includes(q))
  }, [list, query])

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

  if (error) {
    return (
      <Alert variant="destructive" className="mx-auto max-w-lg">
        <AlertTitle>{t("Gagal memuat extension")}</AlertTitle>
        <AlertDescription>{error}</AlertDescription>
      </Alert>
    )
  }
  if (!list) return <Skeleton className="mx-auto h-40 max-w-3xl" />

  if (!list.available) {
    return (
      <Alert className="mx-auto max-w-2xl">
        <PuzzleIcon />
        <AlertTitle>{t("Extension mati di server ini")}</AlertTitle>
        <AlertDescription>
          <p>{t("Nyalakan dengan variabel lingkungan TELEMON_EXTENSIONS=1 lalu mulai ulang server. Setelah itu extension dipasang dan diatur dari halaman ini.")}</p>
        </AlertDescription>
      </Alert>
    )
  }

  const locked = !!reason
  const open = (id: string) => navigatePath("pengaturan", ["extension", id])
  const askToggle = (ext: ExtensionInfo, enable: boolean) => setToggle({ ext, enable })

  const viewToggle = (
    <ToggleGroup
      type="single"
      variant="outline"
      value={view}
      onValueChange={(v) => {
        if (v) {
          setView(v as ViewMode)
          saveView(v as ViewMode)
        }
      }}
      aria-label={t("Tampilan")}
      className="shrink-0"
    >
      <ToggleGroupItem value="list" aria-label={t("Tampilan list")} title={t("List")}>
        <ListIcon />
      </ToggleGroupItem>
      <ToggleGroupItem value="grid" aria-label={t("Tampilan grid")} title={t("Grid")}>
        <LayoutGridIcon />
      </ToggleGroupItem>
    </ToggleGroup>
  )

  return (
    <div className="flex flex-col gap-3">
      <div>
        <h2 className="text-lg font-semibold">{t("Extension")}</h2>
        <p className="text-sm text-muted-foreground">{t("Tambahan fitur dari pihak Teliti. Hanya extension official yang bisa dipasang pada versi ini.")}</p>
      </div>
      {/* Toolbar selebar halaman seperti Users: cari melebar, tampilan dan tombol Pasang di ujung kanan. */}
      <div className="flex items-center gap-2">
        <div className="relative min-w-0 flex-1">
          <SearchIcon className="pointer-events-none absolute top-1/2 left-2.5 size-4 -translate-y-1/2 text-muted-foreground" />
          <Input type="search" value={query} onChange={(e) => setQuery(e.target.value)} placeholder={t("Cari extension…")} aria-label={t("Cari extension")} className="pl-8" />
        </div>
        {viewToggle}
        <input ref={input} type="file" accept=".tmx" className="hidden" onChange={(e) => pick(e.target.files?.[0])} />
        <Button className="shrink-0" disabled={locked} title={reason ?? undefined} onClick={() => setChoosing(true)}>
          <PlusIcon /> <span className="hidden sm:inline">{t("Pasang extension")}</span>
          <span className="sr-only sm:hidden">{t("Pasang extension")}</span>
        </Button>
      </div>
      {reason && (
        <Alert>
          <AlertDescription>{t("{reason}. Anda hanya bisa melihat extension.", { reason })}</AlertDescription>
        </Alert>
      )}
      {list.officialKeys === 0 && (
        <Alert>
          <PuzzleIcon />
          <AlertTitle>{t("Kunci official belum disematkan")}</AlertTitle>
          <AlertDescription>{t("Build ini belum mempercayai kunci penanda tangan official mana pun, jadi extension belum bisa dipasang dari toko maupun berkas. Pemilik server bisa menambah kunci dengan TELEMON_EXTENSIONS_OFFICIAL_KEYS.")}</AlertDescription>
        </Alert>
      )}

      {list.extensions.length === 0 ? (
        <Card className="items-center py-12 text-sm text-muted-foreground">{t("Belum ada extension terpasang.")}</Card>
      ) : filtered.length === 0 ? (
        <Card className="items-center py-12 text-sm text-muted-foreground">{t("Tidak ada extension yang cocok.")}</Card>
      ) : view === "grid" ? (
        <ul className="grid grid-cols-1 gap-2 min-[420px]:grid-cols-2 lg:grid-cols-3">
          {filtered.map((e) => (
            <li key={e.id} className="min-w-0">
              <div className={cn("relative flex h-full flex-col gap-1.5 rounded-xl border border-l-4 bg-card p-3 transition-colors hover:bg-muted/40 focus-within:ring-3 focus-within:ring-ring/50", e.enabled && e.status === "ok" ? "border-l-brand" : "border-l-border")}>
                <div className="flex items-start gap-2">
                  <PuzzleIcon className="mt-0.5 size-4 shrink-0 text-muted-foreground" />
                  <a href={pathHref("pengaturan", ["extension", e.id])} className="min-w-0 flex-1 outline-none after:absolute after:inset-0 after:rounded-xl">
                    <span className="block truncate text-sm font-medium">{e.name}</span>
                    <span className="block truncate text-xs text-muted-foreground">
                      v{e.version} · {e.publisher}
                    </span>
                  </a>
                  <Switch className="relative z-10" checked={e.enabled} disabled={locked} aria-label={t("Aktifkan {name}", { name: e.name })} onCheckedChange={(v) => askToggle(e, v)} />
                </div>
                {e.description && <p className="line-clamp-2 text-xs text-muted-foreground">{e.description}</p>}
                <div className="mt-auto flex flex-wrap items-center gap-1.5 border-t pt-1.5">
                  <TierBadge ext={e} />
                  <StatusBadge ext={e} />
                  <span className="ml-auto min-w-0">
                    <Contributes ext={e} />
                  </span>
                </div>
              </div>
            </li>
          ))}
        </ul>
      ) : (
        <Card className="py-0">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead className="pl-3">{t("Extension")}</TableHead>
                <TableHead className="hidden md:table-cell">{t("Penerbit")}</TableHead>
                <TableHead>{t("Tingkat")}</TableHead>
                <TableHead>{t("Status")}</TableHead>
                <TableHead className="hidden lg:table-cell">{t("Isi")}</TableHead>
                <TableHead className="pr-3 text-right">{t("Aktif")}</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {filtered.map((e) => (
                <TableRow key={e.id} tabIndex={0} onClick={() => open(e.id)} onKeyDown={(ev) => ev.key === "Enter" && open(e.id)} className="cursor-pointer">
                  <TableCell className="pl-3">
                    <span className="font-medium">{e.name}</span> <span className="text-xs text-muted-foreground">v{e.version}</span>
                    <span className="block font-mono text-xs text-muted-foreground">{e.id}</span>
                  </TableCell>
                  <TableCell className="hidden text-muted-foreground md:table-cell">{e.publisher}</TableCell>
                  <TableCell>
                    <TierBadge ext={e} />
                  </TableCell>
                  <TableCell>
                    <StatusBadge ext={e} />
                  </TableCell>
                  <TableCell className="hidden lg:table-cell">
                    <Contributes ext={e} />
                  </TableCell>
                  <TableCell className="pr-3 text-right" onClick={(ev) => ev.stopPropagation()}>
                    <Switch checked={e.enabled} disabled={locked} aria-label={t("Aktifkan {name}", { name: e.name })} onCheckedChange={(v) => askToggle(e, v)} />
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </Card>
      )}

      <InstallChooserDialog
        open={choosing}
        onOpenChange={setChoosing}
        onStore={() => {
          setChoosing(false)
          navigatePath("pengaturan", ["extension", "store"])
        }}
        onUpload={() => {
          setChoosing(false)
          input.current?.click()
        }}
      />
      <InstallDialog
        inspect={inspect}
        file={file}
        onClose={() => {
          setInspect(null)
          setFile(null)
        }}
        onInstalled={(e) => {
          upsert(e)
          // Setelah pasang: ke detail untuk mengaktifkan dan memberi izin.
          navigatePath("pengaturan", ["extension", e.id])
        }}
      />
      <ToggleExtensionDialog
        target={toggle}
        onClose={() => setToggle(null)}
        apply={async (id, enabled) => {
          await patch(id, { enabled })
        }}
      />
    </div>
  )
}
