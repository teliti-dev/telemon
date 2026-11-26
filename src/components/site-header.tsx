import * as React from "react"
import { ChevronDownIcon, GlobeIcon, PuzzleIcon, SettingsIcon, SparklesIcon, WifiIcon } from "lucide-react"

import { AccountMenu } from "@/components/account-menu"
import { AppearanceDrawer } from "@/components/appearance-drawer"
import { SearchTrigger } from "@/components/command-palette"
import { LiveIndicator } from "@/components/live-indicator"
import { TelemonLogo } from "@/components/telemon-logo"
import { WindowControls } from "@/components/window-controls"
import { isNavActive, type NavItem } from "@/components/nav-items"
import { Badge } from "@/components/ui/badge"
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from "@/components/ui/dropdown-menu"
import { useNavBadges } from "@/hooks/use-nav-badges"
import { useNavItems } from "@/hooks/use-nav-items"
import { useT } from "@/lib/i18n"
import { PAGE_CONTAINER } from "@/lib/layout"
import { href, navigate, navigatePath, useRoute } from "@/lib/route"
import { useStore } from "@/lib/store-context"
import { cn } from "@/lib/utils"
import { initFrameless, MAC_TRAFFIC_LIGHTS, useFrameless, useMacOverlay, winStartDragging, winToggleMaximize } from "@/lib/window"

/**
 * Satu toolbar yang selalu menempel di atas (seperti bilah judul aplikasi desktop):
 * - Kiri: logo dan nama Telemon, lalu titik Live yang bisa diklik (popup status).
 * - Desktop (md ke atas): menu utama langsung di toolbar; kelompok kanan (Berbagi, Admin, Pengaturan, Tentang) rata kanan
 *   sebelum Cari (Ctrl+K), Appearance, dan Profile di ujung kanan. Di lebar sempit menu hanya ikon (label mulai xl/2xl).
 * - HP: menu utama ada di bawah layar; toolbar memuat judul halaman dan kontrol yang sama.
 * Toolbar juga tempat menyeret jendela bila bilah judul bawaan dihilangkan (Windows, macOS Overlay).
 */
export function SiteHeader({ title }: { title: string }) {
  const t = useT()
  const { share, access } = useStore()
  const { page } = useRoute()
  const badges = useNavBadges()
  const items = useNavItems()
  const frameless = useFrameless()
  const mac = useMacOverlay()
  // Toolbar menjadi bilah judul (Windows tanpa bilah bawaan, atau macOS Overlay): bisa diseret.
  const titlebar = frameless || mac
  React.useEffect(initFrameless, [])

  const dragProps = titlebar
    ? {
        onMouseDown: (e: React.MouseEvent) => {
          if (e.button !== 0 || e.detail > 1) return
          if ((e.target as HTMLElement).closest("a,button,input,select,textarea,[role=button],[data-window-controls]")) return
          winStartDragging()
        },
        onDoubleClick: (e: React.MouseEvent) => {
          if ((e.target as HTMLElement).closest("a,button,input,select,textarea,[role=button],[data-window-controls]")) return
          winToggleMaximize()
        },
      }
    : {}

  const main = items.filter((i) => !i.end)
  const end = items.filter((i) => i.end)
  const renderItem = (item: NavItem, labelFrom: "xl" | "2xl") => {
    const active = isNavActive(item.page, page)
    if (item.page === "pengaturan") return <SettingsMenu key={item.page} item={item} active={active} labelFrom={labelFrom} />
    const n = badges[item.page]
    return (
      <a
        key={item.page}
        href={href(item.page)}
        aria-current={active ? "page" : undefined}
        aria-label={t(item.label /* i18n-ignore: kunci tk */)}
        title={t(item.label /* i18n-ignore: kunci tk */)}
        className={navClass(active)}
      >
        <item.icon className="size-4" />
        <span className={labelClass(labelFrom)}>{t(item.label /* i18n-ignore: kunci tk */)}</span>
        {n ? (
          <span className="rounded-full bg-critical px-1.5 text-[10px] leading-4 font-semibold text-white tabular-nums" aria-label={t("{n} perlu perhatian", { n })}>
            {n > 99 ? "99+" : n}
          </span>
        ) : null}
      </a>
    )
  }

  return (
    <div {...dragProps} className="sticky top-0 z-30 border-b bg-background/95 backdrop-blur supports-[backdrop-filter]:bg-background/80">
      <div className="flex h-12 items-stretch md:h-14">
        {mac && <div aria-hidden className="shrink-0" style={{ width: MAC_TRAFFIC_LIGHTS }} />}
        <div className={cn(PAGE_CONTAINER, "flex h-full min-w-0 items-center gap-2")}>
          <a
            href={href("ringkasan")}
            aria-label={t("Telemon, ke Ringkasan")}
            className="flex shrink-0 items-center gap-2 rounded-md font-semibold outline-none focus-visible:ring-3 focus-visible:ring-ring/50"
          >
            <TelemonLogo />
            <span className="hidden sm:inline">Telemon</span>
          </a>
          {/* Desktop: titik Live yang bisa diklik (popup status). HP: lencana Live seperti semula. */}
          <div className="hidden md:block">
            <LiveIndicator />
          </div>
          <MobileLiveBadge />

          <h1 className="ml-1 truncate text-sm font-medium md:hidden">{title}</h1>

          {main.length > 0 && (
            <nav aria-label={t("Menu utama")} className="ml-3 hidden h-full items-stretch gap-0.5 md:flex">
              {main.map((i) => renderItem(i, "xl"))}
            </nav>
          )}

          <div className="ml-auto flex h-full shrink-0 items-center gap-1">
            {end.length > 0 && (
              <nav aria-label={t("Menu kelola")} className="hidden h-full items-stretch gap-0.5 md:flex">
                {end.map((i) => renderItem(i, "2xl"))}
              </nav>
            )}
            {end.length > 0 && <span aria-hidden className="mx-1 hidden h-5 w-px bg-border md:block" />}
            {/* HP: lencana LAN seperti semula; di desktop info LAN ada di popup titik Live. */}
            {access === "desktop" && share?.lan.enabled && (
              <Badge variant="outline" asChild className="gap-1.5 md:hidden">
                <a href={href("berbagi")} title={t("Serve ke browser aktif")}>
                  <WifiIcon className="size-3.5" />
                  <span className="hidden sm:inline">LAN</span>
                </a>
              </Badge>
            )}
            {access === "desktop" && share?.publik.status === "on" && (
              <Badge variant="outline" asChild className="gap-1.5 border-warning/70">
                <a href={href("berbagi")} title={t("Akses publik aktif")}>
                  <GlobeIcon className="size-3.5" />
                  <span className="hidden sm:inline">{t("Publik")}</span>
                </a>
              </Badge>
            )}
            <SearchTrigger />
            <AppearanceDrawer />
            <AccountMenu />
          </div>
        </div>
        {mac && <div aria-hidden className="shrink-0" style={{ width: MAC_TRAFFIC_LIGHTS }} />}
        {frameless && <WindowControls />}
      </div>
    </div>
  )
}

