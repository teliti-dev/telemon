import * as React from "react"
import {
  LockIcon,
  LockOpenIcon,
  MoreVerticalIcon,
  PlusIcon,
  TriangleAlertIcon,
  UsersIcon,
  Trash2Icon,
  DownloadIcon,
  PencilIcon,
  PuzzleIcon,
} from "lucide-react"
import { toast } from "sonner"

import { RouterDialog } from "@/components/router-dialog"
import { ImportDialog } from "@/components/import-dialog"
import { RouterStatusBadge, UserMixBar } from "@/components/status"
import { Alert, AlertDescription } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import {
  Card,
  CardAction,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card"
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { ActionMenu, ActionMenuContent, ActionMenuItem, ActionMenuSeparator, ActionMenuTrigger } from "@/components/action-menu"
import { useNow } from "@/hooks/use-now"
import { formatRelative } from "@/lib/format"
import { useUpdateGate } from "@/hooks/use-write-gate"
import { useT } from "@/lib/i18n"
import { useIsDesktop } from "@/hooks/use-media-query"
import { routerActionList, useExtensionState } from "@/lib/extensions/registry"
import { navigate, navigatePath } from "@/lib/route"
import { useStore } from "@/lib/store-context"
import type { RouterInfo } from "@/lib/types"

export function RoutersPage() {
  // Aksi dari extension (mis. Terminal SSH): hanya yang boleh dipakai peran ini; diperbarui saat extension dimuat ulang.
  useExtensionState()
  const extActions = routerActionList().filter((a) => !a.action || a.can(a.action))
  const { routers, liveUsers: users, api } = useStore()
  const t = useT()
  const isDesktop = useIsDesktop()
  const importReason = useUpdateGate("router")()
  const [editing, setEditing] = React.useState<RouterInfo | null>(null)
  const now = useNow(10_000)
  const [adding, setAdding] = React.useState(false)
  const [removing, setRemoving] = React.useState<RouterInfo | null>(null)
  const [importFor, setImportFor] = React.useState<RouterInfo | null>(null)

  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-center justify-between gap-3">
        <p className="text-sm text-muted-foreground">
          {t("Mikrotik adalah source of truth; Telemon membaca dan (bila diizinkan) menulis lewat RouterOS API.")}
        </p>
        <Button onClick={() => (isDesktop ? setAdding(true) : navigatePath("router", ["new"]))} disabled={!!importReason} title={importReason ?? undefined}>
          <PlusIcon />
          <span className="hidden sm:inline">{t("Tambah router")}</span>
          <span className="sm:hidden">{t("Tambah")}</span>
        </Button>
      </div>

      <div className="grid gap-2 md:grid-cols-2 md:gap-3 xl:grid-cols-3">
        {routers.map((r) => {
          const mine = users.filter((u) => u.routerId === r.id)
          const c = { active: 0, inactive: 0, disabled: 0 }
          for (const u of mine) c[u.status]++
          const facts: [string, React.ReactNode][] = [
            [t("Model"), r.model],
            ["RouterOS", r.version],
            [
              t("Koneksi"),
              <span className="inline-flex items-center gap-1.5" key="tls">
                {r.tls ? <LockIcon className="size-3.5" /> : <LockOpenIcon className="size-3.5" />}
                {r.tls ? "API-SSL" : t("API tanpa TLS")}
              </span>,
            ],
            [t("Polling"), r.lastPollAt ? formatRelative(r.lastPollAt, now) : t("Belum pernah")],
            [t("Izin API"), r.canWrite ? t("Baca & tulis") : t("Hanya baca")],
            [t("Import terakhir"), r.lastImportAt ? formatRelative(r.lastImportAt, now) : t("Belum pernah")],
          ]
          return (
            <Card key={r.id} className="gap-2.5">
              <CardHeader>
                <CardTitle className="flex items-center gap-2">
                  <span className="truncate">{r.name}</span>
                  <RouterStatusBadge status={r.status} />
                </CardTitle>
                <CardDescription className="font-mono text-xs">
                  {r.host}:{r.port}
                </CardDescription>
                <CardAction>
                  <ActionMenu>
                    <ActionMenuTrigger asChild>
                      <Button variant="ghost" size="icon" aria-label={t("Aksi {name}", { name: r.name })}>
                        <MoreVerticalIcon />
                      </Button>
                    </ActionMenuTrigger>
                    <ActionMenuContent align="end">
                      <ActionMenuItem onSelect={() => navigate("user", { router: r.id })}>
                        <UsersIcon /> {t("Lihat user")}
                      </ActionMenuItem>
                      <ActionMenuItem disabled={!!importReason} onSelect={() => (isDesktop ? setEditing(r) : navigatePath("router", [r.id, "edit"]))}>
                        <PencilIcon /> {t("Edit router")}
                      </ActionMenuItem>
                      <ActionMenuItem
                        disabled={r.status !== "online" || !!importReason}
                        onSelect={() => (isDesktop ? setImportFor(r) : navigatePath("import", [], { router: r.id }))}
                      >
                        <DownloadIcon /> {t("Import dari Mikrotik")}
                      </ActionMenuItem>
                      {extActions.length > 0 && <ActionMenuSeparator />}
                      {extActions.map((a) => {
                        const ref = { id: r.id, name: r.name, canWrite: r.canWrite, status: r.status }
                        const why = a.disabled?.(ref) ?? null
                        return (
                          <ActionMenuItem key={`${a.extId}/${a.id}`} disabled={!!why} title={why ?? undefined} onSelect={() => a.run(ref)}>
                            <PuzzleIcon /> {a.title}
                          </ActionMenuItem>
                        )
                      })}
                      <ActionMenuSeparator />
                      <ActionMenuItem variant="destructive" disabled={!!importReason} onSelect={() => setRemoving(r)}>
                        <Trash2Icon /> {t("Hapus dari Telemon")}
                      </ActionMenuItem>
                    </ActionMenuContent>
                  </ActionMenu>
                </CardAction>
              </CardHeader>
              <CardContent className="space-y-2.5">
                {r.error && (
                  <Alert variant="destructive">
                    <TriangleAlertIcon />
                    <AlertDescription>{r.error}</AlertDescription>
                  </Alert>
                )}
                <dl className="grid grid-cols-2 gap-x-3 gap-y-1.5 text-[13px] sm:grid-cols-3">
                  {facts.map(([k, v]) => (
                    <div key={k} className="min-w-0">
                      <dt className="text-xs text-muted-foreground">{k}</dt>
                      <dd className="truncate">{v}</dd>
                    </div>
                  ))}
                </dl>
                <div className="space-y-1.5">
                  <UserMixBar active={c.active} inactive={c.inactive} disabled={c.disabled} />
                  <p className="text-xs text-muted-foreground">
                    <span className="font-medium text-foreground tabular-nums">{c.active}</span> {t("aktif")} ·{" "}
                    <span className="font-medium text-foreground tabular-nums">{c.inactive}</span> {t("tidak aktif")} ·{" "}
                    <span className="font-medium text-foreground tabular-nums">{c.disabled}</span> {t("dinonaktifkan")}
                  </p>
                </div>
              </CardContent>
            </Card>
          )
        })}
      </div>

      <RouterDialog open={adding} onOpenChange={setAdding} />
      <RouterDialog open={!!editing} onOpenChange={(o) => !o && setEditing(null)} router={editing ?? undefined} />
      <ImportDialog
        open={!!importFor}
        onOpenChange={(o) => !o && setImportFor(null)}
        defaultRouterId={importFor?.id}
      />

      <Dialog open={!!removing} onOpenChange={(o) => !o && setRemoving(null)}>
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle>{t("Hapus {name}?", { name: removing?.name ?? "" })}</DialogTitle>
            <DialogDescription>
              {t("Router berhenti dipantau dan data user serta event-nya dihapus dari Telemon. Konfigurasi di router tidak diubah.")}
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <DialogClose asChild>
              <Button variant="outline">{t("Batal")}</Button>
            </DialogClose>
            <Button
              variant="destructive"
              onClick={() => {
                const r = removing
                setRemoving(null)
                if (r)
                  api
                    .removeRouter(r.id)
                    .then(() => toast.success(t("{name} dihapus", { name: r.name })))
                    .catch((e: Error) => toast.error(e.message))
              }}
            >
              {t("Hapus")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  )
}
