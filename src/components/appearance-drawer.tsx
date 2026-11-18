import { MonitorIcon, MoonIcon, PaletteIcon, SunIcon, type LucideIcon } from "lucide-react"

import { useTheme } from "@/components/theme-provider"
import { Button } from "@/components/ui/button"
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
  SheetTrigger,
} from "@/components/ui/sheet"
import { setPreviewAccess, type Access } from "@/lib/access"
import { useApi } from "@/lib/api-context"
import { getScale, SCALES, setScale, type Scale } from "@/lib/ui-scale"
import { cn } from "@/lib/utils"
import * as React from "react"

type Theme = "light" | "dark" | "system"

interface Option<T extends string> {
  value: T
  label: string
  hint?: string
  icon?: LucideIcon
}

function ChoiceGroup<T extends string>({
  label,
  value,
  options,
  onChange,
  columns,
}: {
  label: string
  value: T
  options: Option<T>[]
  onChange: (v: T) => void
  columns: 2 | 3
}) {
  return (
    <div role="radiogroup" aria-label={label} className={cn("grid gap-2", columns === 3 ? "grid-cols-3" : "grid-cols-2")}>
      {options.map((o) => {
        const on = o.value === value
        return (
          <button
            key={o.value}
            type="button"
            role="radio"
            aria-checked={on}
            onClick={() => onChange(o.value)}
            className={cn(
              "flex flex-col items-center gap-1 rounded-xl border px-2 py-3 text-center text-sm outline-none transition-colors focus-visible:ring-3 focus-visible:ring-ring/50",
              on ? "font-medium text-brand" : "hover:bg-muted/60"
            )}
          >
            {o.icon && <o.icon className="mb-0.5 size-5" />}
            <span>{o.label}</span>
            {o.hint && <span className="text-xs font-normal text-muted-foreground">{o.hint}</span>}
          </button>
        )
      })}
    </div>
  )
}

const THEMES: Option<Theme>[] = [
  { value: "light", label: "Light mode", icon: SunIcon },
  { value: "dark", label: "Dark mode", icon: MoonIcon },
  { value: "system", label: "By system", icon: MonitorIcon },
]

/** Drawer di kanan: tema (terang, gelap, ikuti sistem) dan ukuran tampilan. */
export function AppearanceDrawer({ iconOnly }: { iconOnly?: boolean }) {
  const { theme, setTheme } = useTheme()
  const { api, access } = useApi()
  const [scale, setScaleState] = React.useState<Scale>(getScale)

  return (
    <Sheet>
      <SheetTrigger asChild>
        <Button variant="ghost" size={iconOnly ? "icon" : "sm"} className={iconOnly ? undefined : "gap-1.5 px-2"} aria-label="Appearance">
          <PaletteIcon />
          {!iconOnly && <span className="hidden sm:inline">Appearance</span>}
        </Button>
      </SheetTrigger>
      <SheetContent className="gap-0 data-[side=right]:w-full pt-[env(safe-area-inset-top)] pb-[env(safe-area-inset-bottom)]">
        <SheetHeader className="border-b px-4 py-3 pr-12">
          <SheetTitle>Appearance</SheetTitle>
          <SheetDescription>Tema dan ukuran tampilan. Pilihan disimpan di browser ini.</SheetDescription>
        </SheetHeader>

        <div className="grid gap-6 overflow-y-auto p-4">
          <section>
            <h3 className="mb-2 text-sm font-medium">Tema</h3>
            <ChoiceGroup label="Tema" value={theme as Theme} options={THEMES} onChange={setTheme} columns={3} />
          </section>

          <section>
            <h3 className="mb-2 text-sm font-medium">Ukuran tampilan</h3>
            <ChoiceGroup
              label="Ukuran tampilan"
              value={scale}
              options={SCALES.map((s) => ({ value: s.value, label: s.label, hint: `${s.value}%` }))}
              onChange={(v) => {
                setScaleState(v)
                setScale(v)
              }}
              columns={2}
            />
            <p className="mt-2 text-xs text-muted-foreground">Menskalakan seluruh tampilan, bukan hanya teks.</p>
          </section>

          {api.kind === "mock" && (
            <section className="rounded-xl border border-dashed p-3">
              <h3 className="mb-1 text-sm font-medium">Pratinjau data contoh</h3>
              <p className="mb-2 text-xs text-muted-foreground">
                Hanya ada di mode data contoh. Meniru aplikasi desktop (tanpa login) atau akses lewat browser (login, hanya baca).
              </p>
              <ChoiceGroup<Access>
                label="Pratinjau akses"
                value={access}
                options={[
                  { value: "desktop", label: "Aplikasi desktop", hint: "tanpa login" },
                  { value: "remote", label: "Jarak jauh", hint: "login, hanya baca" },
                ]}
                onChange={(v) => v !== access && setPreviewAccess(v)}
                columns={2}
              />
            </section>
          )}
        </div>
      </SheetContent>
    </Sheet>
  )
}
