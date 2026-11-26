import * as React from "react"
import { ArrowLeftIcon, Loader2Icon, PuzzleIcon, SearchIcon, ShieldAlertIcon } from "lucide-react"
import { toast } from "sonner"

import { errMsg, TierBadge } from "@/components/extensions/parts"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Card } from "@/components/ui/card"
import { Checkbox } from "@/components/ui/checkbox"
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Skeleton } from "@/components/ui/skeleton"
import { useUpdateGate } from "@/hooks/use-write-gate"
import { reloadExtensions } from "@/lib/extensions/loader"
import { useT } from "@/lib/i18n"
import { navigatePath, replacePath } from "@/lib/route"
import { useStore } from "@/lib/store-context"
import type { ExtensionStore, ExtensionStoreItem } from "@/lib/types"
import { cn } from "@/lib/utils"

type Filter = "all" | "new" | "installed" | "update"

const size = (n: number) => (n >= 1 << 20 ? `${(n / (1 << 20)).toFixed(1)} MB` : `${Math.max(1, Math.round(n / 1024))} KB`)

/** Fitur yang ditambahkan, dalam bahasa manusia (widget, halaman, backend, terminal, worker). */
function features(e: ExtensionStoreItem, t: ReturnType<typeof useT>) {
  return [
    e.widgets.length > 0 && t("{n} widget Ringkasan", { n: e.widgets.length }),
    e.pages.length > 0 && t("{n} halaman", { n: e.pages.length }),
    e.hasBackend && t("Backend (rute API dan data sendiri)"),
    e.workers > 0 && t("{n} tugas terjadwal", { n: e.workers }),
    e.sessions.some((s) => s.kind === "router-ssh") && t("Terminal SSH ke router"),
  ].filter(Boolean) as string[]
}

