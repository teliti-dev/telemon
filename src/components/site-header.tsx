import { GlobeIcon, WifiIcon, ZapIcon } from "lucide-react"

import { AccountMenu } from "@/components/account-menu"
import { isNavActive, navFor } from "@/components/nav-items"
import { PreviewSwitch } from "@/components/preview-switch"
import { ScaleMenu } from "@/components/scale-menu"
import { ThemeToggle } from "@/components/theme-toggle"
import { Badge } from "@/components/ui/badge"
import { useNavBadges } from "@/hooks/use-nav-badges"
import { href, useRoute } from "@/lib/route"
import { useStore } from "@/lib/store-context"
import { cn } from "@/lib/utils"

/** Header: logo, menu utama (desktop), judul halaman (HP), status, tema, dan akun. */
export function SiteHeader({ title }: { title: string }) {
  const { share, status, access } = useStore()
  const { page } = useRoute()
  const badges = useNavBadges()

  return (
    <header className="sticky top-0 z-20 border-b bg-background/95 backdrop-blur supports-[backdrop-filter]:bg-background/80">
      <div className="mx-auto flex h-12 w-full max-w-screen-2xl items-center gap-1.5 px-3 md:px-4">
        <a
          href={href("ringkasan")}
          aria-label="Telemon, ke Ringkasan"
          className="flex shrink-0 items-center gap-2 rounded-md font-semibold outline-none focus-visible:ring-3 focus-visible:ring-ring/50"
        >
          <span className="flex size-7 items-center justify-center rounded-lg bg-primary text-primary-foreground">
            <ZapIcon className="size-3.5" />
          </span>
          <span className="hidden sm:inline">Telemon</span>
        </a>

        <nav aria-label="Menu utama" className="ml-3 hidden items-center gap-0.5 md:flex">
          {navFor(access).map((item) => {
            const active = isNavActive(item.page, page)
            const n = badges[item.page]
            return (
              <a
                key={item.page}
                href={href(item.page)}
                aria-current={active ? "page" : undefined}
                className={cn(
                  "inline-flex h-8 items-center gap-1.5 rounded-md px-2.5 text-sm font-medium transition-colors outline-none focus-visible:ring-3 focus-visible:ring-ring/50",
                  active ? "bg-muted text-foreground" : "text-muted-foreground hover:bg-muted/60 hover:text-foreground"
                )}
              >
                <item.icon className="size-4" />
                {item.label}
                {n ? (
                  <span
                    className="ml-0.5 rounded-full bg-critical px-1.5 text-[10px] leading-4 font-semibold text-white tabular-nums"
                    aria-label={`${n} perlu perhatian`}
                  >
                    {n > 99 ? "99+" : n}
                  </span>
                ) : null}
              </a>
            )
          })}
        </nav>

        <h1 className="ml-1 truncate text-sm font-medium md:hidden">{title}</h1>

        <div className="ml-auto flex shrink-0 items-center gap-1.5">
          {access === "desktop" && share?.lan.enabled && (
            <Badge variant="outline" asChild className="gap-1.5">
              <a href={href("berbagi")} title="Serve ke browser aktif">
                <WifiIcon className="size-3.5" />
                <span className="hidden sm:inline">LAN</span>
              </a>
            </Badge>
          )}
          {access === "desktop" && share?.publik.status === "on" && (
            <Badge variant="outline" asChild className="gap-1.5 border-warning/70">
              <a href={href("berbagi")} title="Akses publik aktif">
                <GlobeIcon className="size-3.5" />
                <span className="hidden sm:inline">Publik</span>
              </a>
            </Badge>
          )}
          <span
            className="flex items-center gap-1.5 px-1.5 text-xs text-muted-foreground"
            title={status === "ready" ? "Menerima event live" : "Memuat"}
          >
            <span className="relative flex size-2">
              {status === "ready" && (
                <span className="absolute inline-flex size-full animate-ping rounded-full bg-good opacity-60 motion-reduce:hidden" />
              )}
              <span
                className={cn(
                  "relative inline-flex size-2 rounded-full",
                  status === "ready" ? "bg-good" : "bg-muted-foreground/50"
                )}
              />
            </span>
            <span className="hidden sm:inline">Live</span>
          </span>
          <PreviewSwitch />
          <ScaleMenu />
          <ThemeToggle />
          <AccountMenu />
        </div>
      </div>
    </header>
  )
}
