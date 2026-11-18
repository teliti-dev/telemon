import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group"
import { LEVEL_LABEL, RESOURCES } from "@/lib/permissions"
import type { Level, Permissions } from "@/lib/types"

const LEVELS: Level[] = ["none", "read", "update"]

/** Izin per menu: Tanpa akses, Hanya baca, atau Ubah (bila menunya mendukung). */
export function PermissionMatrix({
  value,
  onChange,
}: {
  value: Permissions
  onChange: (p: Permissions) => void
}) {
  return (
    <div className="divide-y rounded-lg border">
      {RESOURCES.map((r) => (
        <div key={r.key} className="flex flex-wrap items-center justify-between gap-x-3 gap-y-2 px-3 py-2">
          <div className="min-w-0">
            <div className="text-sm font-medium">{r.label}</div>
            <div className="max-w-64 text-xs text-muted-foreground">{r.hint}</div>
          </div>
          <ToggleGroup
            type="single"
            variant="outline"
            value={value[r.key]}
            onValueChange={(v) => v && onChange({ ...value, [r.key]: v as Level })}
            aria-label={`Izin ${r.label}`}
          >
            {LEVELS.map((l) => (
              <ToggleGroupItem key={l} value={l} disabled={!r.levels.includes(l)} className="px-2.5">
                {LEVEL_LABEL[l]}
              </ToggleGroupItem>
            ))}
          </ToggleGroup>
        </div>
      ))}
    </div>
  )
}
