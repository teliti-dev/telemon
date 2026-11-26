import * as React from "react"
import { CheckIcon, ChevronDownIcon, GlobeIcon, InfoIcon, PuzzleIcon, Share2Icon, SettingsIcon, SlidersHorizontalIcon, SparklesIcon, UserCogIcon, WifiIcon } from "lucide-react"

import { AccountMenu } from "@/components/account-menu"
import { AppearanceDrawer } from "@/components/appearance-drawer"
import { SearchTrigger } from "@/components/command-palette"
import { LiveIndicator } from "@/components/live-indicator"
import { TelemonLogo } from "@/components/telemon-logo"
import { ResizeHandles, WindowControls } from "@/components/window-controls"
import { isNavActive, type NavItem } from "@/components/nav-items"
import { Badge } from "@/components/ui/badge"
import { DropdownMenu, DropdownMenuContent, DropdownMenuGroup, DropdownMenuItem, DropdownMenuLabel, DropdownMenuSeparator, DropdownMenuTrigger } from "@/components/ui/dropdown-menu"
import { useNavBadges } from "@/hooks/use-nav-badges"
import { useNavItems } from "@/hooks/use-nav-items"
import { APP_VERSION } from "@/lib/app-info"
import { useT } from "@/lib/i18n"
import { PAGE_CONTAINER } from "@/lib/layout"
import { href, navigate, navigatePath, useRoute, type Page } from "@/lib/route"
import { useStore } from "@/lib/store-context"
import { cn } from "@/lib/utils"
import { initFrameless, MAC_TRAFFIC_LIGHTS, NEEDS_RESIZE_HANDLES, useFrameless, useMacOverlay, WINDOW_CONTROLS_SIDE, winStartDragging, winToggleMaximize } from "@/lib/window"

/**
 * Dua baris, menempel di atas:
 * - Baris atas (header): logo dan Telemon di kiri, Appearance (dan Profile di web) di kanan.
 *   Web: lencana Live seperti semula dan tombol Cari di header. Aplikasi desktop: titik Live yang bisa diklik
 *   (popup status), Appearance hanya ikon. Di Windows (tanpa bilah bawaan) dan macOS (Overlay) baris ini menjadi
 *   bilah judul: bisa diseret, memuat tombol jendela (Windows) atau ruang tombol lalu lintas (macOS).
 * - Baris menu (md ke atas): menu ringkas seperti semula; di aplikasi desktop kotak Cari ada di kanan menu.
 * - HP: menu ada di bawah layar; header tetap menempel dan memuat judul halaman.
 */
