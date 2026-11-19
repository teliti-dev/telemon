import * as React from "react"
import {
  CopyIcon,
  InfoIcon,
  EyeIcon,
  EyeOffIcon,
  PencilIcon,
  PhoneIcon,
  RotateCcwIcon,
  SparklesIcon,
  Trash2Icon,
  TriangleAlertIcon,
} from "lucide-react"
import { toast } from "sonner"

import { ConfirmDialog } from "@/components/confirm-dialog"
import { UserFormDialog } from "@/components/user-form-dialog"
import { useUpdateGate, useWriteGate } from "@/hooks/use-write-gate"
import { PageHeader } from "@/components/page-header"
import { navigatePath } from "@/lib/route"
import { useAiChat } from "@/lib/ai-context"
import {
  EVENT_META,
  EventIcon,
  UserToneBadge,
  userTone,
} from "@/components/status"
import { Alert, AlertDescription } from "@/components/ui/alert"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { ScrollArea } from "@/components/ui/scroll-area"
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
} from "@/components/ui/sheet"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { Textarea } from "@/components/ui/textarea"
import { copyText } from "@/hooks/use-copy"
import { useNow } from "@/hooks/use-now"
import {
  formatDateTime,
  formatDuration,
  formatRelative,
  formatTime,
} from "@/lib/format"
import { tk, useT } from "@/lib/i18n"
import { useStore } from "@/lib/store-context"
import { cn } from "@/lib/utils"
import type { PppEvent, PppUser, RouterInfo, UserMeta } from "@/lib/types"

const DAY = 86400_000
const HOUR = 3600_000

function Field({
  label,
  value,
  mono,
  copy,
}: {
  label: string
  value?: React.ReactNode
  mono?: boolean
  copy?: string
}) {
  const t = useT()
  const l = t(label) // i18n-ignore: etiket sudah berupa kunci (tk)
  return (
    <div className="min-w-0">
      <dt className="text-xs text-muted-foreground">{l}</dt>
      <dd className="flex items-center gap-1">
        <span className={cn("min-w-0 truncate", mono && "font-mono text-[13px]")}>
          {value ?? "-"}
        </span>
        {copy && (
          <button
            type="button"
            aria-label={t("Salin {label}", { label: l })}
            title={t("Salin {label}", { label: l })}
            onClick={() => copyText(copy, t("{label} disalin", { label: l }))}
            className="shrink-0 rounded p-1 text-muted-foreground md:p-0.5 hover:bg-muted hover:text-foreground"
          >
            <CopyIcon className="size-3.5" />
          </button>
        )}
      </dd>
    </div>
  )
}

/** 24 batang, satu per jam; warna = kejadian terburuk pada jam itu. */
/**
 * Password PPPoE: tersembunyi bawaan, tombol tampil/sembunyikan dan salin. Hanya untuk peran berizin Ubah.
 * Di server nyata password tidak ikut snapshot: diambil saat ditampilkan/disalin (dan tercatat di riwayat server).
 */