/** Popup detail extension di toko: fitur, izin (wajib dan sensitif), syarat, lalu pasang dengan verifikasi. */
function StoreDetailDialog({ item, locked, onClose, onInstalled }: { item: ExtensionStoreItem | null; locked: boolean; onClose: () => void; onInstalled: (id: string) => void }) {
  const t = useT()
  const { api } = useStore()
  const [verify, setVerify] = React.useState(false)
  const [checked, setChecked] = React.useState(false)
  const [busy, setBusy] = React.useState(false)
  const [prev, setPrev] = React.useState<string | null>(null)
  if ((item?.id ?? null) !== prev) {
    setPrev(item?.id ?? null)
    setVerify(false)
    setChecked(false)
  }

  async function install() {
    if (!item) return
    setBusy(true)
    try {
      await api.installFromStore(item.id)
      await reloadExtensions(() => api.getActiveExtensions())
      toast.success(t("{name} dipasang. Aktifkan dan atur izinnya di sini.", { name: item.name }))
      onInstalled(item.id)
    } catch (e) {
      toast.error(errMsg(e))
    } finally {
      setBusy(false)
    }
  }

  const sensitive = item?.permissions.filter((p) => p.sensitive) ?? []
  const feats = item ? features(item, t) : []
  return (
    <Dialog open={!!item} onOpenChange={(o) => !o && !busy && onClose()}>
      <DialogContent className="max-h-[88svh] overflow-y-auto sm:max-w-xl">
        {item && (
          <>
            <DialogHeader>
              <DialogTitle className="flex flex-wrap items-center gap-2">
                <PuzzleIcon className="size-5" /> {item.name}
                <span className="text-xs font-normal text-muted-foreground">v{item.version}</span>
                <TierBadge ext={{ tier: "official" }} />
              </DialogTitle>
              <DialogDescription>
                {item.publisher}
                {item.description ? ` · ${item.description}` : ""}
              </DialogDescription>
            </DialogHeader>

            {!verify ? (
              <div className="grid gap-4 text-sm">
                <section className="grid gap-1.5">
                  <h3 className="font-medium">{t("Fitur")}</h3>
                  {feats.length === 0 ? <p className="text-muted-foreground">{t("Hanya tampilan")}</p> : (
                    <ul className="list-disc pl-5 text-muted-foreground">
                      {feats.map((f) => (
                        <li key={f}>{f}</li>
                      ))}
                    </ul>
                  )}
                  {item.widgets.length > 0 && (
                    <ul className="grid gap-0.5 text-xs text-muted-foreground">
                      {item.widgets.map((w) => (
                        <li key={w.type}>
                          <span className="font-medium text-foreground">{w.title}</span>
                          {w.description ? ` · ${w.description}` : ""}
                        </li>
                      ))}
                    </ul>
                  )}
                </section>

                <section className="grid gap-1.5">
                  <h3 className="font-medium">{t("Izin yang diminta")}</h3>
                  {item.permissions.length === 0 ? (
                    <p className="text-muted-foreground">{t("Tidak meminta izin apa pun.")}</p>
                  ) : (
                    <ul className="divide-y rounded-lg border">
                      {item.permissions.map((p) => (
                        <li key={p.id} className="p-2.5">
                          <div className="flex flex-wrap items-center gap-2">
                            <code className="text-xs">{p.id}</code>
                            {p.required && <Badge variant="secondary">{t("Wajib")}</Badge>}
                            {p.sensitive && (
                              <Badge variant="outline" className="gap-1">
                                <ShieldAlertIcon className="size-3" /> {t("Sensitif")}
                              </Badge>
                            )}
                          </div>
                          {p.description && <p className="text-xs text-muted-foreground">{p.description}</p>}
                        </li>
                      ))}
                    </ul>
                  )}
                  <p className="text-xs text-muted-foreground">{t("Setelah dipasang, extension mati. Anda mengaktifkannya dan memberi izin satu per satu di halaman detail.")}</p>
                </section>

                {item.actions.length > 0 && (
                  <section className="grid gap-1.5">
                    <h3 className="font-medium">{t("Aksi yang bisa diberikan ke peran")}</h3>
                    <ul className="grid gap-0.5 text-xs text-muted-foreground">
                      {item.actions.map((a) => (
                        <li key={a.id}>
                          <span className="font-medium text-foreground">{a.label}</span>
                          {a.level === "update" ? ` (${t("butuh Ubah")})` : ""}
                          {a.description ? ` · ${a.description}` : ""}
                        </li>
                      ))}
                    </ul>
                  </section>
                )}

                <section className="grid gap-1.5">
                  <h3 className="font-medium">{t("Kompatibilitas")}</h3>
                  <p className="text-xs text-muted-foreground">
                    {item.telemon ? t("Versi Telemon: {range}", { range: item.telemon }) : t("Semua versi Telemon")} · {size(item.size)}
                  </p>
                  {item.requires.length > 0 && (
                    <p className="text-xs text-muted-foreground">
                      {t("Membutuhkan")}: {item.requires.map((r) => `${r.id} ${r.version}${r.optional ? ` (${t("Opsional")})` : ""}`).join(", ")}
                    </p>
                  )}
                  {item.alsoInstalls.length > 0 && <p className="text-xs font-medium">{t("Ikut dipasang: {list}", { list: item.alsoInstalls.join(", ") })}</p>}
                  {item.problem && (
                    <Alert variant="destructive">
                      <AlertDescription>{item.problem}</AlertDescription>
                    </Alert>
                  )}
                </section>
              </div>
            ) : (
              <div className="grid gap-3 text-sm">
                <Alert>
                  <ShieldAlertIcon />
                  <AlertTitle>{t("Verifikasi sebelum memasang")}</AlertTitle>
                  <AlertDescription>
                    <p>{t("Tanda tangan katalog dan paket diperiksa otomatis; ukuran dan SHA-256 dicocokkan. Yang perlu Anda periksa: apakah penerbit dan izinnya masuk akal.")}</p>
                  </AlertDescription>
                </Alert>
                <dl className="grid gap-1">
                  <div><dt className="inline text-muted-foreground">{t("Penerbit")}: </dt><dd className="inline">{item.publisher}</dd></div>
                  <div><dt className="inline text-muted-foreground">{t("Izin yang diminta")}: </dt><dd className="inline">{item.permissions.map((p) => p.id).join(", ") || t("Tidak ada")}</dd></div>
                  {item.alsoInstalls.length > 0 && <div><dt className="inline text-muted-foreground">{t("Ikut dipasang")}: </dt><dd className="inline">{item.alsoInstalls.join(", ")}</dd></div>}
                </dl>
                {sensitive.length > 0 && <p className="font-medium">{t("Termasuk izin sensitif: {list}.", { list: sensitive.map((p) => p.id).join(", ") })}</p>}
                <div className="flex items-start gap-2">
                  <Checkbox id="store-verify" checked={checked} onCheckedChange={(v) => setChecked(v === true)} />
                  <Label htmlFor="store-verify" className="text-sm font-normal">
                    {t("Saya sudah memeriksa penerbit dan izin yang diminta, dan ingin memasang extension ini.")}
                  </Label>
                </div>
              </div>
            )}

            <DialogFooter>
              {!verify ? (
                <>
                  <Button variant="outline" onClick={onClose}>
                    {t("Tutup")}
                  </Button>
                  <Button disabled={locked || !!item.problem || item.upToDate} title={item.upToDate ? t("Versi terbaru terpasang") : undefined} onClick={() => setVerify(true)}>
                    {item.installedVersion ? t("Perbarui") : t("Pasang")}
                  </Button>
                </>
              ) : (
                <>
                  <Button variant="outline" disabled={busy} onClick={() => setVerify(false)}>
                    {t("Kembali")}
                  </Button>
                  <Button disabled={!checked || busy} onClick={install}>
                    {busy && <Loader2Icon className="animate-spin" />} {t("Verifikasi dan pasang")}
                  </Button>
                </>
              )}
            </DialogFooter>
          </>
        )}
      </DialogContent>
    </Dialog>
  )
}

