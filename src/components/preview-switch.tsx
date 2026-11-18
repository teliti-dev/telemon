import { MonitorIcon, SmartphoneIcon } from "lucide-react"

import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group"
import { setPreviewAccess, type Access } from "@/lib/access"
import { useStore } from "@/lib/store-context"

/** Hanya saat data contoh: pindah tampilan antara aplikasi desktop dan akses jarak jauh (login). */
export function PreviewSwitch() {
  const { api, access } = useStore()
  if (api.kind !== "mock") return null
  return (
    <ToggleGroup
      type="single"
      variant="outline"
      value={access}
      onValueChange={(v) => v && v !== access && setPreviewAccess(v as Access)}
      aria-label="Pratinjau tampilan"
      className="hidden sm:flex"
    >
      <ToggleGroupItem value="desktop" aria-label="Pratinjau aplikasi desktop" title="Pratinjau: aplikasi desktop" className="px-2">
        <MonitorIcon />
      </ToggleGroupItem>
      <ToggleGroupItem value="remote" aria-label="Pratinjau akses jarak jauh" title="Pratinjau: akses jarak jauh (login)" className="px-2">
        <SmartphoneIcon />
      </ToggleGroupItem>
    </ToggleGroup>
  )
}