function PasswordField({ user }: { user: PppUser }) {
  const t = useT()
  const { api } = useStore()
  const [shown, setShown] = React.useState<string | null>(null)
  const [fetched, setFetched] = React.useState<{ id: string; value: string } | null>(null)
  const [busy, setBusy] = React.useState(false)
  const remote = !!api.revealUserPassword && user.password === undefined
  const known = user.password ?? (fetched?.id === user.id ? fetched.value : undefined)
  const visible = shown === user.id && known !== undefined
  const btn = "shrink-0 rounded p-1 text-muted-foreground md:p-0.5 hover:bg-muted hover:text-foreground disabled:opacity-50"

  async function load(): Promise<string | undefined> {
    if (known !== undefined) return known
    if (!remote) return undefined
    setBusy(true)
    try {
      const value = await api.revealUserPassword!(user.id)
      setFetched({ id: user.id, value })
      return value
    } catch (e) {
      toast.error(e instanceof Error && /tidak ditemukan/i.test(e.message) ? t("Password belum tersimpan di Telemon") : e instanceof Error ? e.message : String(e))
      return undefined
    } finally {
      setBusy(false)
    }
  }

  const available = known !== undefined || remote
  return (
    <div className="min-w-0">
      <dt className="text-xs text-muted-foreground">{t("Password PPPoE")}</dt>
      <dd className="flex items-center gap-1">
        <span className="min-w-0 truncate font-mono text-[13px]">{available ? (visible ? known : "••••••••") : "-"}</span>
        {available && (
          <>
            <button
              type="button"
              disabled={busy}
              aria-label={visible ? t("Sembunyikan password") : t("Tampilkan password")}
              title={visible ? t("Sembunyikan password") : t("Tampilkan password")}
              aria-pressed={visible}
              onClick={async () => {
                if (visible) return setShown(null)
                if ((await load()) !== undefined) setShown(user.id)
              }}
              className={btn}
            >
              {visible ? <EyeOffIcon className="size-3.5" /> : <EyeIcon className="size-3.5" />}
            </button>
            <button
              type="button"
              disabled={busy}
              aria-label={t("Salin password")}
              title={t("Salin password")}
              onClick={async () => {
                const v = await load()
                if (v !== undefined) copyText(v, t("Password disalin"))
              }}
              className={btn}
            >
              <CopyIcon className="size-3.5" />
            </button>
          </>
        )}
      </dd>
    </div>
  )
}

function ActivityStrip({ events, now }: { events: PppEvent[]; now: number }) {
  const t = useT()
  const buckets = Array.from({ length: 24 }, () => ({ c: 0, d: 0, a: 0 }))
  for (const e of events) {
    const age = now - e.at
    if (age < 0 || age >= DAY) continue
    const b = buckets[23 - Math.floor(age / HOUR)]
    if (e.type === "connected") b.c++
    else if (e.type === "disconnected") b.d++
    else if (e.type === "auth_failed") b.a++
  }
  const summary = buckets.reduce(
    (s, b) => ({ c: s.c + b.c, d: s.d + b.d, a: s.a + b.a }),
    { c: 0, d: 0, a: 0 }
  )
  return (
    <div>
      <div
        role="img"
        aria-label={t("24 jam terakhir: {c} terhubung, {d} terputus, {a} gagal auth", { c: summary.c, d: summary.d, a: summary.a })}
        className="flex h-6 items-end gap-0.5"
      >
        {buckets.map((b, i) => {
          const hour = new Date(now - (23 - i) * HOUR).getHours()
          const tone =
            b.a > 0 ? "bg-critical" : b.d > 0 ? "bg-serious" : b.c > 0 ? "bg-good" : "bg-muted"
          return (
            <div
              key={i}
              title={`${String(hour).padStart(2, "0")}:00 · ${t("{c} terhubung, {d} terputus, {a} gagal auth", { c: b.c, d: b.d, a: b.a })}`}
              className={cn("min-w-0 flex-1 rounded-sm", tone, tone === "bg-muted" ? "h-1.5" : "h-6")}
            />
          )
        })}
      </div>
      <div className="mt-1 flex justify-between text-[11px] text-muted-foreground">
        <span>{t("24 jam lalu")}</span>
        <span>{t("sekarang")}</span>
      </div>
    </div>
  )
}

