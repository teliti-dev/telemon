import * as React from "react"
import { Loader2Icon, ShieldAlertIcon } from "lucide-react"
import { toast } from "sonner"

import { ConfirmDialog } from "@/components/confirm-dialog"
import { Alert, AlertDescription } from "@/components/ui/alert"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Checkbox } from "@/components/ui/checkbox"
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog"
import { Label } from "@/components/ui/label"
import { Skeleton } from "@/components/ui/skeleton"
import { Switch } from "@/components/ui/switch"
import { reloadExtensions } from "@/lib/extensions/loader"
import { tk, useT } from "@/lib/i18n"
import { useStore } from "@/lib/store-context"
import type { ExtensionAccess, ExtensionInfo, ExtensionLevel, ExtensionLicense, ExtensionRequirement } from "@/lib/types"

export const errMsg = (e: unknown) => (e instanceof Error ? e.message : String(e))

export const STATUS = { disabled: tk("Mati"), limited: tk("Terbatas"), incompatible: tk("Tidak cocok"), ok: tk("Aktif"), broken: tk("Rusak") } as const

export function StatusBadge({ ext }: { ext: ExtensionInfo }) {
  const t = useT()
  // i18n-ignore: kunci ada di STATUS (tk)
  return <Badge variant={ext.status === "ok" ? "secondary" : "outline"}>{t(STATUS[ext.status])}</Badge>
}

export function TierBadge({ ext }: { ext: Pick<ExtensionInfo, "tier"> }) {
  const t = useT()
  return <Badge variant={ext.tier === "official" ? "default" : "outline"}>{ext.tier === "official" ? t("Official") : t("Publik")}</Badge>
}

export const LICENSE_KIND = {
  "open-source": tk("Open source"),
  free: tk("Gratis"),
  freemium: tk("Freemium"),
  paid: tk("Berbayar"),
  trial: tk("Uji coba"),
  proprietary: tk("Syarat khusus"),
} as const

/**
 * Lisensi extension apa adanya dari penerbit: jenis (+ pengenal SPDX), ringkasan, dan tautan syarat lengkap.
 * Tanpa lisensi tertulis = "Tidak dinyatakan". Telemon tidak menilai atau menegakkan isinya (ADR 0030).
 */
export function LicenseInfo({ license }: { license?: ExtensionLicense | null }) {
  const t = useT()
  if (!license) return <span className="text-muted-foreground">{t("Tidak dinyatakan penerbit")}</span>
  return (
    <span className="grid gap-1">
      <span className="flex flex-wrap items-center gap-1.5">
        {/* i18n-ignore: kunci ada di LICENSE_KIND (tk) */}
        <Badge variant="outline">{t(LICENSE_KIND[license.kind])}</Badge>
        {license.spdx && <code className="text-xs">{license.spdx}</code>}
        {license.url && (
          <a href={license.url} target="_blank" rel="noopener noreferrer" className="text-xs text-brand underline underline-offset-2">
            {t("Syarat lengkap")}
          </a>
        )}
      </span>
      {license.summary && <span className="text-xs text-muted-foreground">{license.summary}</span>}
    </span>
  )
}

/** Pengingat singkat di tempat pemasangan dan detail: lisensi extension urusan penerbit, bukan Telemon. */
export function LicenseNote() {
  const t = useT()
  return <p className="text-xs text-muted-foreground">{t("Lisensi extension ditentukan penerbitnya. Telemon tidak menjual, memverifikasi, atau menegakkan lisensi itu, dan tidak bertanggung jawab atas extension pihak ketiga.")}</p>
}

/** Ubah satu extension (aktif, izin) lalu muat ulang widget dan halaman di aplikasi. */
export function useExtensionPatch(onChanged: (e: ExtensionInfo) => void) {
  const { api } = useStore()
  return React.useCallback(
    async (id: string, p: { enabled?: boolean; perms?: Record<string, boolean> }) => {
      const next = await api.updateExtension(id, p)
      onChanged(next)
      await reloadExtensions(() => api.getActiveExtensions())
      return next
    },
    [api, onChanged]
  )
}

/**
 * Popup konfirmasi untuk menyalakan atau mematikan extension. Dipakai di daftar dan halaman detail.
 * `target` = extension dan keadaan yang diminta; `null` = tertutup.
 */
