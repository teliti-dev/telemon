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
import { tk, useT } from "@/lib/i18n"
import { cn } from "@/lib/utils"
import type { PppEventType, PppUser, RouterStatus, UserStatus } from "@/lib/types"

/* Warna status hanya untuk titik/ikon; teks tetap memakai token teks. */

export const USER_STATUS: Record<UserStatus, { label: string; dot: string }> = {
  active: { label: tk("Aktif"), dot: "bg-good" },
  inactive: { label: tk("Tidak aktif"), dot: "bg-serious" },
  disabled: {
    label: tk("Dinonaktifkan"),
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
  const t = useT()
  const s = USER_STATUS[status]
  return (
    <Badge variant="outline" className={cn("gap-1.5 font-normal", className)}>
      <span className={cn("size-2 shrink-0 rounded-full", s.dot)} />
      {t(s.label) /* i18n-ignore: etiket sudah berupa kunci (tk) */}
    </Badge>
  )
}

/**
 * Kondisi tampilan user: status dari router + gagal auth terakhir.
 * Warna untuk border/latar/titik; label selalu ikut ditampilkan.
 */
export type UserTone = "active" | "inactive" | "auth_failed" | "disabled" | "deleted"

export function userTone(u: Pick<PppUser, "status" | "lastEvent" | "deletedAt">): UserTone {
  if (u.deletedAt) return "deleted"
  if (u.status === "active") return "active"
  if (u.status === "disabled") return "disabled"
  return u.lastEvent === "auth_failed" ? "auth_failed" : "inactive"
}

export const USER_TONE: Record<
  UserTone,
  { label: string; dot: string; card: string; row: string; bar: string }
> = {
  active: {
    label: tk("Aktif"),
    dot: "bg-good",
    card: "border-good/40 border-l-good",
    row: "bg-good/[0.05] dark:bg-good/[0.10]",
    bar: "shadow-[inset_3px_0_0_var(--color-good)]",
  },
  inactive: {
    label: tk("Tidak aktif"),
    dot: "bg-serious",
    card: "border-serious/45 border-l-serious",
    row: "bg-serious/[0.08] dark:bg-serious/[0.14]",
    bar: "shadow-[inset_3px_0_0_var(--color-serious)]",
  },
  auth_failed: {
    label: tk("Gagal auth"),
    dot: "bg-critical",
    card: "border-critical/45 border-l-critical",
    row: "bg-critical/[0.08] dark:bg-critical/[0.16]",
    bar: "shadow-[inset_3px_0_0_var(--color-critical)]",
  },
  disabled: {
    label: tk("Dinonaktifkan"),
    dot: "border-[1.5px] border-muted-foreground bg-transparent",
    card: "border-dashed border-muted-foreground/45 border-l-muted-foreground/70",
    row: "bg-muted/60 dark:bg-muted/40",
    bar: "shadow-[inset_3px_0_0_var(--color-muted-foreground)]",
  },
  deleted: {
    label: tk("Dihapus"),
    dot: "bg-muted-foreground/30",
    card: "border-dashed border-muted-foreground/30 border-l-muted-foreground/40 opacity-60",
    row: "opacity-60",
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
  const t = useT()
  const k = USER_TONE[tone]
  return (
    <Badge variant="outline" className={cn("gap-1.5 bg-background font-normal", className)}>
      <span className={cn("size-2 shrink-0 rounded-full", k.dot)} />
      {t(k.label) /* i18n-ignore: etiket sudah berupa kunci (tk) */}
    </Badge>
  )
}

export const EVENT_META: Record<
  PppEventType,
  { label: string; icon: LucideIcon; tone: string }
> = {
  auth_failed: { label: tk("Gagal auth"), icon: ShieldXIcon, tone: "text-critical" },
  authenticated: {
    label: tk("Terautentikasi"),
    icon: ShieldCheckIcon,
    tone: "text-muted-foreground",
  },
  connected: { label: tk("Terhubung"), icon: PlugZapIcon, tone: "text-good" },
  disconnected: { label: tk("Terputus"), icon: UnplugIcon, tone: "text-serious" },
  router_down: { label: tk("Router down"), icon: ServerOffIcon, tone: "text-critical" },
  router_up: { label: tk("Router up"), icon: ServerIcon, tone: "text-good" },
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
  const t = useT()
  return (
    <Badge variant="outline" className="gap-1.5 font-normal">
      <EventIcon type={type} className="size-3.5" />
      {t(EVENT_META[type].label) /* i18n-ignore: etiket sudah berupa kunci (tk) */}
    </Badge>
  )
}

export const ROUTER_STATUS: Record<
  RouterStatus,
  { label: string; dot: string; icon: LucideIcon }
> = {
  online: { label: tk("Online"), dot: "bg-good", icon: ServerIcon },
  offline: { label: tk("Offline"), dot: "bg-critical", icon: ServerOffIcon },
  error: { label: tk("Error"), dot: "bg-serious", icon: CircleSlashIcon },
}

export function RouterStatusBadge({ status }: { status: RouterStatus }) {
  const t = useT()
  const s = ROUTER_STATUS[status]
  return (
    <Badge variant="outline" className="gap-1.5 font-normal">
      <span className={cn("size-2 shrink-0 rounded-full", s.dot)} />
      {t(s.label) /* i18n-ignore: etiket sudah berupa kunci (tk) */}
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
  const t = useT()
  return (
    <span
      aria-label={t(ROUTER_STATUS[status].label)} // i18n-ignore: etiket sudah berupa kunci (tk)
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
  const t = useT()
  const total = active + inactive + disabled
  const parts: { n: number; cls: string; label: string }[] = [
    { n: active, cls: "bg-good", label: tk("aktif") },
    { n: inactive, cls: "bg-muted-foreground/40", label: tk("tidak aktif") },
    { n: disabled, cls: "bg-muted-foreground/15", label: tk("dinonaktifkan") },
  ]
  const title = parts.map((p) => `${p.n} ${t(p.label)}`).join(" · ") // i18n-ignore: etiket sudah berupa kunci (tk)
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