function InfoTab({
  user,
  router,
  history,
  now,
}: {
  user: PppUser
  router?: RouterInfo
  history: PppEvent[]
  now: number
}) {
  const t = useT()
  const day = history.filter((e) => now - e.at < DAY)
  const count = (ty: PppEvent["type"]) => day.filter((e) => e.type === ty).length
  const connected = count("connected")
  const disconnected = count("disconnected")
  const failed = count("auth_failed")
  const lastFail = day.find((e) => e.type === "auth_failed")
  const tone = userTone(user)
  const canSeePassword = useUpdateGate("user")() === null

  const notices: React.ReactNode[] = []
  if (user.deletedAt) {
    notices.push(
      <Alert key="deleted">
        <InfoIcon />
        <AlertDescription>
          {user.deletedReason === "router"
            ? t("Dihapus (soft) {when}, karena sudah tidak ada di router saat import.", { when: formatRelative(user.deletedAt, now) })
            : t("Dihapus (soft) {when}.", { when: formatRelative(user.deletedAt, now) })}{" "}
          {t("Secret dinonaktifkan di router dan bisa dipulihkan.")}
        </AlertDescription>
      </Alert>
    )
  }
  if (router && router.status !== "online") {
    notices.push(
      <Alert key="router">
        <InfoIcon />
        <AlertDescription>
          {router.status === "offline"
            ? t("Router {name} sedang offline; status user ini mungkin bukan yang terbaru.", { name: router.name })
            : t("Router {name} sedang error; status user ini mungkin bukan yang terbaru.", { name: router.name })}
        </AlertDescription>
      </Alert>
    )
  }
  if (failed > 0 && tone !== "active") {
    notices.push(
      <Alert key="auth" variant="destructive">
        <TriangleAlertIcon />
        <AlertDescription>
          {lastFail
            ? t("Gagal autentikasi {n}× dalam 24 jam, terakhir {when}.", { n: failed, when: formatRelative(lastFail.at, now) })
            : t("Gagal autentikasi {n}× dalam 24 jam.", { n: failed })}{" "}
          {t("Cek username dan password di perangkat pelanggan, atau secret di router.")}
        </AlertDescription>
      </Alert>
    )
  }
  if (disconnected >= 4) {
    notices.push(
      <Alert key="flap">
        <TriangleAlertIcon />
        <AlertDescription>
          {t("Sering putus: {n}× dalam 24 jam. Cek kabel, ONU, atau sinyal.", { n: disconnected })}
        </AlertDescription>
      </Alert>
    )
  }
  if (user.status === "disabled" && !user.deletedAt) {
    notices.push(
      <Alert key="disabled">
        <InfoIcon />
        <AlertDescription>
          {t("Secret dinonaktifkan di router, jadi user ini tidak bisa login.")}
        </AlertDescription>
      </Alert>
    )
  }

  return (
    <div className="grid gap-3 p-3">
      {notices}

      <section aria-label={t("Sesi")}>
        <h3 className="mb-1.5 text-xs font-medium text-muted-foreground">{t("Sesi")}</h3>
        <dl className="grid grid-cols-2 gap-x-3 gap-y-2 text-[13px]">
          <Field
            label={tk("Uptime")}
            value={user.connectedAt ? formatDuration(now - user.connectedAt) : t("Tidak terhubung")}
          />
          <Field
            label={user.connectedAt ? tk("Mulai sesi") : tk("Terakhir terlihat")}
            value={
              user.connectedAt
                ? formatDateTime(user.connectedAt)
                : user.lastSeenAt
                  ? formatRelative(user.lastSeenAt, now)
                  : undefined
            }
          />
          <Field label="IP" value={user.address} mono copy={user.address} />
          <Field label={tk("MAC (Caller ID)")} value={user.callerId} mono copy={user.callerId} />
          <Field label={tk("Profil")} value={user.profile} />
          <Field label={tk("IP statis")} value={user.staticAddress} mono />
          <Field label={tk("Router")} value={router?.name} />
          {canSeePassword && <PasswordField user={user} />}
        </dl>
      </section>

      <section aria-label={t("24 jam terakhir")}>
        <div className="mb-1.5 flex items-baseline justify-between gap-2">
          <h3 className="text-xs font-medium text-muted-foreground">{t("24 jam terakhir")}</h3>
          <p className="text-xs text-muted-foreground tabular-nums">
            {t("{n} terhubung", { n: connected })} · {t("{n} terputus", { n: disconnected })} ·{" "}
            <span className={failed > 0 ? "font-medium text-foreground" : undefined}>
              {t("{n} gagal auth", { n: failed })}
            </span>
          </p>
        </div>
        <ActivityStrip events={history} now={now} />
      </section>

      {user.comment && (
        <section aria-label={t("Komentar di router")}>
          <h3 className="mb-1 text-xs font-medium text-muted-foreground">{t("Komentar di router")}</h3>
          <p className="text-[13px]">{user.comment}</p>
        </section>
      )}
    </div>
  )
}

