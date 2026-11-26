import * as React from "react"
import { ArrowLeftIcon, ExternalLinkIcon, PuzzleIcon, ShieldAlertIcon, Trash2Icon } from "lucide-react"
import { toast } from "sonner"

import { ConfirmDialog } from "@/components/confirm-dialog"
import { AccessPanel, BackendInfo, Permissions, Requires, StatusBadge, TierBadge, ToggleExtensionDialog, errMsg, useExtensionPatch } from "@/components/extensions/parts"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import { Checkbox } from "@/components/ui/checkbox"
import { Label } from "@/components/ui/label"
import { Skeleton } from "@/components/ui/skeleton"
import { Switch } from "@/components/ui/switch"
import { useUpdateGate } from "@/hooks/use-write-gate"
import { reloadExtensions } from "@/lib/extensions/loader"
import { formatDateTime } from "@/lib/format"
import { useT } from "@/lib/i18n"
import { navigatePath, pathHref } from "@/lib/route"
import { useStore } from "@/lib/store-context"
import type { ExtensionInfo, ExtensionList } from "@/lib/types"

function Row({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="grid grid-cols-[8.5rem_1fr] gap-2 border-b py-2 text-sm last:border-b-0 sm:grid-cols-[11rem_1fr]">
      <dt className="text-muted-foreground">{label}</dt>
      <dd className="min-w-0 break-words">{children}</dd>
    </div>
  )
}

/**
 * Detail satu extension (`#/pengaturan/extension/<id>`), di tengah halaman: informasi, kompatibilitas, izin (toggle),
 * akses per peran, backend, isi yang ditambahkan, dan hapus (ADR 0028).
 */
