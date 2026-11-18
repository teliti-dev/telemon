import { Checkbox } from "@/components/ui/checkbox"
import { RESOURCES } from "@/lib/permissions"
import type { Permissions, Resource } from "@/lib/types"

const GRID = "grid grid-cols-[minmax(0,1fr)_3.5rem_3.5rem] items-center gap-x-3"

/**
 * Izin per menu. Bawaan tanpa akses: yang dicentang yang diberikan.
 * Ubah otomatis mencakup Lihat. Menu yang tidak punya fitur ubah hanya punya Lihat.
 */
export function PermissionMatrix({
  value,
  onChange,
}: {
  value: Permissions
  onChange: (p: Permissions) => void
}) {
  const set = (key: Resource, level: Permissions[Resource]) => onChange({ ...value, [key]: level })
  return (
    <div className="divide-y rounded-lg border">
      <div className={`${GRID} px-3 py-1.5 text-xs text-muted-foreground`}>
        <span>Menu</span>
        <span className="text-center">Lihat</span>
        <span className="text-center">Ubah</span>
      </div>
      {RESOURCES.map((r) => {
        const level = value[r.key]
        const canUpdate = r.levels.includes("update")
        return (
          <div key={r.key} className={`${GRID} px-3 py-2`}>
            <div className="min-w-0">
              <div className="text-sm font-medium">{r.label}</div>
              <div className="text-xs text-muted-foreground">{r.hint}</div>
            </div>
            <div className="flex justify-center">
              <Checkbox
                checked={level !== "none"}
                onCheckedChange={(c) => set(r.key, c === true ? "read" : "none")}
                aria-label={`Lihat ${r.label}`}
              />
            </div>
            <div className="flex justify-center">
              {canUpdate && (
                <Checkbox
                  checked={level === "update"}
                  onCheckedChange={(c) => set(r.key, c === true ? "update" : "read")}
                  aria-label={`Ubah ${r.label}`}
                />
              )}
            </div>
          </div>
        )
      })}
    </div>
  )
}