function HistoryTab({ history, now }: { history: PppEvent[]; now: number }) {
  const t = useT()
  if (history.length === 0) {
    return <p className="p-3 text-sm text-muted-foreground">{t("Belum ada event untuk user ini.")}</p>
  }
  return (
    <ol className="divide-y">
      {history.map((e) => (
        <li key={e.id} className="flex gap-2.5 px-3 py-2 text-[13px]">
          <EventIcon type={e.type} className="mt-0.5" />
          <div className="min-w-0 flex-1">
            <div className="flex flex-wrap items-baseline justify-between gap-x-2">
              <span className="font-medium">{t(EVENT_META[e.type].label) /* i18n-ignore: etiket sudah berupa kunci (tk) */}</span>
              <time
                className="text-xs text-muted-foreground tabular-nums"
                dateTime={new Date(e.at).toISOString()}
                title={formatDateTime(e.at)}
              >
                {formatTime(e.at)} · {formatRelative(e.at, now)}
              </time>
            </div>
            <p className="font-mono text-xs break-all text-muted-foreground">{e.message}</p>
          </div>
        </li>
      ))}
    </ol>
  )
}

function NotesTab({
  userId,
  meta,
  canEdit,
}: {
  userId: string
  meta?: UserMeta
  canEdit: boolean
}) {
  const t = useT()
  const { api } = useStore()
  const [editing, setEditing] = React.useState(false)
  const [busy, setBusy] = React.useState(false)

  async function onSubmit(e: React.FormEvent<HTMLFormElement>) {
    e.preventDefault()
    const f = new FormData(e.currentTarget)
    setBusy(true)
    try {
      await api.saveUserMeta(userId, {
        customerName: String(f.get("customerName") ?? ""),
        phone: String(f.get("phone") ?? ""),
        note: String(f.get("note") ?? ""),
        tags: String(f.get("tags") ?? "").split(","),
      })
      toast.success(t("Catatan disimpan"))
      setEditing(false)
    } catch (err) {
      toast.error(err instanceof Error ? err.message : String(err))
    } finally {
      setBusy(false)
    }
  }

  if (editing) {
    return (
      <form onSubmit={onSubmit} className="grid gap-3 p-3">
        <div className="grid gap-1.5">
          <Label htmlFor="n-name">{t("Nama pelanggan")}</Label>
          <Input id="n-name" name="customerName" defaultValue={meta?.customerName} maxLength={80} autoFocus />
        </div>
        <div className="grid gap-1.5">
          <Label htmlFor="n-phone">{t("Telepon / WhatsApp")}</Label>
          <Input id="n-phone" name="phone" type="tel" inputMode="tel" defaultValue={meta?.phone} maxLength={20} />
        </div>
        <div className="grid gap-1.5">
          <Label htmlFor="n-tags">{t("Label")}</Label>
          <Input
            id="n-tags"
            name="tags"
            defaultValue={meta?.tags.join(", ")}
            placeholder={t("VIP, Tunggakan")}
            maxLength={120}
          />
          <p className="text-xs text-muted-foreground">{t("Pisahkan dengan koma, maksimal 8.")}</p>
        </div>
        <div className="grid gap-1.5">
          <Label htmlFor="n-note">{t("Catatan")}</Label>
          <Textarea id="n-note" name="note" rows={4} defaultValue={meta?.note} maxLength={500} />
        </div>
        <p className="text-xs text-muted-foreground">
          {t("Disimpan di Telemon saja. Secret di router tidak diubah.")}
        </p>
        <div className="flex justify-end gap-2">
          <Button type="button" variant="outline" onClick={() => setEditing(false)} disabled={busy}>
            {t("Batal")}
          </Button>
          <Button type="submit" disabled={busy}>
            {busy ? t("Menyimpan…") : t("Simpan")}
          </Button>
        </div>
      </form>
    )
  }

  const empty = !meta || (!meta.customerName && !meta.phone && !meta.note && meta.tags.length === 0)

  return (
    <div className="grid gap-3 p-3">
      {empty ? (
        <p className="text-sm text-muted-foreground">
          {t("Belum ada catatan. Tambahkan nama pelanggan, kontak, atau label agar user ini mudah dikenali.")}
        </p>
      ) : (
        <dl className="grid gap-2.5 text-[13px]">
          <Field label={tk("Nama pelanggan")} value={meta?.customerName} />
          <div className="min-w-0">
            <dt className="text-xs text-muted-foreground">{t("Telepon / WhatsApp")}</dt>
            <dd>
              {meta?.phone ? (
                <a
                  href={`tel:${meta.phone}`}
                  className="inline-flex items-center gap-1.5 font-mono text-[13px] hover:underline"
                >
                  <PhoneIcon className="size-3.5" />
                  {meta.phone}
                </a>
              ) : (
                "-"
              )}
            </dd>
          </div>
          <div className="min-w-0">
            <dt className="text-xs text-muted-foreground">{t("Label")}</dt>
            <dd className="flex flex-wrap gap-1">
              {meta && meta.tags.length > 0
                ? meta.tags.map((tag) => (
                    <Badge key={tag} variant="secondary">
                      {tag}
                    </Badge>
                  ))
                : "-"}
            </dd>
          </div>
          <div className="min-w-0">
            <dt className="text-xs text-muted-foreground">{t("Catatan")}</dt>
            <dd className="whitespace-pre-wrap">{meta?.note ?? "-"}</dd>
          </div>
          {meta && (
            <p className="text-xs text-muted-foreground">
              {t("Diubah {when}", { when: formatDateTime(meta.updatedAt) })}
            </p>
          )}
        </dl>
      )}
      {canEdit ? (
        <Button variant="outline" className="w-fit" onClick={() => setEditing(true)}>
          <PencilIcon /> {empty ? t("Tambah catatan") : t("Edit")}
        </Button>
      ) : (
        <p className="text-xs text-muted-foreground">
          {t("Catatan hanya bisa diubah dari aplikasi desktop.")}
        </p>
      )}
    </div>
  )
}

