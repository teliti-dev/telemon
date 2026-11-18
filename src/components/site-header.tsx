import { GlobeIcon, MoonIcon, SunIcon, WifiIcon } from "lucide-react"

import { useTheme } from "@/components/theme-provider"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Separator } from "@/components/ui/separator"
import { SidebarTrigger } from "@/components/ui/sidebar"
import { href } from "@/lib/route"
import { useStore } from "@/lib/store-context"

export function SiteHeader({ title }: { title: string }) {
  const { share, status } = useStore()
  const { theme, setTheme } = useTheme()
  const isDark =
    theme === "dark" ||
    (theme === "system" &&
      window.matchMedia("(prefers-color-scheme: dark)").matches)

  return (
    <header className="sticky top-0 z-10 flex h-12 shrink-0 items-center gap-2 border-b bg-background/95 px-3 backdrop-blur supports-[backdrop-filter]:bg-background/80 md:px-4">
      <SidebarTrigger className="-ml-1" />
      <Separator orientation="vertical" className="mr-1 data-[orientation=vertical]:h-4" />
      <h1 className="truncate text-sm font-medium">{title}</h1>

      <div className="ml-auto flex items-center gap-1.5">
        {share?.lan.enabled && (
          <Badge variant="outline" asChild className="gap-1.5">
            <a href={href("berbagi")} title="Serve ke browser aktif">
              <WifiIcon className="size-3.5" />
              <span className="hidden sm:inline">LAN</span>
            </a>
          </Badge>
        )}
        {share?.publik.status === "on" && (
          <Badge variant="outline" asChild className="gap-1.5">
            <a href={href("berbagi")} title="Akses publik aktif">
              <GlobeIcon className="size-3.5 text-warning" />
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
              className={
                status === "ready"
                  ? "relative inline-flex size-2 rounded-full bg-good"
                  : "relative inline-flex size-2 rounded-full bg-muted-foreground/50"
              }
            />
          </span>
          <span className="hidden sm:inline">Live</span>
        </span>
        <Button
          variant="ghost"
          size="icon"
          aria-label={isDark ? "Mode terang" : "Mode gelap"}
          onClick={() => setTheme(isDark ? "light" : "dark")}
        >
          {isDark ? <SunIcon /> : <MoonIcon />}
        </Button>
      </div>
    </header>
  )
}
