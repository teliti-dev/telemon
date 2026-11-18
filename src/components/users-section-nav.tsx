import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group"
import { useAuth } from "@/lib/auth-context"
import { navigate, type Page } from "@/lib/route"
import { useT } from "@/lib/i18n"
import { useStore } from "@/lib/store-context"

/** Users dan Profile satu menu sidebar; pindah lewat segmen ini. */
export function UsersSectionNav({ page }: { page: Extract<Page, "user" | "profile"> }) {
  const t = useT()
  const { liveUsers, profiles } = useStore()
  const { session } = useAuth()
  const perms = session?.permissions
  // Akun jarak jauh yang hanya boleh satu bagian tidak perlu segmen.
  if (perms && (perms.user === "none" || perms.profile === "none")) return null
  const nProfiles = profiles.filter((p) => !p.deletedAt).length
  return (
    <ToggleGroup
      type="single"
      variant="outline"
      value={page}
      onValueChange={(v) => v && navigate(v as "user" | "profile")}
      aria-label={t("Bagian")}
    >
      <ToggleGroupItem value="user" className="gap-1.5 px-3">
        {t("Users")} <span className="text-xs text-muted-foreground tabular-nums">{liveUsers.length}</span>
      </ToggleGroupItem>
      <ToggleGroupItem value="profile" className="gap-1.5 px-3">
        {t("Profil PPP")} <span className="text-xs text-muted-foreground tabular-nums">{nProfiles}</span>
      </ToggleGroupItem>
    </ToggleGroup>
  )
}
