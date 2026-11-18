import * as React from "react"
import {
  CopyIcon,
  InfoIcon,
  PencilIcon,
  PhoneIcon,
  TriangleAlertIcon,
} from "lucide-react"
import { toast } from "sonner"

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
  return (
    <div className="min-w-0">
      <dt className="text-xs text-muted-foreground">{label}</dt>
      <dd className="flex items-center gap-1">
        <span className={cn("min-w-0 truncate", mono && "font-mono text-[13px]")}>
          {value ?? "-"}
        </span>
        {copy && (
          <button
            type="button"
            aria-label={`Salin ${label}`}
            title={`Salin ${label}`}
            onClick={() => copyText(copy, `${label} disalin`)}
            className="shrink-0 rounded p-0.5 text-muted-foreground hover:bg-muted hover:text-foreground"
          >
            <CopyIcon className="size-3.5" />
          </button>
        )}
      </dd>
    </div>
  )
}

/** 24 batang, satu per jam; warna = kejadian terburuk pada jam itu. */
function ActivityStrip({ events, now }: { events: PppEvent[]; now: number }) {
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
    (t, b) => ({ c: t.c + b.c, d: t.d + b.d, a: t.a + b.a }),
    { c: 0, d: 0, a: 0 }
  )
  return (
    <div>
      <div
        role="img"
        aria-label={`24 jam terakhir: ${summary.c} terhubung, ${summary.d} terputus, ${summary.a} gagal auth`}
        className="flex h-6 items-end gap-0.5"
      >
        {buckets.map((b, i) => {
          const hour = new Date(now - (23 - i) * HOUR).getHours()
          const tone =
            b.a > 0 ? "bg-critical" : b.d > 0 ? "bg-serious" : b.c > 0 ? "bg-good" : "bg-muted"
          return (
            <div
              key={i}
              title={`${String(hour).padStart(2, "0")}:00 · ${b.c} terhubung, ${b.d} terputus, ${b.a} gagal auth`}
              className={cn("min-w-0 flex-1 rounded-sm", tone, tone === "bg-muted" ? "h-1.5" : "h-6")}
            />
          )
        })}
      </div>
      <div className="mt-1 flex justify-between text-[11px] text-muted-foreground">
        <span>24 jam lalu</span>
        <span>sekarang</span>
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
  const day = history.filter((e) => now - e.at < DAY)
  const count = (t: PppEvent["type"]) => day.filter((e) => e.type === t).length
  const connected = count("connected")
  const disconnected = count("disconnected")
  const failed = count("auth_failed")
  const lastFail = day.find((e) => e.type === "auth_failed")
  const tone = userTone(user)

  const notices: React.ReactNode[] = []
  if (router && router.status !== "online") {
    notices.push(
      <Alert key="router">
        <InfoIcon />
        <AlertDescription>
          Router {router.name} sedang {router.status === "offline" ? "offline" : "error"};
          status user ini mungkin bukan yang terbaru.
        </AlertDescription>
      </Alert>
    )
  }
  if (failed > 0 && tone !== "active") {
    notices.push(
      <Alert key="auth" variant="destructive">
        <TriangleAlertIcon />
        <AlertDescription>
          Gagal autentikasi {failed}× dalam 24 jam
          {lastFail && `, terakhir ${formatRelative(lastFail.at, now)}`}. Cek username
          dan password di perangkat pelanggan, atau secret di router.
        </AlertDescription>
      </Alert>
    )
  }
  if (disconnected >= 4) {
    notices.push(
      <Alert key="flap">
        <TriangleAlertIcon />
        <AlertDescription>
          Sering putus: {disconnected}× dalam 24 jam. Cek kabel, ONU, atau sinyal.
        </AlertDescription>
      </Alert>
    )
  }
  if (user.status === "disabled") {
    notices.push(
      <Alert key="disabled">
        <InfoIcon />
        <AlertDescription>
          Secret dinonaktifkan di router, jadi user ini tidak bisa login.
        </AlertDescription>
      </Alert>
    )
  }

  return (
    <div className="grid gap-3 p-3">
      {notices}

      <section aria-label="Sesi">
        <h3 className="mb-1.5 text-xs font-medium text-muted-foreground">Sesi</h3>
        <dl className="grid grid-cols-2 gap-x-3 gap-y-2 text-[13px]">
          <Field
            label="Uptime"
            value={user.connectedAt ? formatDuration(now - user.connectedAt) : "Tidak terhubung"}
          />
          <Field
            label={user.connectedAt ? "Mulai sesi" : "Terakhir terlihat"}
            value={
              user.connectedAt
                ? formatDateTime(user.connectedAt)
                : user.lastSeenAt
                  ? formatRelative(user.lastSeenAt, now)
                  : undefined
            }
          />
          <Field label="IP" value={user.address} mono copy={user.address} />
          <Field label="MAC (Caller ID)" value={user.callerId} mono copy={user.callerId} />
          <Field label="Profil" value={user.profile} />
          <Field label="Router" value={router?.name} />
        </dl>
      </section>

      <section aria-label="24 jam terakhir">
        <div className="mb-1.5 flex items-baseline justify-between gap-2">
          <h3 className="text-xs font-medium text-muted-foreground">24 jam terakhir</h3>
          <p className="text-xs text-muted-foreground tabular-nums">
            {connected} terhubung · {disconnected} terputus ·{" "}
            <span className={failed > 0 ? "font-medium text-foreground" : undefined}>
              {failed} gagal auth
            </span>
          </p>
        </div>
        <ActivityStrip events={history} now={now} />
      </section>

      {user.comment && (
        <section aria-label="Komentar di router">
          <h3 className="mb-1 text-xs font-medium text-muted-foreground">Komentar di router</h3>
          <p className="text-[13px]">{user.comment}</p>
        </section>
      )}
    </div>
  )
}