/**
 * Toko extension (`#/pengaturan/extension/store`): jelajah, cari, dan filter katalog official; klik untuk detail dan pasang.
 * Setelah dipasang, pindah ke detail extension dengan mengganti entri riwayat, jadi Back tidak kembali ke toko.
 */
export function ExtensionStorePage() {
  const t = useT()
  const { api } = useStore()
  const reason = useUpdateGate("admin")()
  const [store, setStore] = React.useState<ExtensionStore | null>(null)
  const [error, setError] = React.useState<{ code?: string; message: string } | null>(null)
  const [query, setQuery] = React.useState("")
  const [filter, setFilter] = React.useState<Filter>("all")
  const [open, setOpen] = React.useState<string | null>(null)
  const [tick, setTick] = React.useState(0)

  React.useEffect(() => {
    let cancelled = false
    api
      .getExtensionStore()
      .then((s) => !cancelled && (setStore(s), setError(null)))
      .catch((e) => !cancelled && setError({ code: (e as { code?: string }).code, message: errMsg(e) }))
    return () => {
      cancelled = true
    }
  }, [api, tick])

  const list = React.useMemo(() => {
    const q = query.trim().toLowerCase()
    return (store?.extensions ?? []).filter((e) => {
      if (q && !`${e.name} ${e.id} ${e.publisher} ${e.description}`.toLowerCase().includes(q)) return false
      if (filter === "new") return !e.installedVersion
      if (filter === "installed") return !!e.installedVersion
      if (filter === "update") return !!e.installedVersion && !e.upToDate
      return true
    })
  }, [store, query, filter])
  const item = store?.extensions.find((e) => e.id === open) ?? null

  const filters: { value: Filter; label: string }[] = [
    { value: "all", label: t("Semua") },
    { value: "new", label: t("Belum terpasang") },
    { value: "installed", label: t("Terpasang") },
    { value: "update", label: t("Ada pembaruan") },
  ]

  return (
    <div className="mx-auto grid w-full max-w-5xl gap-3">
      <Button variant="ghost" size="sm" className="w-fit" onClick={() => navigatePath("pengaturan", ["extension"])}>
        <ArrowLeftIcon /> {t("Extension")}
      </Button>
      <div className="flex flex-wrap items-end gap-2">
        <div className="mr-auto">
          <h2 className="text-lg font-semibold">{t("Toko extension")}</h2>
          <p className="text-sm text-muted-foreground">{t("Extension official dari Teliti. Katalog dan paketnya bertanda tangan dan diperiksa sebelum dipasang.")}</p>
        </div>
        <div className="relative min-w-0 flex-1 sm:w-72 sm:flex-none">
          <SearchIcon className="pointer-events-none absolute top-1/2 left-2.5 size-4 -translate-y-1/2 text-muted-foreground" />
          <Input type="search" value={query} onChange={(e) => setQuery(e.target.value)} placeholder={t("Cari di toko…")} aria-label={t("Cari di toko")} className="pl-8" />
        </div>
      </div>
      <div className="flex flex-wrap gap-1.5" role="group" aria-label={t("Filter")}>
        {filters.map((f) => (
          <Button key={f.value} size="sm" variant="outline" aria-pressed={filter === f.value} className={cn(filter === f.value && "border-brand text-brand")} onClick={() => setFilter(f.value)}>
            {f.label}
          </Button>
        ))}
      </div>
      {reason && (
        <Alert>
          <AlertDescription>{t("{reason}. Anda hanya bisa melihat extension.", { reason })}</AlertDescription>
        </Alert>
      )}

      {error?.code === "store_off" ? (
        <Alert className="mx-auto max-w-lg">
          <AlertTitle>{t("Toko mati di server ini")}</AlertTitle>
          <AlertDescription>{t("Pasang lewat berkas lokal, atau nyalakan toko dengan TELEMON_EXTENSIONS_STORE_URL.")}</AlertDescription>
        </Alert>
      ) : error ? (
        <Alert variant="destructive" className="mx-auto max-w-lg">
          <AlertTitle>{t("Toko tidak bisa dibuka")}</AlertTitle>
          <AlertDescription>
            <p>{error.message}</p>
            <Button variant="outline" size="sm" className="mt-2" onClick={() => setTick((x) => x + 1)}>
              {t("Coba lagi")}
            </Button>
          </AlertDescription>
        </Alert>
      ) : !store ? (
        <div className="grid grid-cols-1 gap-2 min-[560px]:grid-cols-2 lg:grid-cols-3">
          <Skeleton className="h-28" />
          <Skeleton className="h-28" />
          <Skeleton className="h-28" />
        </div>
      ) : list.length === 0 ? (
        <Card className="items-center py-12 text-sm text-muted-foreground">{store.extensions.length === 0 ? t("Belum ada extension di toko.") : t("Tidak ada extension yang cocok.")}</Card>
      ) : (
        <ul className="grid grid-cols-1 gap-2 min-[560px]:grid-cols-2 lg:grid-cols-3">
          {list.map((e) => (
            <li key={e.id} className="min-w-0">
              <button
                type="button"
                onClick={() => setOpen(e.id)}
                className="flex h-full w-full flex-col gap-1.5 rounded-xl border border-l-4 border-l-brand bg-card p-3 text-left transition-colors hover:bg-muted/40 focus-visible:ring-3 focus-visible:ring-ring/50 focus-visible:outline-none"
              >
                <div className="flex items-start gap-2">
                  <PuzzleIcon className="mt-0.5 size-4 shrink-0 text-muted-foreground" />
                  <span className="min-w-0 flex-1">
                    <span className="block truncate text-sm font-medium">{e.name}</span>
                    <span className="block truncate text-xs text-muted-foreground">
                      v{e.version} · {e.publisher}
                    </span>
                  </span>
                </div>
                {e.description && <p className="line-clamp-2 text-xs text-muted-foreground">{e.description}</p>}
                <div className="mt-auto flex flex-wrap items-center gap-1.5 border-t pt-1.5 text-xs">
                  {e.installedVersion ? <Badge variant="secondary">{e.upToDate ? t("Terpasang") : t("Terpasang v{v}", { v: e.installedVersion })}</Badge> : <Badge variant="outline">{t("Belum terpasang")}</Badge>}
                  {e.installedVersion && !e.upToDate && <Badge>{t("Ada pembaruan")}</Badge>}
                  {e.problem && <Badge variant="destructive">{t("Tidak cocok")}</Badge>}
                  <span className="ml-auto truncate text-muted-foreground">{features(e, t).slice(0, 2).join(" · ") || t("Hanya tampilan")}</span>
                </div>
              </button>
            </li>
          ))}
        </ul>
      )}

      <StoreDetailDialog
        item={item}
        locked={!!reason}
        onClose={() => setOpen(null)}
        onInstalled={(id) => {
          setOpen(null)
          // Ganti entri riwayat: Back dari detail menuju daftar extension, bukan kembali ke toko.
          replacePath("pengaturan", ["extension", id])
        }}
      />
    </div>
  )
}