export function SiteHeader({ title }: { title: string }) {
  const t = useT()
  const { share, access } = useStore()
  const { page } = useRoute()
  const badges = useNavBadges()
  const items = useNavItems()
  const frameless = useFrameless()
  const mac = useMacOverlay()
  const isDesktop = access === "desktop"
  // Header menjadi bilah judul (Windows tanpa bilah bawaan, atau macOS Overlay): bisa diseret dan harus tetap terlihat.
  const titlebar = frameless || mac
  React.useEffect(initFrameless, [])
  // Windows, Linux dan macOS: bilah judul sama ringkasnya di semua platform (setinggi bilah judul bawaan), tanpa logo, ikon kecil.
  const compact = titlebar

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

  return (
    <>
      <div
        {...dragProps}
        className={cn(
          "sticky top-0 z-30 border-b bg-background/95 backdrop-blur supports-[backdrop-filter]:bg-background/80",
          !titlebar && "md:static md:border-b-0 md:bg-background md:backdrop-blur-none",
          // Bilah judul ringkas menyatu dengan baris menu di bawahnya: tanpa garis pemisah.
          compact && "border-b-0"
        )}
      >
        <div className={cn("flex items-stretch", compact ? "h-9" : "h-12 md:h-14")}>
          {compact && WINDOW_CONTROLS_SIDE === "left" && <WindowControls side="left" compact />}
          {mac && <div aria-hidden className="shrink-0" style={{ width: MAC_TRAFFIC_LIGHTS }} />}
          <div className={cn(PAGE_CONTAINER, "flex h-full min-w-0 items-center gap-2")}>
            <div className="flex min-w-0 flex-1 items-center gap-2">
              <a
                href={href("ringkasan")}
                aria-label={t("Telemon, ke Ringkasan")}
                className="flex shrink-0 items-center gap-2 rounded-md font-semibold outline-none focus-visible:ring-3 focus-visible:ring-ring/50"
              >
                {!compact && <TelemonLogo />}
                <span className={cn(!compact && "hidden sm:inline", compact && "text-sm")}>Telemon</span>
              </a>
              {isDesktop ? <LiveIndicator /> : <LivePill />}

              <h1 className="ml-1 truncate text-sm font-medium md:hidden">{title}</h1>
            </div>
            {!isDesktop && (
              <div className="hidden md:block">
                <SearchTrigger className="md:w-72 lg:w-96" />
              </div>
            )}
            <div className="flex flex-1 shrink-0 items-center justify-end gap-1">
              {isDesktop && share?.lan.enabled && (
                <Badge variant="outline" asChild className="gap-1.5">
                  <a href={href("berbagi")} title={t("Serve ke browser aktif")}>
                    <WifiIcon className="size-3.5" />
                    <span className="hidden sm:inline">LAN</span>
                  </a>
                </Badge>
              )}
              {isDesktop && share?.publik.status === "on" && (
                <Badge variant="outline" asChild className="gap-1.5 border-warning/70">
                  <a href={href("berbagi")} title={t("Akses publik aktif")}>
                    <GlobeIcon className="size-3.5" />
                    <span className="hidden sm:inline">{t("Publik")}</span>
                  </a>
                </Badge>
              )}
              {!isDesktop && (
                <span className="md:hidden">
                  <SearchTrigger />
                </span>
              )}
              <AppearanceDrawer iconOnly={isDesktop} compact={compact} />
              <AccountMenu />
            </div>
          </div>
          {mac && <div aria-hidden className="shrink-0" style={{ width: MAC_TRAFFIC_LIGHTS }} />}
          {frameless && WINDOW_CONTROLS_SIDE === "right" && <WindowControls compact />}
          {frameless && NEEDS_RESIZE_HANDLES && <ResizeHandles />}
        </div>
      </div>

      <div className={cn("sticky z-20 hidden border-b bg-background/95 backdrop-blur supports-[backdrop-filter]:bg-background/80 md:block", titlebar ? (compact ? "top-9" : "top-14") : "top-0")}>
        <div className={cn(PAGE_CONTAINER, "flex h-9 items-center gap-2")}>
          <nav aria-label={t("Menu utama")} className="-mb-px flex h-9 min-w-0 flex-1 items-end gap-0.5">
            {items
              .filter((i) => !i.end)
              .map((item) => {
                const active = isNavActive(item.page, page)
                const n = badges[item.page]
                return (
                  <a
                    key={item.page}
                    href={href(item.page)}
                    aria-current={active ? "page" : undefined}
                    className={navClass(active)}
                  >
                    <item.icon className="size-4" />
                    {/* i18n-ignore: label sudah berupa kunci (tk) */}
                    {t(item.label)}
                    {n ? (
                      <span
                        className="rounded-full bg-critical px-1.5 text-[10px] leading-4 font-semibold text-white tabular-nums"
                        aria-label={t("{n} perlu perhatian", { n })}
                      >
                        {n > 99 ? "99+" : n}
                      </span>
                    ) : null}
                  </a>
                )
              })}
            <SettingsMenu items={items} />
          </nav>
        </div>
      </div>
    </>
  )
}