function HistoryTab({ history, now }: { history: PppEvent[]; now: number }) {
  if (history.length === 0) {
    return <p className="p-3 text-sm text-muted-foreground">Belum ada event untuk user ini.</p>
  }
  return (
    <ol className="divide-y">
      {history.map((e) => (
        <li key={e.id} className="flex gap-2.5 px-3 py-2 text-[13px]">
          <EventIcon type={e.type} className="mt-0.5" />
          <div className="min-w-0 flex-1">
            <div className="flex flex-wrap items-baseline justify-between gap-x-2">
              <span className="font-medium">{EVENT_META[e.type].label}</span>
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
      toast.success("Catatan disimpan")
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
          <Label htmlFor="n-name">Nama pelanggan</Label>
          <Input id="n-name" name="customerName" defaultValue={meta?.customerName} maxLength={80} autoFocus />
        </div>
        <div className="grid gap-1.5">
          <Label htmlFor="n-phone">Telepon / WhatsApp</Label>
          <Input id="n-phone" name="phone" type="tel" inputMode="tel" defaultValue={meta?.phone} maxLength={20} />
        </div>
        <div className="grid gap-1.5">
          <Label htmlFor="n-tags">Label</Label>
          <Input
            id="n-tags"
            name="tags"
            defaultValue={meta?.tags.join(", ")}
            placeholder="VIP, Tunggakan"
            maxLength={120}
          />
          <p className="text-xs text-muted-foreground">Pisahkan dengan koma, maksimal 8.</p>
        </div>
        <div className="grid gap-1.5">
          <Label htmlFor="n-note">Catatan</Label>
          <Textarea id="n-note" name="note" rows={4} defaultValue={meta?.note} maxLength={500} />
        </div>
        <p className="text-xs text-muted-foreground">
          Disimpan di Telemon saja. Secret di router tidak diubah.
        </p>
        <div className="flex justify-end gap-2">
          <Button type="button" variant="outline" onClick={() => setEditing(false)} disabled={busy}>
            Batal
          </Button>
          <Button type="submit" disabled={busy}>
            {busy ? "Menyimpan…" : "Simpan"}
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
          Belum ada catatan. Tambahkan nama pelanggan, kontak, atau label agar user ini mudah dikenali.
        </p>
      ) : (
        <dl className="grid gap-2.5 text-[13px]">
          <Field label="Nama pelanggan" value={meta?.customerName} />
          <div className="min-w-0">
            <dt className="text-xs text-muted-foreground">Telepon / WhatsApp</dt>
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
            <dt className="text-xs text-muted-foreground">Label</dt>
            <dd className="flex flex-wrap gap-1">
              {meta && meta.tags.length > 0
                ? meta.tags.map((t) => (
                    <Badge key={t} variant="secondary">
                      {t}
                    </Badge>
                  ))
                : "-"}
            </dd>
          </div>
          <div className="min-w-0">
            <dt className="text-xs text-muted-foreground">Catatan</dt>
            <dd className="whitespace-pre-wrap">{meta?.note ?? "-"}</dd>
          </div>
          {meta && (
            <p className="text-xs text-muted-foreground">
              Diubah {formatDateTime(meta.updatedAt)}
            </p>
          )}
        </dl>
      )}
      {canEdit ? (
        <Button variant="outline" className="w-fit" onClick={() => setEditing(true)}>
          <PencilIcon /> {empty ? "Tambah catatan" : "Edit"}
        </Button>
      ) : (
        <p className="text-xs text-muted-foreground">
          Catatan hanya bisa diubah dari aplikasi desktop.
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
  const { users, events, routerById, metas, api } = useStore()
  const now = useNow(5_000)
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

  return (
    <Sheet open={!!user} onOpenChange={onOpenChange}>
      <SheetContent className="w-full gap-0 sm:max-w-md">
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
                  {meta?.tags.map((t) => (
                    <Badge key={t} variant="secondary">
                      {t}
                    </Badge>
                  ))}
                </div>
              </SheetDescription>
            </SheetHeader>

            <Tabs key={user.id} defaultValue="info" className="min-h-0 flex-1 gap-0">
              <div className="border-b px-3 py-2">
                <TabsList className="w-full">
                  <TabsTrigger value="info">Info</TabsTrigger>
                  <TabsTrigger value="riwayat">
                    Riwayat
                    <span className="text-xs text-muted-foreground tabular-nums">{history.length}</span>
                  </TabsTrigger>
                  <TabsTrigger value="catatan">Catatan</TabsTrigger>
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
                  <NotesTab userId={user.id} meta={meta} canEdit={api.kind !== "http"} />
                </TabsContent>
              </ScrollArea>
            </Tabs>
          </>
        )}
      </SheetContent>
    </Sheet>
  )
}