export function UserDetailSheet({
  userId,
  onOpenChange,
}: {
  userId: string | null
  onOpenChange: (open: boolean) => void
}) {
  const t = useT()
  const { users, events, routerById, metas, api } = useStore()
  const now = useNow(5_000)
  const gate = useWriteGate("user")
  const noteGate = useUpdateGate("user")
  const ai = useAiChat()
  const [editing, setEditing] = React.useState(false)
  const [removing, setRemoving] = React.useState(false)
  const user = userId ? users.find((u) => u.id === userId) : undefined
  const router = user ? routerById.get(user.routerId) : undefined
  const meta = user ? metas[user.id] : undefined
  const history = React.useMemo(
    () =>
      user
        ? events
            .filter((e) => e.routerId === user.routerId && e.username === user.username)
            .slice(0, 150)
        : [],
    [events, user]
  )

  const blocked = user ? gate(user.routerId) : null

  async function restore() {
    if (!user) return
    try {
      await api.restoreUser(user.id)
      toast.success(t("{user} dipulihkan", { user: user.username }))
    } catch (e) {
      toast.error(e instanceof Error ? e.message : String(e))
    }
  }

  return (
    <>
    <Sheet open={!!user} onOpenChange={onOpenChange}>
      <SheetContent className="gap-0 data-[side=right]:w-full pt-[env(safe-area-inset-top)] pb-[env(safe-area-inset-bottom)]">
        {user && (
          <>
            <SheetHeader className="gap-1 border-b px-3 py-2.5 pr-12">
              <SheetTitle className="font-mono text-base break-all">{user.username}</SheetTitle>
              <SheetDescription asChild>
                <div className="flex flex-wrap items-center gap-x-2 gap-y-1">
                  <UserToneBadge tone={userTone(user)} />
                  {meta?.customerName && (
                    <span className="text-sm text-foreground">{meta.customerName}</span>
                  )}
                  <span className="text-xs">{router?.name}</span>
                  {meta?.tags.map((tag) => (
                    <Badge key={tag} variant="secondary">
                      {tag}
                    </Badge>
                  ))}
                </div>
              </SheetDescription>
              <div className="flex flex-wrap items-center gap-1.5 pt-1">
                {user.deletedAt ? (
                  <Button size="sm" variant="outline" disabled={!!blocked} onClick={restore}>
                    <RotateCcwIcon /> {t("Pulihkan")}
                  </Button>
                ) : (
                  <>
                    <Button size="sm" variant="outline" disabled={!!blocked} onClick={() => setEditing(true)}>
                      <PencilIcon /> {t("Edit")}
                    </Button>
                    <Button size="sm" variant="outline" disabled={!!blocked} onClick={() => setRemoving(true)}>
                      <Trash2Icon /> {t("Hapus")}
                    </Button>
                  </>
                )}
                {ai.available && (
                  <Button
                    size="sm"
                    variant="outline"
                    onClick={() => {
                      onOpenChange(false)
                      ai.ask(`${t("cek")} ${user.username}`)
                    }}
                  >
                    <SparklesIcon /> {t("Tanya AI")}
                  </Button>
                )}
                {blocked && <span className="text-xs text-muted-foreground">{blocked}.</span>}
              </div>
            </SheetHeader>

            <Tabs key={user.id} defaultValue="info" className="min-h-0 flex-1 gap-0">
              <div className="border-b px-3 py-2">
                <TabsList className="w-full">
                  <TabsTrigger value="info">{t("Info")}</TabsTrigger>
                  <TabsTrigger value="riwayat">
                    {t("Riwayat")}
                    <span className="text-xs text-muted-foreground tabular-nums">{history.length}</span>
                  </TabsTrigger>
                  <TabsTrigger value="catatan">{t("Catatan")}</TabsTrigger>
                </TabsList>
              </div>
              <ScrollArea className="min-h-0 flex-1">
                <TabsContent value="info">
                  <InfoTab user={user} router={router} history={history} now={now} />
                </TabsContent>
                <TabsContent value="riwayat">
                  <HistoryTab history={history} now={now} />
                </TabsContent>
                <TabsContent value="catatan">
                  <NotesTab userId={user.id} meta={meta} canEdit={!noteGate()} />
                </TabsContent>
              </ScrollArea>
            </Tabs>
          </>
        )}
      </SheetContent>
    </Sheet>
    {user && (
      <>
        <UserFormDialog open={editing} onOpenChange={setEditing} mode="edit" user={user} />
        <ConfirmDialog
          open={removing}
          onOpenChange={setRemoving}
          title={t("Hapus {user}?", { user: user.username })}
          description={
            <p>
              {t("Soft delete: secret di")} <b>{router?.name}</b>{" "}
              {t("dinonaktifkan dan sesi yang sedang berjalan diputus. Secret tidak dihapus dari router dan bisa dipulihkan dari Telemon.")}
            </p>
          }
          confirmLabel={t("Hapus")}
          destructive
          onConfirm={async () => {
            await api.softDeleteUser(user.id)
            toast.success(t("{user} dihapus (soft)", { user: user.username }))
          }}
        />
      </>
    )}
    </>
  )
}


