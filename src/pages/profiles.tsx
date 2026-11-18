import * as React from "react"
import {
  DownloadIcon,
  MoreVerticalIcon,
  PencilIcon,
  PlusIcon,
  RotateCcwIcon,
  SearchIcon,
  Trash2Icon,
} from "lucide-react"
import { toast } from "sonner"

import { ConfirmDialog } from "@/components/confirm-dialog"
import { ImportDialog } from "@/components/import-dialog"
import { ONLY_ONE_LABEL, ProfileFormDialog } from "@/components/profile-form-dialog"
import { UsersSectionNav } from "@/components/users-section-nav"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Card } from "@/components/ui/card"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import { Input } from "@/components/ui/input"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table"
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group"
import { useUpdateGate, useWriteGate } from "@/hooks/use-write-gate"
import { formatNumber } from "@/lib/format"
import { navigate } from "@/lib/route"
import { useStore } from "@/lib/store-context"
import { cn } from "@/lib/utils"
import type { PppProfile } from "@/lib/types"

type View = "active" | "deleted"

export function ProfilesPage({ params }: { params: URLSearchParams }) {
  const { profiles, liveUsers, routers, routerById, api } = useStore()
  const gate = useWriteGate("profile")
  const profileReason = useUpdateGate("profile")()
  const importReason = useUpdateGate("router")()

  const [query, setQuery] = React.useState(params.get("q") ?? "")
  const [routerId, setRouterId] = React.useState(params.get("router") ?? "all")
  const [view, setView] = React.useState<View>("active")
  const [form, setForm] = React.useState<{ mode: "create" | "edit"; profile?: PppProfile } | null>(null)
  const [importing, setImporting] = React.useState(false)
  const [removing, setRemoving] = React.useState<PppProfile | null>(null)

  const usage = React.useMemo(() => {
    const m = new Map<string, { total: number; active: number }>()
    for (const u of liveUsers) {
      const k = `${u.routerId}:${u.profile}`
      const c = m.get(k) ?? { total: 0, active: 0 }
      c.total++
      if (u.status === "active") c.active++
      m.set(k, c)
    }
    return m
  }, [liveUsers])

  const filtered = React.useMemo(() => {
    const q = query.trim().toLowerCase()
    return profiles
      .filter(
        (p) =>
          (view === "deleted" ? !!p.deletedAt : !p.deletedAt) &&
          (routerId === "all" || p.routerId === routerId) &&
          (!q ||
            p.name.toLowerCase().includes(q) ||
            p.rateLimit?.toLowerCase().includes(q) ||
            p.comment?.toLowerCase().includes(q))
      )
      .sort((a, b) => a.name.localeCompare(b.name, "id", { numeric: true }) || a.routerId.localeCompare(b.routerId))
  }, [profiles, query, routerId, view])

  const canWriteAny = !profileReason && routers.some((r) => r.canWrite)
  const deletedCount = profiles.filter((p) => !!p.deletedAt).length

  async function restore(p: PppProfile) {
    try {
      await api.restoreProfile(p.id)
      toast.success(`Profil PPP ${p.name} dipulihkan`)
    } catch (e) {
      toast.error(e instanceof Error ? e.message : String(e))
    }
  }

  function RowActions({ p }: { p: PppProfile }) {
    const reason = gate(p.routerId)
    const used = usage.get(p.id)?.total ?? 0
    return (
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <Button variant="ghost" size="icon-sm" aria-label={`Aksi ${p.name}`} onClick={(e) => e.stopPropagation()}>
            <MoreVerticalIcon />
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end" onClick={(e) => e.stopPropagation()}>
          {p.deletedAt ? (
            <DropdownMenuItem onSelect={() => restore(p)}>
              <RotateCcwIcon /> Pulihkan
            </DropdownMenuItem>
          ) : (
            <>
              <DropdownMenuItem disabled={!!reason} onSelect={() => setForm({ mode: "edit", profile: p })}>
                <PencilIcon /> Edit
              </DropdownMenuItem>
              <DropdownMenuSeparator />
              <DropdownMenuItem variant="destructive" disabled={used > 0} onSelect={() => setRemoving(p)}>
                <Trash2Icon /> {used > 0 ? `Dipakai ${used} user` : "Hapus (soft)"}
              </DropdownMenuItem>
            </>
          )}
          {reason && !p.deletedAt && (
            <p className="max-w-56 px-2 py-1.5 text-xs text-muted-foreground">{reason}.</p>
          )}
        </DropdownMenuContent>
      </DropdownMenu>
    )
  }

  return (
    <div className="flex flex-col gap-3">
      <div className="flex flex-wrap items-center gap-2">
        <UsersSectionNav page="profile" />
        <div className="ml-auto flex gap-2">
          <Button variant="outline" onClick={() => setImporting(true)} disabled={!!importReason} title={importReason ?? undefined}>
            <DownloadIcon />
            <span className="hidden sm:inline">Import dari Mikrotik</span>
            <span className="sm:hidden">Import</span>
          </Button>
          <Button
            onClick={() => setForm({ mode: "create" })}
            disabled={!canWriteAny}
            title={canWriteAny ? undefined : (profileReason ?? "Tidak ada router dengan hak write")}
          >
            <PlusIcon />
            <span className="hidden sm:inline">Tambah profil PPP</span>
            <span className="sm:hidden">Tambah</span>
          </Button>
        </div>
      </div>

      <div className="flex flex-col gap-2 sm:flex-row sm:items-center">
        <div className="relative sm:w-72">
          <SearchIcon className="pointer-events-none absolute top-1/2 left-2.5 size-4 -translate-y-1/2 text-muted-foreground" />
          <Input
            type="search"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Cari nama, rate limit…"
            aria-label="Cari profil PPP"
            className="pl-8"
          />
        </div>
        <div className="flex gap-2 sm:ml-auto">
          <Select value={routerId} onValueChange={setRouterId}>
            <SelectTrigger className="min-w-0 flex-1 sm:w-44 sm:flex-none" aria-label="Router">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="all">Semua router</SelectItem>
              {routers.map((r) => (
                <SelectItem key={r.id} value={r.id}>
                  {r.name}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          <ToggleGroup
            type="single"
            variant="outline"
            value={view}
            onValueChange={(v) => v && setView(v as View)}
            aria-label="Tampilkan"
            className="shrink-0"
          >
            <ToggleGroupItem value="active" className="px-3">
              Aktif
            </ToggleGroupItem>
            <ToggleGroupItem value="deleted" className="gap-1.5 px-3">
              Terhapus
              {deletedCount > 0 && <span className="text-xs text-muted-foreground tabular-nums">{deletedCount}</span>}
            </ToggleGroupItem>
          </ToggleGroup>
        </div>
      </div>

      <p className="-mb-1 text-xs text-muted-foreground" aria-live="polite">
        {formatNumber(filtered.length)} profil PPP
      </p>

      {filtered.length === 0 ? (
        <Card className="items-center py-12 text-sm text-muted-foreground">
          {view === "deleted" ? "Tidak ada profil PPP terhapus." : "Tidak ada profil PPP yang cocok dengan filter."}
        </Card>
      ) : (
        <>
          {/* Desktop: tabel */}
          <Card className="hidden py-0 md:block">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead className="pl-3">Nama</TableHead>
                  <TableHead>Router</TableHead>
                  <TableHead>Rate limit</TableHead>
                  <TableHead className="hidden lg:table-cell">Local address</TableHead>
                  <TableHead className="hidden lg:table-cell">Remote / pool</TableHead>
                  <TableHead className="hidden xl:table-cell">Only one</TableHead>
                  <TableHead>User</TableHead>
                  <TableHead className="w-10 pr-3" />
                </TableRow>
              </TableHeader>
              <TableBody>
                {filtered.map((p) => {
                  const u = usage.get(p.id)
                  return (
                    <TableRow key={p.id} className={cn(p.deletedAt && "opacity-60")}>
                      <TableCell className="pl-3">
                        <span className={cn("font-mono text-[13px]", p.deletedAt && "line-through")}>{p.name}</span>
                        {p.comment && <span className="ml-1.5 text-xs text-muted-foreground">· {p.comment}</span>}
                      </TableCell>
                      <TableCell>{routerById.get(p.routerId)?.name}</TableCell>
                      <TableCell className="font-mono text-[13px]">{p.rateLimit ?? "-"}</TableCell>
                      <TableCell className="hidden font-mono text-[13px] lg:table-cell">{p.localAddress ?? "-"}</TableCell>
                      <TableCell className="hidden font-mono text-[13px] lg:table-cell">{p.remoteAddress ?? "-"}</TableCell>
                      <TableCell className="hidden text-muted-foreground xl:table-cell">{ONLY_ONE_LABEL[p.onlyOne].split(" (")[0]}</TableCell>
                      <TableCell className="tabular-nums">
                        <button
                          type="button"
                          className="hover:underline disabled:no-underline"
                          disabled={!u}
                          onClick={() => navigate("user", { router: p.routerId, q: p.name })}
                          title="Lihat user dengan profil PPP ini"
                        >
                          {u ? `${u.active}/${u.total}` : "0"}
                        </button>
                        <span className="ml-1 text-xs text-muted-foreground">aktif</span>
                      </TableCell>
                      <TableCell className="pr-3">
                        <RowActions p={p} />
                      </TableCell>
                    </TableRow>
                  )
                })}
              </TableBody>
            </Table>
          </Card>

          {/* HP: daftar kartu */}
          <ul className="flex flex-col gap-1.5 md:hidden">
            {filtered.map((p) => {
              const u = usage.get(p.id)
              return (
                <li
                  key={p.id}
                  className={cn("rounded-lg border bg-card px-3 py-2 text-sm", p.deletedAt && "border-dashed opacity-60")}
                >
                  <div className="flex items-start justify-between gap-2">
                    <div className="min-w-0">
                      <div className={cn("truncate font-mono text-[13px] font-medium", p.deletedAt && "line-through")}>
                        {p.name}
                      </div>
                      <div className="truncate text-xs text-muted-foreground">
                        {routerById.get(p.routerId)?.name}
                        {p.comment && ` · ${p.comment}`}
                      </div>
                    </div>
                    <RowActions p={p} />
                  </div>
                  <div className="mt-1.5 flex flex-wrap items-center gap-x-3 gap-y-1 text-xs">
                    <Badge variant="outline" className="font-mono">{p.rateLimit ?? "tanpa batas"}</Badge>
                    <span className="text-muted-foreground">
                      {u ? `${u.active}/${u.total}` : "0"} user aktif
                    </span>
                    {p.remoteAddress && <span className="font-mono text-muted-foreground">{p.remoteAddress}</span>}
                  </div>
                </li>
              )
            })}
          </ul>
        </>
      )}

      <ProfileFormDialog
        open={!!form}
        onOpenChange={(o) => !o && setForm(null)}
        mode={form?.mode ?? "create"}
        profile={form?.profile}
        defaultRouterId={routerId !== "all" ? routerId : undefined}
      />
      <ImportDialog
        open={importing}
        onOpenChange={setImporting}
        defaultRouterId={routerId !== "all" ? routerId : undefined}
      />
      <ConfirmDialog
        open={!!removing}
        onOpenChange={(o) => !o && setRemoving(null)}
        title={`Hapus profil PPP ${removing?.name}?`}
        description={
          <p>
            Soft delete: profil PPP disembunyikan dari pilihan di Telemon dan bisa dipulihkan.
            Profil PPP di router <b>{routerById.get(removing?.routerId ?? "")?.name}</b> tidak dihapus.
          </p>
        }
        confirmLabel="Hapus"
        destructive
        onConfirm={async () => {
          if (removing) {
            await api.softDeleteProfile(removing.id)
            toast.success(`Profil PPP ${removing.name} dihapus (soft)`)
          }
        }}
      />
    </div>
  )
}