/** Lencana Live untuk HP (tampilan lama): titik berdenyut dalam pil, teks "Live" dari lebar sm. */
function MobileLiveBadge() {
  const t = useT()
  const { status } = useStore()
  return (
    <span className="flex shrink-0 items-center gap-1.5 rounded-full border px-2 py-0.5 text-xs text-muted-foreground md:hidden" title={status === "ready" ? t("Menerima event live") : t("Memuat")}>
      <span className="relative flex size-2">
        {status === "ready" && <span className="absolute inline-flex size-full animate-ping rounded-full bg-good opacity-60 motion-reduce:hidden" />}
        <span className={cn("relative inline-flex size-2 rounded-full", status === "ready" ? "bg-good" : "bg-muted-foreground/50")} />
      </span>
      <span className="hidden sm:inline">Live</span>
    </span>
  )
}

const navClass = (active: boolean) =>
  cn(
    "inline-flex h-full items-center gap-1.5 border-b-2 px-2 text-sm font-medium transition-colors outline-none focus-visible:bg-muted",
    active ? "border-brand text-brand" : "border-transparent text-muted-foreground hover:text-foreground"
  )

/** Label menu tersembunyi di lebar sempit (hanya ikon dengan tooltip) dan muncul mulai `xl` atau `2xl`. */
const labelClass = (from: "xl" | "2xl") => (from === "xl" ? "hidden xl:inline" : "hidden 2xl:inline")

/** Menu Pengaturan: dropdown berisi Umum, AI (dasbor provider, pemakaian, anggaran), dan Extension. */
function SettingsMenu({ item, active, labelFrom }: { item: NavItem; active: boolean; labelFrom: "xl" | "2xl" }) {
  const t = useT()
  const { segments } = useRoute()
  const onAi = segments[0] === "ai" || segments[0] === "ai-provider"
  const onExt = segments[0] === "extension"
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <button type="button" aria-current={active ? "page" : undefined} aria-label={t(item.label /* i18n-ignore: kunci tk */)} title={t(item.label /* i18n-ignore: kunci tk */)} className={navClass(active)}>
          <item.icon className="size-4" />
          <span className={labelClass(labelFrom)}>{t(item.label /* i18n-ignore: kunci tk */)}</span>
          <ChevronDownIcon className="size-3.5" />
        </button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="w-44">
        <DropdownMenuItem className={cn(active && !onAi && !onExt && "text-brand")} onSelect={() => navigate("pengaturan")}>
          <SettingsIcon /> {t("Umum")}
        </DropdownMenuItem>
        <DropdownMenuItem className={cn(onAi && "text-brand")} onSelect={() => navigatePath("pengaturan", ["ai"])}>
          <SparklesIcon /> {t("AI")}
        </DropdownMenuItem>
        <DropdownMenuItem className={cn(onExt && "text-brand")} onSelect={() => navigatePath("pengaturan", ["extension"])}>
          <PuzzleIcon /> {t("Extension")}
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  )
}
