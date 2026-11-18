import * as React from "react"
import { MoreVerticalIcon, PencilIcon, PlusIcon, Trash2Icon } from "lucide-react"
import { toast } from "sonner"

import { ConfirmDialog } from "@/components/confirm-dialog"
import { RoleDialog } from "@/components/role-dialog"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Card, CardAction, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table"
import { useApi } from "@/lib/api-context"
import { LEVEL_SHORT, RESOURCES } from "@/lib/permissions"
import type { Level, Role } from "@/lib/types"
import { cn } from "@/lib/utils"

const errMsg = (e: unknown) => (e instanceof Error ? e.message : String(e))

function LevelCell({ level }: { level: Level }) {
  return (
    <span
      className={cn(
        level === "update" && "font-medium text-foreground",
        level === "read" && "text-muted-foreground",
        level === "none" && "text-muted-foreground/50"
      )}
    >
      {LEVEL_SHORT[level]}
    </span>
  )
}

/** Peran dan izin per menu dalam satu tabel. Peran bawaan tidak bisa dihapus. */
export function RolesCard() {
  const { api } = useApi()
  const [roles, setRoles] = React.useState<Role[] | null>(null)
  const [dialog, setDialog] = React.useState<{ role?: Role } | null>(null)
  const [removing, setRemoving] = React.useState<Role | null>(null)

  const load = React.useCallback(() => {
    api
      .listRoles()
      .then(setRoles)
      .catch((e: unknown) => toast.error(errMsg(e)))
  }, [api])

  React.useEffect(() => {
    load()
  }, [load])

  return (
    <Card>
      <CardHeader>
        <CardTitle>Peran dan izin</CardTitle>
        <CardDescription>
          Izin per menu untuk akun yang masuk lewat browser. Bawaan tanpa akses; centang Lihat atau Ubah per menu. Aplikasi desktop tidak
          dibatasi peran.
        </CardDescription>
        <CardAction>
          <Button onClick={() => setDialog({})}>
            <PlusIcon /> <span className="hidden sm:inline">Tambah peran</span>
            <span className="sm:hidden">Tambah</span>
          </Button>
        </CardAction>
      </CardHeader>
      <CardContent className="px-0">
        {roles === null ? (
          <p className="px-4 py-3 text-sm text-muted-foreground">Memuat…</p>
        ) : (
          <div className="overflow-x-auto">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead className="pl-4">Peran</TableHead>
                  {RESOURCES.map((r) => (
                    <TableHead key={r.key}>{r.label}</TableHead>
                  ))}
                  <TableHead>Akun</TableHead>
                  <TableHead className="w-10 pr-3" />
                </TableRow>
              </TableHeader>
              <TableBody>
                {roles.map((role) => (
                  <TableRow key={role.id}>
                    <TableCell className="pl-4">
                      <div className="flex items-center gap-2">
                        <span className="font-medium">{role.name}</span>
                        {role.builtin && <Badge variant="secondary">Bawaan</Badge>}
                      </div>
                      {role.description && <div className="max-w-64 truncate text-xs text-muted-foreground">{role.description}</div>}
                    </TableCell>
                    {RESOURCES.map((r) => (
                      <TableCell key={r.key}>
                        <LevelCell level={role.permissions[r.key]} />
                      </TableCell>
                    ))}
                    <TableCell className="tabular-nums">{role.accountCount}</TableCell>
                    <TableCell className="pr-3">
                      <DropdownMenu>
                        <DropdownMenuTrigger asChild>
                          <Button variant="ghost" size="icon-sm" aria-label={`Aksi ${role.name}`}>
                            <MoreVerticalIcon />
                          </Button>
                        </DropdownMenuTrigger>
                        <DropdownMenuContent align="end">
                          <DropdownMenuItem onSelect={() => setDialog({ role })}>
                            <PencilIcon /> Edit
                          </DropdownMenuItem>
                          <DropdownMenuSeparator />
                          <DropdownMenuItem
                            variant="destructive"
                            disabled={role.builtin || role.accountCount > 0}
                            onSelect={() => setRemoving(role)}
                          >
                            <Trash2Icon />{" "}
                            {role.builtin ? "Bawaan" : role.accountCount > 0 ? `Dipakai ${role.accountCount} akun` : "Hapus"}
                          </DropdownMenuItem>
                        </DropdownMenuContent>
                      </DropdownMenu>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </div>
        )}
      </CardContent>

      <RoleDialog open={!!dialog} onOpenChange={(o) => !o && setDialog(null)} role={dialog?.role} onDone={load} />
      <ConfirmDialog
        open={!!removing}
        onOpenChange={(o) => !o && setRemoving(null)}
        title={`Hapus peran ${removing?.name}?`}
        description={<p>Peran dihapus permanen. Akun tidak terpengaruh karena tidak ada yang memakai peran ini.</p>}
        confirmLabel="Hapus"
        destructive
        onConfirm={async () => {
          if (removing) {
            await api.deleteRole(removing.id)
            toast.success("Peran dihapus")
            load()
          }
        }}
      />
    </Card>
  )
}