/** Lencana Live untuk HP (tampilan lama): titik berdenyut dalam pil, teks "Live" dari lebar sm. */
function LivePill() {
  const t = useT()
  const { status } = useStore()
  return (
    <span className="flex shrink-0 items-center gap-1.5 rounded-full border px-2 py-0.5 text-xs text-muted-foreground " title={status === "ready" ? t("Menerima event live") : t("Memuat")}>
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
    "inline-flex h-9 items-center gap-1.5 border-b-2 px-2 text-sm font-medium transition-colors outline-none focus-visible:bg-muted",
    active ? "border-brand text-brand" : "border-transparent text-muted-foreground hover:text-foreground"
  )

const SETTINGS_PAGES: Page[] = ["pengaturan", "berbagi", "admin", "tentang"]

/**
 * Menu Pengaturan (desktop dan web): satu dropdown rata kanan yang memuat Pengaturan (Umum, AI, Extension), Kelola
 * (Admin, Berbagi) dan Tentang. Entri hanya muncul bila akun boleh melihatnya; Tentang selalu ada.
 */
function SettingsMenu({ items }: { items: NavItem[] }) {
  const t = useT()
  const { page, segments } = useRoute()
  const { share } = useStore()
  const has = (p: Page) => items.some((i) => i.page === p)
  const active = SETTINGS_PAGES.includes(page)
  const sub = segments[0]
  const onAi = page === "pengaturan" && (sub === "ai" || sub === "ai-provider")
  const onExt = page === "pengaturan" && sub === "extension"
  const lanOn = share?.lan.enabled
  const publicOn = share?.publik.status === "on"
  const tone = (on: boolean) => (on ? "text-brand" : undefined)

  const entry = (on: boolean, icon: React.ReactNode, label: string, go: () => void, extra?: React.ReactNode) => (
    <DropdownMenuItem key={label} className={cn("gap-2", tone(on), on && "bg-accent/60")} aria-current={on ? "page" : undefined} onSelect={go}>
      {icon}
      <span className="flex-1">{label}</span>
      {extra}
      {on && <CheckIcon className="size-3.5" />}
    </DropdownMenuItem>
  )

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <button
          type="button"
          aria-current={active ? "page" : undefined}
          aria-label={t("Menu pengaturan")}
          className={cn(navClass(active), "ml-auto")}
        >
          <SettingsIcon className="size-4" />
          {t("Pengaturan")}
          <ChevronDownIcon className="size-3.5" />
        </button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="w-56">
        {has("pengaturan") && (
          <DropdownMenuGroup>
            <DropdownMenuLabel className="text-xs text-muted-foreground">{t("Pengaturan")}</DropdownMenuLabel>
            {entry(page === "pengaturan" && !onAi && !onExt, <SlidersHorizontalIcon />, t("Umum"), () => navigate("pengaturan"))}
            {entry(onAi, <SparklesIcon />, t("AI"), () => navigatePath("pengaturan", ["ai"]))}
            {entry(onExt, <PuzzleIcon />, t("Extension"), () => navigatePath("pengaturan", ["extension"]))}
          </DropdownMenuGroup>
        )}
        {(has("admin") || has("berbagi")) && (
          <>
            {has("pengaturan") && <DropdownMenuSeparator />}
            <DropdownMenuGroup>
              <DropdownMenuLabel className="text-xs text-muted-foreground">{t("Kelola")}</DropdownMenuLabel>
              {has("admin") && entry(page === "admin", <UserCogIcon />, t("Admin"), () => navigate("admin"))}
              {has("berbagi") &&
                entry(
                  page === "berbagi",
                  <Share2Icon />,
                  t("Berbagi"),
                  () => navigate("berbagi"),
                  (lanOn || publicOn) && (
                    <span className={cn("rounded-full border px-1.5 text-[10px] leading-4", publicOn ? "border-warning/70" : "text-muted-foreground")}>
                      {publicOn ? t("Publik") : "LAN"}
                    </span>
                  )
                )}
            </DropdownMenuGroup>
          </>
        )}
        <DropdownMenuSeparator />
        {entry(page === "tentang", <InfoIcon />, t("Tentang"), () => navigate("tentang"), <span className="text-xs text-muted-foreground tabular-nums">{APP_VERSION}</span>)}
      </DropdownMenuContent>
    </DropdownMenu>
  )
}
