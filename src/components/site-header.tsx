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
  const firstEnd = items.findIndex((i) => i.end)
  const frameless = useFrameless()
  const mac = useMacOverlay()
  const isDesktop = access === "desktop"
  // Header menjadi bilah judul (Windows tanpa bilah bawaan, atau macOS Overlay): bisa diseret dan harus tetap terlihat.
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

  return (
    <>
      <div
        {...dragProps}
        className={cn(
          "sticky top-0 z-30 border-b bg-background/95 backdrop-blur supports-[backdrop-filter]:bg-background/80",
          !titlebar && "md:static md:border-b-0 md:bg-background md:backdrop-blur-none"
        )}
      >
        <div className="flex h-12 items-stretch md:h-14">
          {mac && <div aria-hidden className="shrink-0" style={{ width: MAC_TRAFFIC_LIGHTS }} />}
          <div className={cn(PAGE_CONTAINER, "flex h-full min-w-0 items-center gap-2")}>
            <div className="flex min-w-0 flex-1 items-center gap-2">
              <a
                href={href("ringkasan")}
                aria-label={t("Telemon, ke Ringkasan")}
                className="flex shrink-0 items-center gap-2 rounded-md font-semibold outline-none focus-visible:ring-3 focus-visible:ring-ring/50"
              >
                <TelemonLogo />
                <span className="hidden sm:inline">Telemon</span>
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
              <AppearanceDrawer iconOnly={isDesktop} />
              <AccountMenu />
            </div>
          </div>
          {mac && <div aria-hidden className="shrink-0" style={{ width: MAC_TRAFFIC_LIGHTS }} />}
          {frameless && <WindowControls />}
        </div>
      </div>

      <div className={cn("sticky z-20 hidden border-b bg-background/95 backdrop-blur supports-[backdrop-filter]:bg-background/80 md:block", titlebar ? "top-14" : "top-0")}>
        <div className={cn(PAGE_CONTAINER, "flex h-9 items-center gap-2")}>
          <nav aria-label={t("Menu utama")} className="-mb-px flex h-9 min-w-0 flex-1 items-end gap-0.5">
            {items.map((item, index) => {
              const active = isNavActive(item.page, page)
              const n = badges[item.page]
              if (item.page === "pengaturan") {
                return <SettingsMenu key={item.page} item={item} active={active} first={index === firstEnd} />
              }
              return (
                <a
                  key={item.page}
                  href={href(item.page)}
                  aria-current={active ? "page" : undefined}
                  title={t(item.label /* i18n-ignore: kunci tk */)}
                  className={cn(navClass(active), index === firstEnd && "ml-auto")}
                >
                  <item.icon className="size-4" />
                  <span className={labelClass}>{t(item.label /* i18n-ignore: kunci tk */)}</span>
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
          </nav>
          {isDesktop && <SearchTrigger />}
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

/** Di lebar md (960 sampai 1023 px) label menu disembunyikan (hanya ikon dengan tooltip) supaya kotak Cari muat. */
const labelClass = "max-lg:hidden"

const navClass = (active: boolean) =>
  cn(
    "inline-flex h-9 items-center gap-1.5 border-b-2 px-2 text-sm font-medium transition-colors outline-none focus-visible:bg-muted",
    active ? "border-brand text-brand" : "border-transparent text-muted-foreground hover:text-foreground"
  )

/** Menu Pengaturan: dropdown berisi Umum, AI (dasbor provider, pemakaian, anggaran), dan Extension. */
function SettingsMenu({ item, active, first }: { item: NavItem; active: boolean; first: boolean }) {
  const t = useT()
  const { segments } = useRoute()
  const onAi = segments[0] === "ai" || segments[0] === "ai-provider"
  const onExt = segments[0] === "extension"
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <button type="button" aria-current={active ? "page" : undefined} title={t(item.label /* i18n-ignore: kunci tk */)} aria-label={t(item.label /* i18n-ignore: kunci tk */)} className={cn(navClass(active), first && "ml-auto")}>
          <item.icon className="size-4" />
          <span className={labelClass}>{t(item.label /* i18n-ignore: kunci tk */)}</span>
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