export function ToggleExtensionDialog({ target, onClose, apply }: { target: { ext: ExtensionInfo; enable: boolean } | null; onClose: () => void; apply: (id: string, enabled: boolean) => Promise<void> }) {
  const t = useT()
  const ext = target?.ext
  const enable = target?.enable ?? true
  const granted = ext?.permissions.filter((p) => p.granted) ?? []
  const sensitive = granted.filter((p) => p.sensitive)
  return (
    <ConfirmDialog
      open={!!target}
      onOpenChange={(o) => !o && onClose()}
      title={enable ? t("Aktifkan {name}?", { name: ext?.name ?? "" }) : t("Nonaktifkan {name}?", { name: ext?.name ?? "" })}
      description={
        ext ? (
          <div className="grid gap-2">
            {enable ? (
              <>
                <p>{t("Extension dimuat dan fiturnya muncul untuk peran yang punya akses.")}</p>
                {granted.length > 0 ? (
                  <p>
                    {t("Izin yang sedang menyala")}: {granted.map((p) => p.id).join(", ")}.
                  </p>
                ) : (
                  <p>{t("Belum ada izin yang menyala.")}</p>
                )}
                {sensitive.length > 0 && <p className="font-medium">{t("Termasuk izin sensitif: {list}.", { list: sensitive.map((p) => p.id).join(", ") })}</p>}
              </>
            ) : (
              <>
                <p>{t("Extension tidak dimuat lagi: widget dan halamannya hilang, rute dan worker berhenti. Pengaturan dan datanya tetap tersimpan.")}</p>
                {ext.dependents.length > 0 && <p className="font-medium">{t("Extension ini dibutuhkan oleh {list}; extension itu menjadi terbatas.", { list: ext.dependents.join(", ") })}</p>}
              </>
            )}
          </div>
        ) : (
          ""
        )
      }
      confirmLabel={enable ? t("Aktifkan") : t("Nonaktifkan")}
      destructive={!enable}
      onConfirm={async () => {
        if (ext) await apply(ext.id, enable)
      }}
    />
  )
}

export function Permissions({
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

export function Requires({ list }: { list: ExtensionRequirement[] }) {
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
export function AccessPanel({ ext, locked }: { ext: ExtensionInfo; locked: boolean }) {
  const t = useT()
  const { api } = useStore()
  const [data, setData] = React.useState<ExtensionAccess | null>(null)
  const [error, setError] = React.useState<string | null>(null)
  const [busy, setBusy] = React.useState<string | null>(null)
  const id = ext.id

  React.useEffect(() => {
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

  if (error) {
    return (
      <Alert variant="destructive">
        <AlertDescription>{error}</AlertDescription>
      </Alert>
    )
  }
  if (!data) return <Skeleton className="h-24" />
  return (
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
  )
}

/** Backend WASM: rute, worker (status dan jalankan sekarang), dan log. */
export function BackendInfo({ ext, locked, onChanged }: { ext: ExtensionInfo; locked: boolean; onChanged: (e: ExtensionInfo) => void }) {
  const t = useT()
  const { api } = useStore()
  const [busy, setBusy] = React.useState<string | null>(null)
  const [log, setLog] = React.useState<string[] | null>(null)

  async function run(worker: string) {
    setBusy(worker)
    try {
      const r = await api.runExtensionWorker(ext.id, worker)
      onChanged({ ...ext, workers: r.workers })
      if (r.ok) toast.success(t("Worker dijalankan"))
      else toast.error(r.error ?? t("Worker gagal"))
    } catch (e) {
      toast.error(errMsg(e))
    } finally {
      setBusy(null)
    }
  }

  return (
    <div className="grid gap-2">
      <div className="flex items-center justify-between gap-2">
        <div className="text-sm font-medium">{t("Backend")}</div>
        <Button variant="outline" size="sm" onClick={() => api.getExtensionLogs(ext.id).then(setLog).catch((e) => toast.error(errMsg(e)))}>
          {t("Log")}
        </Button>
      </div>
      {ext.routes.length > 0 && (
        <ul className="text-xs text-muted-foreground">
          {ext.routes.map((r) => (
            <li key={r.id}>
              <code>
                {r.method} /api/ext/{ext.id}
                {r.path}
              </code>
              {r.action ? ` · ${r.action}` : ""}
            </li>
          ))}
        </ul>
      )}
      {ext.workers.map((w) => (
        <div key={w.id} className="flex flex-wrap items-center gap-2 rounded-lg border p-2 text-xs">
          <code>{w.id}</code>
          <span className="text-muted-foreground">{t("tiap {n} dtk", { n: w.everySeconds })}</span>
          {w.state.paused && <Badge variant="destructive">{t("Dijeda")}</Badge>}
          {w.state.lastOk === true && <Badge variant="secondary">{t("Terakhir berhasil")}</Badge>}
          {w.state.lastOk === false && <Badge variant="destructive">{t("Terakhir gagal")}</Badge>}
          {w.state.lastError && <span className="text-destructive">{w.state.lastError}</span>}
          <Button className="ml-auto" variant="outline" size="sm" disabled={locked || busy === w.id} onClick={() => run(w.id)}>
            {busy === w.id && <Loader2Icon className="animate-spin" />} {t("Jalankan sekarang")}
          </Button>
        </div>
      ))}
      <Dialog open={!!log} onOpenChange={(o) => !o && setLog(null)}>
        <DialogContent className="max-h-[80svh] overflow-y-auto sm:max-w-2xl">
          <DialogHeader>
            <DialogTitle>{t("Log {name}", { name: ext.name })}</DialogTitle>
          </DialogHeader>
          <pre className="max-h-[55svh] overflow-auto rounded-md bg-muted p-3 text-xs whitespace-pre-wrap">{log && log.length > 0 ? log.join("\n") : t("Belum ada log.")}</pre>
        </DialogContent>
      </Dialog>
    </div>
  )
}
