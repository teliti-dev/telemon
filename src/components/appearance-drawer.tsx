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
import { useIsDesktop } from "@/hooks/use-media-query"
import { useApi } from "@/lib/api-context"
import { setLang, tk, useLang, useT, type Lang } from "@/lib/i18n"
import { getScale, SCALES, setScale, type Scale } from "@/lib/ui-scale"
import { navigate } from "@/lib/route"
import { cn } from "@/lib/utils"
import * as React from "react"
import { SearchSourceToggles } from "@/components/command-palette"

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
  { value: "light", label: tk("Light mode"), icon: SunIcon },
  { value: "dark", label: tk("Dark mode"), icon: MoonIcon },
  { value: "system", label: tk("By system"), icon: MonitorIcon },
]

/** Nama bahasa selalu dalam bahasanya sendiri. */
const LANGS: Option<Lang>[] = [
  { value: "id", label: "Bahasa Indonesia" }, // i18n-ignore: nama bahasa
  { value: "en", label: "English" }, // i18n-ignore: nama bahasa
]

/** Isi pengaturan tampilan: tema, bahasa, ukuran, dan (mode contoh) pratinjau akses. Dipakai drawer dan halaman. */
export function AppearanceBody() {
  const t = useT()
  const lang = useLang()
  const { theme, setTheme } = useTheme()
  const { api, access } = useApi()
  const [scale, setScaleState] = React.useState<Scale>(getScale)

  return (
    <div className="grid gap-6">
      <section>
        <h3 className="mb-2 text-sm font-medium">{t("Tema")}</h3>
        <ChoiceGroup
          label={t("Tema")}
          value={theme as Theme}
          options={THEMES.map((o) => ({ ...o, label: t(o.label) /* i18n-ignore: kunci tk */ }))}
          onChange={setTheme}
          columns={3}
        />
      </section>

      <section>
        <h3 className="mb-2 text-sm font-medium">{t("Bahasa")}</h3>
        <ChoiceGroup label={t("Bahasa")} value={lang} options={LANGS} onChange={setLang} columns={2} />
      </section>

      <section>
        <h3 className="mb-2 text-sm font-medium">{t("Ukuran tampilan")}</h3>
        <ChoiceGroup
          label={t("Ukuran tampilan")}
          value={scale}
          options={SCALES.map((s) => ({ value: s.value, label: t(s.label) /* i18n-ignore: kunci tk */, hint: `${s.value}%` }))}
          onChange={(v) => {
            setScaleState(v)
            setScale(v)
          }}
          columns={2}
        />
        <p className="mt-2 text-xs text-muted-foreground">{t("Menskalakan seluruh tampilan, bukan hanya teks.")}</p>
      </section>

      <section>
        <h3 className="mb-2 text-sm font-medium">{t("Pencarian (Ctrl+K)")}</h3>
        <SearchSourceToggles />
        <p className="mt-2 text-xs text-muted-foreground">{t("Pilih apa saja yang dicari. Bawaan: hanya menu. Pilihan ini hanya berlaku di browser atau aplikasi ini.")}</p>
      </section>

      {api.kind === "mock" && (
        <section className="rounded-xl border border-dashed p-3">
          <h3 className="mb-1 text-sm font-medium">{t("Pratinjau data contoh")}</h3>
          <p className="mb-2 text-xs text-muted-foreground">
            {t("Hanya ada di mode data contoh. Meniru aplikasi desktop (tanpa login) atau akses lewat browser (login, izin mengikuti peran).")}
          </p>
          <ChoiceGroup<Access>
            label={t("Pratinjau akses")}
            value={access}
            options={[
              { value: "desktop", label: t("Aplikasi desktop"), hint: t("tanpa login") },
              { value: "remote", label: t("Jarak jauh"), hint: t("login, sesuai peran") },
            ]}
            onChange={(v) => v !== access && setPreviewAccess(v)}
            columns={2}
          />
        </section>
      )}
    </div>
  )
}

/**
 * Desktop/tablet: drawer di kanan. HP: tombolnya membuka halaman Appearance (bukan drawer).
 * `iconOnly` (halaman login) selalu memakai drawer.
 */
export function AppearanceDrawer({ iconOnly, compact }: { iconOnly?: boolean; compact?: boolean }) {
  const t = useT()
  const isDesktop = useIsDesktop()

  const trigger = (onClick?: () => void) => (
    <Button
      variant="ghost"
      size={iconOnly ? "icon" : "sm"}
      className={cn(iconOnly ? undefined : "gap-1.5 px-2", compact && "size-6 md:size-6 [&_svg]:size-3.5")}
      aria-label={t("Appearance")}
      onClick={onClick}
    >
      <PaletteIcon />
      {!iconOnly && <span className="hidden sm:inline">{t("Appearance")}</span>}
    </Button>
  )

  if (!isDesktop && !iconOnly) return trigger(() => navigate("appearance"))

  return (
    <Sheet>
      <SheetTrigger asChild>{trigger()}</SheetTrigger>
      <SheetContent className="gap-0 data-[side=right]:w-full pt-[env(safe-area-inset-top)] pb-[env(safe-area-inset-bottom)]">
        <SheetHeader className="border-b px-4 py-3 pr-12">
          <SheetTitle>{t("Appearance")}</SheetTitle>
          <SheetDescription>{t("Tema, bahasa, dan ukuran tampilan. Pilihan disimpan di browser ini.")}</SheetDescription>
        </SheetHeader>
        <div className="overflow-y-auto p-4">
          <AppearanceBody />
        </div>
      </SheetContent>
    </Sheet>
  )
}