export function ExtensionDetailPage({ id }: { id: string }) {
  const t = useT()
  const { api } = useStore()
  const reason = useUpdateGate("admin")()
  const [list, setList] = React.useState<ExtensionList | null>(null)
  const [error, setError] = React.useState<string | null>(null)
  const [toggle, setToggle] = React.useState<{ ext: ExtensionInfo; enable: boolean } | null>(null)
  const [permAsk, setPermAsk] = React.useState<{ perm: string } | null>(null)
  const [removing, setRemoving] = React.useState(false)
  const [deleteData, setDeleteData] = React.useState(false)
  const [busyPerm, setBusyPerm] = React.useState(false)

  const upsert = React.useCallback((e: ExtensionInfo) => setList((l) => (l ? { ...l, extensions: [...l.extensions.filter((x) => x.id !== e.id), e] } : l)), [])
  const patch = useExtensionPatch(upsert)
  React.useEffect(() => {
    let cancelled = false
    api
      .getExtensions()
      .then((l) => !cancelled && (setList(l), setError(null)))
      .catch((e) => !cancelled && setError(errMsg(e)))
    return () => {
      cancelled = true
    }
  }, [api])

  const back = () => navigatePath("pengaturan", ["extension"])
  const frame = "mx-auto grid w-full max-w-3xl gap-4"

  if (error) {
    return (
      <Alert variant="destructive" className="mx-auto max-w-lg">
        <AlertTitle>{t("Gagal memuat extension")}</AlertTitle>
        <AlertDescription>{error}</AlertDescription>
      </Alert>
    )
  }
  if (!list) return <Skeleton className="mx-auto h-64 w-full max-w-3xl" />
  const ext = list.extensions.find((e) => e.id === id)
  if (!ext) {
    return (
      <div className={frame}>
        <Button variant="ghost" size="sm" className="w-fit" onClick={back}>
          <ArrowLeftIcon /> {t("Extension")}
        </Button>
        <Alert>
          <PuzzleIcon />
          <AlertTitle>{t("Extension tidak ditemukan")}</AlertTitle>
          <AlertDescription>{t("Mungkin sudah dihapus.")}</AlertDescription>
        </Alert>
      </div>
    )
  }

  const locked = !!reason
  const setPerm = async (perm: string, on: boolean) => {
    setBusyPerm(true)
    try {
      await patch(ext.id, { perms: { [perm]: on } })
    } catch (e) {
      toast.error(errMsg(e))
    } finally {
      setBusyPerm(false)
    }
  }
  const sensitiveOf = (perm: string) => ext.permissions.find((p) => p.id === perm)?.sensitive

  return (
    <div className={frame}>
      <Button variant="ghost" size="sm" className="w-fit" onClick={back}>
        <ArrowLeftIcon /> {t("Extension")}
      </Button>

      <Card>
        <CardHeader>
          <CardTitle className="flex flex-wrap items-center gap-2 text-base">
            <PuzzleIcon className="size-5" /> {ext.name}
            <span className="text-xs font-normal text-muted-foreground">v{ext.version}</span>
            <TierBadge ext={ext} />
            <StatusBadge ext={ext} />
          </CardTitle>
          <CardDescription>
            {ext.publisher}
            {ext.description ? ` · ${ext.description}` : ""}
          </CardDescription>
        </CardHeader>
        <CardContent className="grid gap-3">
          <div className="flex items-center justify-between gap-3 rounded-lg border p-3">
            <div>
              <div className="text-sm font-medium">{t("Aktifkan extension")}</div>
              <p className="text-xs text-muted-foreground">{t("Yang mati tidak dimuat dan tidak disajikan.")}</p>
            </div>
            <Switch checked={ext.enabled} disabled={locked} aria-label={t("Aktifkan extension")} onCheckedChange={(v) => setToggle({ ext, enable: v })} />
          </div>
          {ext.status === "incompatible" && (
            <Alert variant="destructive">
              <ShieldAlertIcon />
              <AlertDescription>{t("Versi Telemon ini di luar rentang yang didukung extension ({range}), jadi tidak dimuat. Perbarui extension.", { range: ext.telemon ?? "" })}</AlertDescription>
            </Alert>
          )}
          {ext.enabled && ext.status === "ok" && ext.sessions.some((s) => s.kind === "router-ssh") && !ext.permissions.some((p) => p.id === "feature.router-ssh" && p.granted) && (
            <Alert>
              <ShieldAlertIcon />
              <AlertTitle>{t("Belum bisa dipakai")}</AlertTitle>
              <AlertDescription>{t("Terminal SSH butuh izin feature.router-ssh. Nyalakan di bagian Izin di bawah, lalu berikan akses ke peran di Akses peran.")}</AlertDescription>
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
          {ext.enabled && ext.status === "ok" && (ext.pages ?? []).length > 0 && (
            <div className="flex flex-wrap gap-2">
              {ext.pages.map((pg) => (
                <Button key={pg.id} variant="outline" size="sm" asChild>
                  <a href={pathHref("ext", [ext.id, pg.id])}>
                    <ExternalLinkIcon /> {t("Buka {title}", { title: pg.title })}
                  </a>
                </Button>
              ))}
            </div>
          )}
          {reason && (
            <Alert>
              <AlertDescription>{t("{reason}. Anda hanya bisa melihat extension.", { reason })}</AlertDescription>
            </Alert>
          )}
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="text-base">{t("Informasi")}</CardTitle>
        </CardHeader>
        <CardContent>
          <dl>
            <Row label="ID">
              <code className="text-xs">{ext.id}</code>
            </Row>
            <Row label={t("Penerbit")}>{ext.publisher}</Row>
            <Row label={t("Versi")}>v{ext.version}</Row>
            <Row label={t("Tingkat")}>{ext.tier === "official" ? t("Official (buatan Teliti)") : t("Publik (pihak ketiga)")}</Row>
            <Row label={t("Tanda tangan")}>{ext.signed ? t("Valid") : t("Tidak bertanda tangan")}</Row>
            <Row label={t("Dipasang")}>{formatDateTime(ext.installedAt)}</Row>
            <Row label={t("Diperbarui")}>{formatDateTime(ext.updatedAt)}</Row>
            <Row label={t("Data sendiri")}>{ext.hasData ? t("Ya (tabel di database)") : t("Tidak")}</Row>
            <Row label={t("Menambah")}>
              {[ext.widgets.length > 0 && t("{n} widget", { n: ext.widgets.length }), (ext.pages ?? []).length > 0 && t("{n} halaman", { n: ext.pages.length }), ext.hasBackend && t("Backend")].filter(Boolean).join(" · ") || t("Hanya tampilan")}
            </Row>
          </dl>
          {ext.widgets.length > 0 && (
            <ul className="mt-2 grid gap-1 text-xs text-muted-foreground">
              {ext.widgets.map((w) => (
                <li key={w.type}>
                  <span className="font-medium text-foreground">{w.title}</span>
                  {w.description ? ` · ${w.description}` : ""}
                </li>
              ))}
            </ul>
          )}
        </CardContent>
      </Card>

      {(ext.requires.length > 0 || ext.telemon || ext.dependents.length > 0) && (
        <Card>
          <CardHeader>
            <CardTitle className="text-base">{t("Kompatibilitas")}</CardTitle>
          </CardHeader>
          <CardContent className="grid gap-2">
            {ext.telemon && <p className="text-sm text-muted-foreground">{t("Versi Telemon: {range}", { range: ext.telemon })}</p>}
            <Requires list={ext.requires} />
            {ext.dependents.length > 0 && <p className="text-sm text-muted-foreground">{t("Dibutuhkan oleh: {list}", { list: ext.dependents.join(", ") })}</p>}
          </CardContent>
        </Card>
      )}

      <Card>
        <CardHeader>
          <CardTitle className="text-base">{t("Izin")}</CardTitle>
          <CardDescription>{t("Apa yang boleh dijangkau extension ini. Izin sensitif mulai mati dan butuh konfirmasi untuk dinyalakan.")}</CardDescription>
        </CardHeader>
        <CardContent>
          <Permissions
            perms={ext.permissions}
            granted={(p) => ext.permissions.find((x) => x.id === p)?.granted ?? false}
            locked={locked || busyPerm}
            onToggle={(p, on) => (on && sensitiveOf(p) ? setPermAsk({ perm: p }) : void setPerm(p, on))}
          />
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="text-base">{t("Akses peran")}</CardTitle>
          <CardDescription>{t("Atur peran mana yang boleh membuka extension ini dan aksi apa yang boleh dilakukan. Anda tidak bisa memberi lebih daripada akses Anda sendiri atau mengubah peran Anda sendiri.")}</CardDescription>
        </CardHeader>
        <CardContent>
          <AccessPanel ext={ext} locked={locked} />
        </CardContent>
      </Card>

      {ext.hasBackend && (
        <Card>
          <CardContent>
            <BackendInfo ext={ext} locked={locked} onChanged={upsert} />
          </CardContent>
        </Card>
      )}

      <Card>
        <CardHeader>
          <CardTitle className="text-base">{t("Hapus extension")}</CardTitle>
          <CardDescription>{ext.dependents.length > 0 ? t("Masih dibutuhkan extension lain") : t("Menghapus paket, izin, dan aksesnya. Data bisa disimpan atau ikut dihapus.")}</CardDescription>
        </CardHeader>
        <CardContent>
          <Button variant="outline" disabled={locked || ext.dependents.length > 0} onClick={() => setRemoving(true)}>
            <Trash2Icon /> {t("Hapus")}
          </Button>
        </CardContent>
      </Card>

      <ToggleExtensionDialog
        target={toggle}
        onClose={() => setToggle(null)}
        apply={async (eid, enabled) => {
          await patch(eid, { enabled })
        }}
      />
      <ConfirmDialog
        open={!!permAsk}
        onOpenChange={(o) => !o && setPermAsk(null)}
        title={t("Nyalakan izin sensitif?")}
        description={
          <div className="grid gap-2">
            <p>
              <code>{permAsk?.perm}</code>
            </p>
            <p>{ext.permissions.find((p) => p.id === permAsk?.perm)?.description || t("Izin ini memberi extension jangkauan yang lebih luas.")}</p>
          </div>
        }
        confirmLabel={t("Nyalakan")}
        onConfirm={async () => {
          if (permAsk) await patch(ext.id, { perms: { [permAsk.perm]: true } })
        }}
      />
      <ConfirmDialog
        open={removing}
        onOpenChange={setRemoving}
        title={t("Hapus extension?")}
        description={
          <div className="grid gap-3">
            <p>{t("{name} dan pengaturannya dihapus. Widget-nya hilang dari Ringkasan.", { name: ext.name })}</p>
            {ext.hasData && (
              <div className="flex items-start gap-2">
                <Checkbox id="ext-del-data" checked={deleteData} onCheckedChange={(v) => setDeleteData(v === true)} />
                <Label htmlFor="ext-del-data" className="text-sm font-normal">
                  {t("Hapus juga data extension ini (tabel di database). Tanpa centang, data disimpan dan dipakai lagi bila dipasang ulang.")}
                </Label>
              </div>
            )}
          </div>
        }
        confirmLabel={t("Hapus")}
        destructive
        onConfirm={async () => {
          await api.removeExtension(ext.id, { deleteData: ext.hasData && deleteData })
          await reloadExtensions(() => api.getActiveExtensions())
          navigatePath("pengaturan", ["extension"])
        }}
      />
    </div>
  )
}
