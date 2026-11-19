import * as React from "react"
import { DatabaseIcon, RadioTowerIcon, RotateCcwIcon, ShieldIcon, SparklesIcon, TimerResetIcon } from "lucide-react"
import { toast } from "sonner"

import { AiDashboard } from "@/components/ai-dashboard"
import { UpdateCard } from "@/components/update-card"
import { ConfirmDialog } from "@/components/confirm-dialog"
import { Alert, AlertDescription } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import { Card, CardAction, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
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
import { tk, useT } from "@/lib/i18n"
import { useUpdateGate } from "@/hooks/use-write-gate"
import { useApi } from "@/lib/api-context"
import { DEFAULT_SETTINGS } from "@/lib/defaults"
import { formatBytes, formatNumber } from "@/lib/format"
import { useStore } from "@/lib/store-context"
import type { AppSettings, StorageInfo } from "@/lib/types"

const errMsg = (e: unknown) => (e instanceof Error ? e.message : String(e))

// ---- Isian ----

function NumberField({
  id, label, hint, value, onChange, min, max, unit,
}: {
  id: string
  label: string
  hint?: string
  value: number
  onChange: (v: number) => void
  min: number
  max: number
  unit?: string
}) {
  const t = useT()
  const invalid = !Number.isInteger(value) || value < min || value > max
  return (
    <div className="grid gap-1.5">
      <Label htmlFor={id}>{label}</Label>
      <div className="flex items-center gap-2">
        <Input
          id={id}
          type="number"
          inputMode="numeric"
          min={min}
          max={max}
          value={Number.isNaN(value) ? "" : value}
          onChange={(e) => onChange(e.target.value === "" ? NaN : Number(e.target.value))}
          aria-invalid={invalid}
          className="w-32"
        />
        {unit && <span className="text-sm text-muted-foreground">{t(unit)}</span>}{/* i18n-ignore: unit dari literal yang sudah ada di kamus */}
      </div>
      {invalid ? (
        <p className="text-xs text-destructive">{t("Isi angka {min}-{max}.", { min: formatNumber(min), max: formatNumber(max) })}</p>
      ) : (
        hint && <p className="text-xs text-muted-foreground">{hint}</p>
      )}
    </div>
  )
}

interface Opt {
  value: number | null
  /** Kunci terjemahan; bila `n` ada, dipakai sebagai templat "{n}". */
  label: string
  n?: number
}

function ChoiceField({
  id, label, hint, value, options, onChange,
}: {
  id: string
  label: string
  hint?: string
  value: number | null
  options: Opt[]
  onChange: (v: number | null) => void
}) {
  const t = useT()
  const opts = options.some((o) => o.value === value) ? options : [...options, { value, label: tk("{n} (kustom)"), n: value ?? 0 }]
  return (
    <div className="grid gap-1.5">
      <Label htmlFor={id}>{label}</Label>
      <Select value={String(value)} onValueChange={(v) => onChange(v === "null" ? null : Number(v))}>
        <SelectTrigger id={id} className="w-full sm:w-56">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          {opts.map((o) => (
            <SelectItem key={String(o.value)} value={String(o.value)}>
              {t(o.label, { n: o.n ?? 0 })}{/* i18n-ignore: label dari tk() */}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
      {hint && <p className="text-xs text-muted-foreground">{hint}</p>}
    </div>
  )
}

const days = (...n: number[]): Opt[] => n.map((d) => ({ value: d, label: tk("{n} hari"), n: d }))
const OPT_EVENT: Opt[] = [...days(7, 14, 30, 60, 90, 180, 365), { value: null, label: tk("Selamanya") }]
const OPT_DELETED: Opt[] = [...days(30, 60, 90, 180, 365), { value: null, label: tk("Tidak pernah") }]
const OPT_AICHAT: Opt[] = [...days(7, 30, 90, 180, 365), { value: null, label: tk("Selamanya") }]
const OPT_AUDIT: Opt[] = [...days(90, 180, 365, 730), { value: null, label: tk("Selamanya") }]
const OPT_IMPORT: Opt[] = [
  { value: null, label: tk("Mati (hanya manual)") },
  { value: 1, label: tk("Tiap {n} jam"), n: 1 },
  { value: 6, label: tk("Tiap {n} jam"), n: 6 },
  { value: 12, label: tk("Tiap {n} jam"), n: 12 },
  { value: 24, label: tk("Tiap {n} jam"), n: 24 },
  { value: 72, label: tk("Tiap {n} hari"), n: 3 },
  { value: 168, label: tk("Tiap {n} hari"), n: 7 },
]
const OPT_SESSION: Opt[] = [1, 4, 12, 24, 72, 168].map((h) => ({ value: h, label: tk("{n} jam"), n: h }))

function problems(s: AppSettings) {
  const bad = (v: number | null, min: number, max: number, nullable = false) =>
    v === null ? !nullable : !Number.isInteger(v) || v < min || v > max
  const { retention: r, polling: p, remote: m } = s
  return [
    bad(r.eventDays, 1, 3650, true), bad(r.maxEvents, 1000, 5_000_000), bad(r.deletedDays, 1, 3650, true),
    bad(r.auditDays, 30, 3650, true), bad(p.sessionSeconds, 10, 600), bad(p.logSeconds, 10, 600),
    bad(p.autoImportHours, 1, 168, true), bad(p.timeoutSeconds, 3, 60), bad(p.retries, 0, 5),
    bad(m.sessionHours, 1, 720), bad(m.maxFailures, 3, 20), bad(m.lockMinutes, 1, 1440),
  ].filter(Boolean).length
}

/** Pengaturan: retensi data, polling dan sinkronisasi, akses jarak jauh, penyimpanan. Mengubah butuh izin Ubah. */
export function SettingsPage() {
  return <SettingsForm />
}

function SettingsForm() {
  const t = useT()
  const { api } = useApi()
  const { access } = useStore()
  const readReason = useUpdateGate("pengaturan")()
  const readOnly = !!readReason
  const [saved, setSaved] = React.useState<AppSettings | null>(null)
  const [draft, setDraft] = React.useState<AppSettings | null>(null)
  const [busy, setBusy] = React.useState(false)
  const [storageKey, setStorageKey] = React.useState(0)

  React.useEffect(() => {
    api
      .getSettings()
      .then((s) => {
        setSaved(s)
        setDraft(structuredClone(s))
      })
      .catch((e: unknown) => toast.error(errMsg(e)))
  }, [api])

  if (!saved || !draft) return <p className="text-sm text-muted-foreground">{t("Memuat pengaturan…")}</p>

  const dirty = JSON.stringify(saved) !== JSON.stringify(draft)
  const nProblems = problems(draft)

  function set<K extends keyof AppSettings>(group: K, patch: Partial<AppSettings[K]>) {
    setDraft((d) => (d ? { ...d, [group]: { ...d[group], ...patch } } : d))
  }
  const resetGroup = (group: keyof AppSettings) =>
    setDraft((d) => (d ? { ...d, [group]: structuredClone(DEFAULT_SETTINGS[group]) } : d))

  async function save() {
    if (!draft) return
    setBusy(true)
    try {
      const next = await api.saveSettings(draft)
      setSaved(next)
      setDraft(structuredClone(next))
      setStorageKey((k) => k + 1)
      toast.success(t("Pengaturan disimpan"))
    } catch (e) {
      toast.error(errMsg(e))
    } finally {
      setBusy(false)
    }
  }

  const { retention: r, polling: p, remote: m } = draft
  const resetButton = (group: keyof AppSettings) => (
    <Button variant="ghost" size="sm" onClick={() => resetGroup(group)} title={t("Kembalikan bagian ini ke nilai bawaan")}>
      <RotateCcwIcon /> {t("Bawaan")}
    </Button>
  )

  return (
    <fieldset disabled={readOnly} className="contents">
    <div className="grid gap-3">
      {readOnly && (
        <Alert>
          <AlertDescription>{t("{reason}. Anda hanya bisa melihat pengaturan.", { reason: readReason })}</AlertDescription>
        </Alert>
      )}
      <div className="flex flex-wrap items-center justify-between gap-2">
        <p className="text-sm text-muted-foreground">{t("Perubahan berlaku setelah disimpan.")}</p>
        <Button
          variant="ghost"
          size="sm"
          onClick={() => setDraft(structuredClone(DEFAULT_SETTINGS))}
          disabled={JSON.stringify(draft) === JSON.stringify(DEFAULT_SETTINGS)}
        >
          <RotateCcwIcon /> {t("Kembalikan semua ke bawaan")}
        </Button>
      </div>

      <div className="grid items-start gap-3 xl:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2"><TimerResetIcon className="size-4" /> {t("Retensi data")}</CardTitle>
            <CardDescription>{t("Berapa lama riwayat disimpan di Telemon. Yang lebih lama dibersihkan otomatis.")}</CardDescription>
            <CardAction>{resetButton("retention")}</CardAction>
          </CardHeader>
          <CardContent className="grid gap-4">
            <ChoiceField id="ret-event" label={t("Simpan event PPPoE")} value={r.eventDays} options={OPT_EVENT} onChange={(v) => set("retention", { eventDays: v })} />
            <NumberField id="ret-max" label={t("Maksimum event")} value={r.maxEvents} min={1000} max={5_000_000} unit="event" hint={t("Bila lebih banyak, yang terlama dihapus lebih dulu.")} onChange={(v) => set("retention", { maxEvents: v })} />
            <ChoiceField
              id="ret-del"
              label={t("Hapus permanen item yang sudah dihapus (soft delete)")}
              value={r.deletedDays}
              options={OPT_DELETED}
              hint={t("Hanya menghapus catatan di Telemon. Secret yang dinonaktifkan di router tidak disentuh.")}
              onChange={(v) => set("retention", { deletedDays: v })}
            />
            <ChoiceField id="ret-aichat" label={t("Simpan riwayat chat AI")} value={r.aiChatDays} options={OPT_AICHAT} onChange={(v) => set("retention", { aiChatDays: v })} />
            <ChoiceField id="ret-audit" label={t("Simpan riwayat perubahan dan login")} value={r.auditDays} options={OPT_AUDIT} onChange={(v) => set("retention", { auditDays: v })} />
            <p className="text-xs text-muted-foreground">{t("Catatan pelanggan dan data user/profil PPP dari router tidak dihapus otomatis.")}</p>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2"><RadioTowerIcon className="size-4" /> {t("Polling dan sinkronisasi")}</CardTitle>
            <CardDescription>{t("Seberapa sering Telemon membaca router. Interval pendek lebih segar tetapi membebani router.")}</CardDescription>
            <CardAction>{resetButton("polling")}</CardAction>
          </CardHeader>
          <CardContent className="grid gap-4">
            <NumberField id="pol-sess" label={t("Baca sesi aktif")} value={p.sessionSeconds} min={10} max={600} unit="detik" onChange={(v) => set("polling", { sessionSeconds: v })} />
            <NumberField id="pol-log" label={t("Baca log router")} value={p.logSeconds} min={10} max={600} unit="detik" onChange={(v) => set("polling", { logSeconds: v })} />
            <ChoiceField
              id="pol-import"
              label={t("Import otomatis dari Mikrotik")}
              value={p.autoImportHours}
              options={OPT_IMPORT}
              hint={t("Router tetap menjadi source of truth; selisih dimenangkan router.")}
              onChange={(v) => set("polling", { autoImportHours: v })}
            />
            <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
              <NumberField id="pol-to" label={t("Timeout koneksi")} value={p.timeoutSeconds} min={3} max={60} unit="detik" onChange={(v) => set("polling", { timeoutSeconds: v })} />
              <NumberField id="pol-retry" label={t("Percobaan ulang")} value={p.retries} min={0} max={5} unit="kali" onChange={(v) => set("polling", { retries: v })} />
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2"><ShieldIcon className="size-4" /> {t("Akses jarak jauh")}</CardTitle>
            <CardDescription>{t("Berlaku untuk akun yang masuk lewat browser (Serve LAN dan Publik).")}</CardDescription>
            <CardAction>{resetButton("remote")}</CardAction>
          </CardHeader>
          <CardContent className="grid gap-4">
            <ChoiceField id="rem-sess" label={t("Masa sesi login")} value={m.sessionHours} options={OPT_SESSION} onChange={(v) => set("remote", { sessionHours: v ?? 12 })} />
            <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
              <NumberField id="rem-fail" label={t("Batas gagal login")} value={m.maxFailures} min={3} max={20} unit="kali" onChange={(v) => set("remote", { maxFailures: v })} />
              <NumberField id="rem-lock" label={t("Lama penguncian")} value={m.lockMinutes} min={1} max={1440} unit="menit" onChange={(v) => set("remote", { lockMinutes: v })} />
            </div>
            <div className="flex items-center justify-between gap-4 rounded-lg border p-3">
              <div className="grid gap-0.5">
                <Label htmlFor="rem-write">{t("Izinkan tulis lewat akses publik")}</Label>
                <p className="text-xs text-muted-foreground">
                  {t("Bawaan mati. Peran dengan izin Ubah baru bisa menulis lewat internet bila ini dinyalakan. Hanya bisa diubah dari aplikasi desktop.")}
                </p>
              </div>
              <Switch id="rem-write" checked={m.allowPublicWrite} disabled={access !== "desktop"} onCheckedChange={(v) => set("remote", { allowPublicWrite: v })} />
            </div>
            {m.allowPublicWrite && (
              <Alert variant="destructive">
                <AlertDescription>
                  {t("Perubahan user dan profil PPP bisa dikirim ke router dari internet. Pastikan hanya peran tepercaya yang punya izin Ubah, dan semua akun memakai 2FA.")}
                </AlertDescription>
              </Alert>
            )}
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2"><SparklesIcon className="size-4" /> {t("Asisten AI")}</CardTitle>
            <CardDescription>{t("Chat untuk menanyakan data Telemon. Hanya membaca, dan hanya dari data yang boleh dilihat peran penanya.")}</CardDescription>
            <CardAction>{resetButton("ai")}</CardAction>
          </CardHeader>
          <CardContent className="grid gap-4">
            <div className="flex items-center justify-between gap-4 rounded-lg border p-3">
              <div className="grid gap-0.5">
                <Label htmlFor="ai-on">{t("Aktifkan AI chat")}</Label>
                <p className="text-xs text-muted-foreground">{t("Bila mati, tombol AI disembunyikan untuk semua orang.")}</p>
              </div>
              <Switch id="ai-on" checked={draft.ai.enabled} onCheckedChange={(v) => set("ai", { enabled: v })} />
            </div>
            <div className="flex items-center justify-between gap-4 rounded-lg border p-3">
              <div className="grid gap-0.5">
                <Label htmlFor="ai-mask">{t("Samarkan data sensitif")}</Label>
                <p className="text-xs text-muted-foreground">{t("IP dan MAC pelanggan disamarkan di jawaban AI.")}</p>
              </div>
              <Switch id="ai-mask" checked={draft.ai.maskSensitive} onCheckedChange={(v) => set("ai", { maskSensitive: v })} />
            </div>
          </CardContent>
        </Card>

        <div className="xl:col-span-2"><AiDashboard /></div>

        <UpdateCard />

        <StorageCard key={storageKey} dirty={dirty} />
      </div>

      {dirty && (
        <div className="sticky bottom-[calc(4.5rem+env(safe-area-inset-bottom))] z-10 flex flex-wrap items-center justify-between gap-2 rounded-xl border bg-background/95 p-3 shadow-lg backdrop-blur md:bottom-4 md:mr-14">
          <p className="text-sm">
            {nProblems > 0 ? <span className="text-destructive">{t("{n} isian belum valid.", { n: nProblems })}</span> : t("Ada perubahan yang belum disimpan.")}
          </p>
          <div className="flex gap-2">
            <Button variant="outline" onClick={() => setDraft(structuredClone(saved))} disabled={busy}>{t("Batalkan")}</Button>
            <Button onClick={save} disabled={busy || nProblems > 0}>{busy ? t("Menyimpan…") : t("Simpan")}</Button>
          </div>
        </div>
      )}
    </div>
    </fieldset>
  )
}

function StorageCard({ dirty }: { dirty: boolean }) {
  const t = useT()
  const { api } = useApi()
  const [info, setInfo] = React.useState<StorageInfo | null>(null)
  const [confirm, setConfirm] = React.useState(false)

  const load = React.useCallback(() => {
    api.getStorageInfo().then(setInfo).catch((e: unknown) => toast.error(errMsg(e)))
  }, [api])
  React.useEffect(() => {
    load()
  }, [load])

  const p = info?.purgeable
  const nothing = !p || p.events + p.users + p.profiles + p.chats === 0

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2"><DatabaseIcon className="size-4" /> {t("Penyimpanan")}</CardTitle>
        <CardDescription>{t("Data Telemon disimpan lokal di komputer ini.")}</CardDescription>
      </CardHeader>
      <CardContent className="grid gap-3 text-sm">
        {!info ? (
          <p className="text-muted-foreground">{t("Memuat…")}</p>
        ) : (
          <>
            <dl className="grid grid-cols-2 gap-x-3 gap-y-2">
              <div className="col-span-2 min-w-0">
                <dt className="text-xs text-muted-foreground">{t("Lokasi")}</dt>
                <dd className="truncate font-mono text-[13px]">{info.path}</dd>
              </div>
              <div>
                <dt className="text-xs text-muted-foreground">{t("Ukuran database")}</dt>
                <dd>{formatBytes(info.sizeBytes)}</dd>
              </div>
              <div>
                <dt className="text-xs text-muted-foreground">{t("Event tersimpan")}</dt>
                <dd className="tabular-nums">{formatNumber(info.events)}</dd>
              </div>
              <div>
                <dt className="text-xs text-muted-foreground">{t("Users")}</dt>
                <dd className="tabular-nums">{t("{users} ({deleted} terhapus)", { users: formatNumber(info.users), deleted: formatNumber(info.deletedUsers) })}</dd>
              </div>
              <div>
                <dt className="text-xs text-muted-foreground">{t("Profil PPP terhapus")}</dt>
                <dd className="tabular-nums">{formatNumber(info.deletedProfiles)}</dd>
              </div>
              <div>
                <dt className="text-xs text-muted-foreground">{t("Percakapan AI")}</dt>
                <dd className="tabular-nums">{formatNumber(info.chats)}</dd>
              </div>
            </dl>
            <div className="rounded-lg border p-3">
              <p className="mb-1 font-medium">{t("Bersihkan sekarang")}</p>
              <p className="mb-2 text-xs text-muted-foreground">
                {nothing
                  ? t("Tidak ada yang melewati batas retensi.")
                  : t("Akan dihapus: {events} event, {users} user, {profiles} profile.", { events: formatNumber(p!.events), users: formatNumber(p!.users), profiles: formatNumber(p!.profiles) })}{p && p.chats > 0 ? ` ${t("Ditambah {n} percakapan AI.", { n: p.chats })}` : ""}{" "}
                {t("Memakai pengaturan yang sudah disimpan.")}
              </p>
              <Button variant="outline" size="sm" disabled={nothing || dirty} onClick={() => setConfirm(true)} title={dirty ? t("Simpan perubahan pengaturan dulu") : undefined}>
                {t("Bersihkan sekarang")}
              </Button>
            </div>
          </>
        )}
      </CardContent>
      <ConfirmDialog
        open={confirm}
        onOpenChange={setConfirm}
        title={t("Bersihkan data sekarang?")}
        description={
          <p>
            {t("Data yang melewati batas retensi dihapus permanen dari Telemon ({detail}). Data di router tidak berubah.", { detail: p ? t("{events} event, {users} user, {profiles} profil PPP", { events: formatNumber(p.events), users: formatNumber(p.users), profiles: formatNumber(p.profiles) }) : "" })}
          </p>
        }
        confirmLabel={t("Bersihkan")}
        destructive
        onConfirm={async () => {
          const c = await api.purgeNow()
          toast.success(t("Dibersihkan: {events} event, {users} user, {profiles} profil PPP", { events: formatNumber(c.events), users: formatNumber(c.users), profiles: formatNumber(c.profiles) }))
          load()
        }}
      />
    </Card>
  )
}
