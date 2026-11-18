import { EventIcon, EVENT_META, UserStatusBadge } from "@/components/status"
import { ScrollArea } from "@/components/ui/scroll-area"
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
} from "@/components/ui/sheet"
import { useNow } from "@/hooks/use-now"
import { formatDateTime, formatDuration, formatRelative } from "@/lib/format"
import { useStore } from "@/lib/store-context"

export function UserDetailSheet({
  userId,
  onOpenChange,
}: {
  userId: string | null
  onOpenChange: (open: boolean) => void
}) {
  const { users, events, routerById } = useStore()
  const now = useNow(5_000)
  const user = userId ? users.find((u) => u.id === userId) : undefined
  const router = user ? routerById.get(user.routerId) : undefined
  const history = user
    ? events
        .filter((e) => e.routerId === user.routerId && e.username === user.username)
        .slice(0, 100)
    : []

  const fields: [string, React.ReactNode][] = user
    ? [
        ["Router", router?.name ?? "-"],
        ["Profil", user.profile],
        ["IP", user.address ?? "-"],
        ["MAC (Caller ID)", user.callerId ?? "-"],
        [
          "Uptime",
          user.connectedAt ? formatDuration(now - user.connectedAt) : "-",
        ],
        [
          "Terakhir terlihat",
          user.lastSeenAt ? formatRelative(user.lastSeenAt, now) : "-",
        ],
        ["Komentar", user.comment ?? "-"],
      ]
    : []

  return (
    <Sheet open={!!user} onOpenChange={onOpenChange}>
      <SheetContent className="w-full gap-0 sm:max-w-md">
        {user && (
          <>
            <SheetHeader className="border-b">
              <SheetTitle className="font-mono text-base break-all">
                {user.username}
              </SheetTitle>
              <SheetDescription asChild>
                <div>
                  <UserStatusBadge status={user.status} />
                </div>
              </SheetDescription>
            </SheetHeader>
            <ScrollArea className="min-h-0 flex-1">
              <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1.5 p-3 text-[13px]">
                {fields.map(([k, v]) => (
                  <div key={k} className="contents">
                    <dt className="text-muted-foreground">{k}</dt>
                    <dd className="min-w-0 font-mono text-[13px] break-all">{v}</dd>
                  </div>
                ))}
              </dl>
              <div className="border-t p-3">
                <h3 className="mb-3 text-sm font-medium">
                  Riwayat event{" "}
                  <span className="font-normal text-muted-foreground">
                    ({history.length})
                  </span>
                </h3>
                {history.length === 0 ? (
                  <p className="text-sm text-muted-foreground">
                    Belum ada event untuk user ini.
                  </p>
                ) : (
                  <ol className="space-y-2">
                    {history.map((e) => (
                      <li key={e.id} className="flex gap-2.5 text-[13px]">
                        <EventIcon type={e.type} className="mt-0.5" />
                        <div className="min-w-0 flex-1">
                          <div className="flex flex-wrap items-baseline justify-between gap-x-2">
                            <span className="font-medium">
                              {EVENT_META[e.type].label}
                            </span>
                            <time
                              className="text-xs text-muted-foreground tabular-nums"
                              dateTime={new Date(e.at).toISOString()}
                              title={formatDateTime(e.at)}
                            >
                              {formatRelative(e.at, now)}
                            </time>
                          </div>
                          <p className="font-mono text-xs break-all text-muted-foreground">
                            {e.message}
                          </p>
                        </div>
                      </li>
                    ))}
                  </ol>
                )}
              </div>
            </ScrollArea>
          </>
        )}
      </SheetContent>
    </Sheet>
  )
}
