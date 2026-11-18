import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group"
import { navigate, type Page } from "@/lib/route"
import { useStore } from "@/lib/store-context"

/** Users dan Profile satu menu sidebar; pindah lewat segmen ini. */
export function UsersSectionNav({ page }: { page: Extract<Page, "user" | "profile"> }) {
  const { liveUsers, profiles } = useStore()
  const nProfiles = profiles.filter((p) => !p.deletedAt).length
  return (
    <ToggleGroup
      type="single"
      variant="outline"
      value={page}
      onValueChange={(v) => v && navigate(v as "user" | "profile")}
      aria-label="Bagian"
    >
      <ToggleGroupItem value="user" className="gap-1.5 px-3">
        Users <span className="text-xs text-muted-foreground tabular-nums">{liveUsers.length}</span>
      </ToggleGroupItem>
      <ToggleGroupItem value="profile" className="gap-1.5 px-3">
        Profile <span className="text-xs text-muted-foreground tabular-nums">{nProfiles}</span>
      </ToggleGroupItem>
    </ToggleGroup>
  )
}