/**
 * Detail user sebagai halaman penuh (HP): isi sama dengan sheet di desktop, tetapi bisa dibuka
 * lewat alamat sendiri (#/user/<id>) dan tombol kembali bawaan browser berfungsi.
 */
export function UserDetailPage({ userId, onBack }: { userId: string; onBack: () => void }) {
  const t = useT()
  const { users, events, routerById, metas, api } = useStore()
  const now = useNow(5_000)
  const gate = useWriteGate("user")
  const noteGate = useUpdateGate("user")
  const ai = useAiChat()
  const [removing, setRemoving] = React.useState(false)
  const user = users.find((u) => u.id === userId)
  const router = user ? routerById.get(user.routerId) : undefined
  const meta = user ? metas[user.id] : undefined
  const history = React.useMemo(
    () => (user ? events.filter((e) => e.routerId === user.routerId && e.username === user.username).slice(0, 150) : []),
    [events, user]
  )

  if (!user) {
    return (
      <div className="mx-auto max-w-xl">
        <PageHeader title={t("User tidak ditemukan")} onBack={onBack} />
        <p className="text-sm text-muted-foreground">{t("User ini tidak ada lagi atau Anda tidak boleh melihatnya.")}</p>
      </div>
    )
  }

  const blocked = gate(user.routerId)

  return (
    <div className="mx-auto max-w-xl">
      <PageHeader title={<span className="font-mono">{user.username}</span>} subtitle={router?.name} onBack={onBack} />

      <div className="mb-3 flex flex-wrap items-center gap-x-2 gap-y-1">
        <UserToneBadge tone={userTone(user)} />
        {meta?.customerName && <span className="text-sm">{meta.customerName}</span>}
        {meta?.tags.map((tag) => (
          <Badge key={tag} variant="secondary">
            {tag}
          </Badge>
        ))}
      </div>

      <div className="mb-3 flex flex-wrap items-center gap-1.5">
        {user.deletedAt ? (
          <Button
            size="sm"
            variant="outline"
            disabled={!!blocked}
            onClick={async () => {
              try {
                await api.restoreUser(user.id)
                toast.success(t("{user} dipulihkan", { user: user.username }))
              } catch (e) {
                toast.error(e instanceof Error ? e.message : String(e))
              }
            }}
          >
            <RotateCcwIcon /> {t("Pulihkan")}
          </Button>
        ) : (
          <>
            <Button size="sm" variant="outline" disabled={!!blocked} onClick={() => navigatePath("user", [user.id, "edit"])}>
              <PencilIcon /> {t("Edit")}
            </Button>
            <Button size="sm" variant="outline" disabled={!!blocked} onClick={() => setRemoving(true)}>
              <Trash2Icon /> {t("Hapus")}
            </Button>
          </>
        )}
        {ai.available && (
          <Button size="sm" variant="outline" onClick={() => ai.ask(`${t("cek")} ${user.username}`)}>
            <SparklesIcon /> {t("Tanya AI")}
          </Button>
        )}
        {blocked && <span className="text-xs text-muted-foreground">{blocked}.</span>}
      </div>

      <Tabs key={user.id} defaultValue="info" className="gap-3">
        <TabsList className="w-full">
          <TabsTrigger value="info">{t("Info")}</TabsTrigger>
          <TabsTrigger value="riwayat">
            {t("Riwayat")}
            <span className="text-xs text-muted-foreground tabular-nums">{history.length}</span>
          </TabsTrigger>
          <TabsTrigger value="catatan">{t("Catatan")}</TabsTrigger>
        </TabsList>
        <div className="rounded-xl border">
          <TabsContent value="info">
            <InfoTab user={user} router={router} history={history} now={now} />
          </TabsContent>
          <TabsContent value="riwayat">
            <HistoryTab history={history} now={now} />
          </TabsContent>
          <TabsContent value="catatan">
            <NotesTab userId={user.id} meta={meta} canEdit={!noteGate()} />
          </TabsContent>
        </div>
      </Tabs>

      <ConfirmDialog
        open={removing}
        onOpenChange={setRemoving}
        title={t("Hapus {user}?", { user: user.username })}
        description={
          <p>
            {t("Soft delete: secret di")} <b>{router?.name}</b>{" "}
            {t("dinonaktifkan dan sesi yang sedang berjalan diputus. Secret tidak dihapus dari router dan bisa dipulihkan dari Telemon.")}
          </p>
        }
        confirmLabel={t("Hapus")}
        destructive
        onConfirm={async () => {
          await api.softDeleteUser(user.id)
          toast.success(t("{user} dihapus (soft)", { user: user.username }))
        }}
      />
    </div>
  )
}
