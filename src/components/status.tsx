import {
  CircleSlashIcon,
  PlugZapIcon,
  ServerIcon,
  ServerOffIcon,
  ShieldCheckIcon,
  ShieldXIcon,
  UnplugIcon,
  type LucideIcon,
} from "lucide-react"

import { Badge } from "@/components/ui/badge"
import { cn } from "@/lib/utils"
import type { PppEventType, PppUser, RouterStatus, UserStatus } from "@/lib/types"

/* Warna status hanya untuk titik/ikon; teks tetap memakai token teks. */

export const USER_STATUS: Record<UserStatus, { label: string; dot: string }> = {
  active: { label: "Aktif", dot: "bg-good" },
  inactive: { label: "Tidak aktif", dot: "bg-serious" },
  disabled: {
    label: "Dinonaktifkan",
    dot: "border-[1.5px] border-muted-foreground bg-transparent",
  },
}

export function UserStatusBadge({
  status,
  className,
}: {
  status: UserStatus
  className?: string
}) {
  const s = USER_STATUS[status]
  return (
    <Badge variant="outline" className={cn("gap-1.5 font-normal", className)}>
      <span className={cn("size-2 shrink-0 rounded-full", s.dot)} />
      {s.label}
    </Badge>
  )
}

/**
 * Kondisi tampilan user: status dari router + gagal auth terakhir.
 * Warna untuk border/latar/titik; label selalu ikut ditampilkan.
 */
export type UserTone = "active" | "inactive" | "auth_failed" | "disabled"

export function userTone(u: Pick<PppUser, "status" | "lastEvent">): UserTone {
  if (u.status === "active") return "active"
  if (u.status === "disabled") return "disabled"
  return u.lastEvent === "auth_failed" ? "auth_failed" : "inactive"
}

export const USER_TONE: Record<
  UserTone,
  { label: string; dot: string; card: string; row: string; bar: string }
> = {
  active: {
    label: "Aktif",
    dot: "bg-good",
    card: "border-good/40 border-l-good",
    row: "bg-good/[0.05]",
    bar: "shadow-[inset_3px_0_0_var(--color-good)]",
  },
  inactive: {
    label: "Tidak aktif",
    dot: "bg-serious",
    card: "border-serious/45 border-l-serious",
    row: "bg-serious/[0.08]",
    bar: "shadow-[inset_3px_0_0_var(--color-serious)]",
  },
  auth_failed: {
    label: "Gagal auth",
    dot: "bg-critical",
    card: "border-critical/45 border-l-critical",
    row: "bg-critical/[0.08]",
    bar: "shadow-[inset_3px_0_0_var(--color-critical)]",
  },
  disabled: {
    label: "Dinonaktifkan",
    dot: "border-[1.5px] border-muted-foreground bg-transparent",
    card: "border-dashed border-muted-foreground/45 border-l-muted-foreground/70",
    row: "bg-muted/60",
    bar: "shadow-[inset_3px_0_0_var(--color-muted-foreground)]",
  },
}

export const USER_TONES = Object.keys(USER_TONE) as UserTone[]

export function UserToneBadge({
  tone,
  className,
}: {
  tone: UserTone
  className?: string
}) {
  const t = USER_TONE[tone]
  return (
    <Badge variant="outline" className={cn("gap-1.5 bg-background font-normal", className)}>
      <span className={cn("size-2 shrink-0 rounded-full", t.dot)} />
      {t.label}
    </Badge>
  )
}

export const EVENT_META: Record<
  PppEventType,
  { label: string; icon: LucideIcon; tone: string }
> = {
  auth_failed: { label: "Gagal auth", icon: ShieldXIcon, tone: "text-critical" },
  authenticated: {
    label: "Terautentikasi",
    icon: ShieldCheckIcon,
    tone: "text-muted-foreground",
  },
  connected: { label: "Terhubung", icon: PlugZapIcon, tone: "text-good" },
  disconnected: { label: "Terputus", icon: UnplugIcon, tone: "text-serious" },
  router_down: { label: "Router down", icon: ServerOffIcon, tone: "text-critical" },
  router_up: { label: "Router up", icon: ServerIcon, tone: "text-good" },
}

export const EVENT_TYPES = Object.keys(EVENT_META) as PppEventType[]

export function EventIcon({
  type,
  className,
}: {
  type: PppEventType
  className?: string
}) {
  const m = EVENT_META[type]
  const Icon = m.icon
  return <Icon aria-hidden className={cn("size-4 shrink-0", m.tone, className)} />
}

export function EventBadge({ type }: { type: PppEventType }) {
  return (
    <Badge variant="outline" className="gap-1.5 font-normal">
      <EventIcon type={type} className="size-3.5" />
      {EVENT_META[type].label}
    </Badge>
  )
}

export const ROUTER_STATUS: Record<
  RouterStatus,
  { label: string; dot: string; icon: LucideIcon }
> = {
  online: { label: "Online", dot: "bg-good", icon: ServerIcon },
  offline: { label: "Offline", dot: "bg-critical", icon: ServerOffIcon },
  error: { label: "Error", dot: "bg-serious", icon: CircleSlashIcon },
}

export function RouterStatusBadge({ status }: { status: RouterStatus }) {
  const s = ROUTER_STATUS[status]
  return (
    <Badge variant="outline" className="gap-1.5 font-normal">
      <span className={cn("size-2 shrink-0 rounded-full", s.dot)} />
      {s.label}
    </Badge>
  )
}

export function StatusDot({
  status,
  className,
}: {
  status: RouterStatus
  className?: string
}) {
  return (
    <span
      aria-label={ROUTER_STATUS[status].label}
      className={cn(
        "inline-block size-2 shrink-0 rounded-full",
        ROUTER_STATUS[status].dot,
        className
      )}
    />
  )
}

/**
 * Bar tersegmen tipis: komposisi status user pada satu router.
 * Celah 2px antar segmen; legenda teks di bawahnya membawa angka dan label.
 */
export function UserMixBar({
  active,
  inactive,
  disabled,
  className,
}: {
  active: number
  inactive: number
  disabled: number
  className?: string
}) {
  const total = active + inactive + disabled
  const parts: { n: number; cls: string; label: string }[] = [
    { n: active, cls: "bg-good", label: "aktif" },
    { n: inactive, cls: "bg-muted-foreground/40", label: "tidak aktif" },
    { n: disabled, cls: "bg-muted-foreground/15", label: "dinonaktifkan" },
  ]
  const title = parts.map((p) => `${p.n} ${p.label}`).join(" · ")
  return (
    <div
      role="img"
      aria-label={title}
      title={title}
      className={cn("flex h-2 w-full gap-0.5", className)}
    >
      {total === 0 ? (
        <div className="h-full w-full rounded-full bg-muted" />
      ) : (
        parts
          .filter((p) => p.n > 0)
          .map((p) => (
            <div
              key={p.label}
              className={cn("h-full rounded-full", p.cls)}
              style={{ flexGrow: p.n, flexBasis: 0 }}
            />
          ))
      )}
    </div>
  )
}
