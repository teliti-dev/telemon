import * as React from "react"
import { CheckCircle2Icon, InfoIcon } from "lucide-react"

import { Alert, AlertDescription } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Label } from "@/components/ui/label"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { useNow } from "@/hooks/use-now"
import { useUpdateGate } from "@/hooks/use-write-gate"
import { formatRelative } from "@/lib/format"
import { useStore } from "@/lib/store-context"
import type { ImportCounts, ImportResult } from "@/lib/types"

interface RouterResult {
  routerId: string
  result?: ImportResult
  error?: string
}

function Counts({ label, c }: { label: string; c: ImportCounts }) {
  const parts = [
    c.added > 0 && `${c.added} baru`,
    c.updated > 0 && `${c.updated} diperbarui`,
    c.deleted > 0 && `${c.deleted} hilang dari router (ditandai dihapus)`,
  ].filter(Boolean)
  return (
    <div className="flex justify-between gap-3">
      <span className="text-muted-foreground">{label}</span>
      <span className="text-right">{parts.length ? parts.join(", ") : "tidak ada perubahan"}</span>
    </div>
  )
}

export function ImportDialog({
  open,
  onOpenChange,
  defaultRouterId,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  defaultRouterId?: string
}) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-h-[92svh] overflow-y-auto sm:max-w-md">
        {open && <ImportBody onClose={() => onOpenChange(false)} defaultRouterId={defaultRouterId} />}
      </DialogContent>
    </Dialog>
  )
}

function ImportBody({
  defaultRouterId,
  onClose,
}: {
  defaultRouterId?: string
  onClose: () => void
}) {
  const { routers, routerById, api } = useStore()
  const importReason = useUpdateGate("router")()
  const now = useNow(30_000)
  const online = routers.filter((r) => r.status === "online")
  const [target, setTarget] = React.useState(
    defaultRouterId && online.some((r) => r.id === defaultRouterId) ? defaultRouterId : "all"
  )
  const [busy, setBusy] = React.useState(false)
  const [results, setResults] = React.useState<RouterResult[] | null>(null)

  async function run() {
    const ids = target === "all" ? online.map((r) => r.id) : [target]
    setBusy(true)
    setResults(null)
    const out: RouterResult[] = []
    for (const id of ids) {
      try {
        out.push({ routerId: id, result: await api.importFromRouter(id) })
      } catch (e) {
        out.push({ routerId: id, error: e instanceof Error ? e.message : String(e) })
      }
      setResults([...out])
    }
    setBusy(false)
  }

  return (
    <>
      <DialogHeader>
        <DialogTitle>Import dari Mikrotik</DialogTitle>
        <DialogDescription>
          Membaca <code>/ppp/secret</code> dan <code>/ppp/profile</code> dari router.
        </DialogDescription>
      </DialogHeader>

      <Alert>
        <InfoIcon />
        <AlertDescription>
          <b>Router adalah source of truth.</b> Saat ada selisih, data di Telemon ditimpa
          dengan data router. User atau profile yang sudah tidak ada di router ditandai
          dihapus (soft delete), bukan dibuang. Password tidak ikut diimpor.
        </AlertDescription>
      </Alert>

      <div className="grid gap-1.5">
        <Label htmlFor="imp-router">Router</Label>
        <Select value={target} onValueChange={setTarget} disabled={busy}>
          <SelectTrigger id="imp-router" className="w-full">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="all" disabled={online.length === 0}>
              Semua router online ({online.length})
            </SelectItem>
            {routers.map((r) => (
              <SelectItem key={r.id} value={r.id} disabled={r.status !== "online"}>
                {r.name}
                {r.status !== "online"
                  ? ` (${r.status})`
                  : r.lastImportAt
                    ? ` · import ${formatRelative(r.lastImportAt, now)}`
                    : " · belum pernah diimpor"}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>

      {results && (
        <ul className="grid gap-2">
          {results.map((r) => (
            <li key={r.routerId} className="grid gap-1 rounded-lg border p-2.5 text-[13px]">
              <div className="flex items-center gap-1.5 font-medium">
                {r.result && <CheckCircle2Icon className="size-4 text-good" />}
                {routerById.get(r.routerId)?.name}
              </div>
              {r.error && <p className="text-destructive">{r.error}</p>}
              {r.result && (
                <>
                  <Counts label="User" c={r.result.users} />
                  <Counts label="Profile" c={r.result.profiles} />
                </>
              )}
            </li>
          ))}
        </ul>
      )}

      <DialogFooter>
        <Button variant="outline" onClick={onClose} disabled={busy}>
          {results && !busy ? "Tutup" : "Batal"}
        </Button>
        <Button onClick={run} disabled={busy || online.length === 0 || !!importReason}>
          {busy ? "Membaca router…" : results ? "Import lagi" : "Import"}
        </Button>
      </DialogFooter>
    </>
  )
}
