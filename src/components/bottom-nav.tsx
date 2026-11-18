import { EllipsisIcon } from "lucide-react"

import { isNavActive } from "@/components/nav-items"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import { useNavBadges } from "@/hooks/use-nav-badges"
import { useNavItems } from "@/hooks/use-nav-items"
import { href, useRoute } from "@/lib/route"
import { cn } from "@/lib/utils"

const CELL =
  "flex h-14 w-full flex-col items-center justify-center gap-1 text-[11px] font-medium outline-none focus-visible:bg-muted"

/**
 * Menu utama di bawah layar untuk HP (< 768px). Menu kelola (Berbagi, Admin,
 * Pengaturan) dikumpulkan di "Lainnya" agar menu bawah tidak penuh.
 */
export function BottomNav() {
  const { page } = useRoute()
  const badges = useNavBadges()
  const all = useNavItems()
  const main = all.filter((i) => !i.more)
  const extra = all.filter((i) => i.more)
  const cols = main.length + (extra.length > 0 ? 1 : 0)
  const extraActive = extra.some((i) => isNavActive(i.page, page))

  return (
    <nav
      aria-label="Menu utama"
      className="fixed inset-x-0 bottom-0 z-30 border-t bg-background/95 pb-[env(safe-area-inset-bottom)] backdrop-blur supports-[backdrop-filter]:bg-background/85 md:hidden"
    >
      <ul className="grid" style={{ gridTemplateColumns: `repeat(${Math.max(cols, 1)}, minmax(0, 1fr))` }}>
        {main.map((item) => {
          const active = isNavActive(item.page, page)
          const n = badges[item.page]
          return (
            <li key={item.page}>
              <a
                href={href(item.page)}
                aria-current={active ? "page" : undefined}
                className={cn(CELL, active ? "text-brand" : "text-muted-foreground")}
              >
                <span className="relative flex h-6 items-center justify-center">
                  <item.icon className="size-5" strokeWidth={active ? 2.4 : 2} />
                  {n ? (
                    <span
                      className="absolute -top-1 -right-2.5 min-w-4 rounded-full bg-critical px-1 text-center text-[10px] leading-4 font-semibold text-white tabular-nums"
                      aria-label={`${n} perlu perhatian`}
                    >
                      {n > 99 ? "99+" : n}
                    </span>
                  ) : null}
                </span>
                {item.label}
              </a>
            </li>
          )
        })}
        {extra.length > 0 && (
          <li>
            <DropdownMenu>
              <DropdownMenuTrigger asChild>
                <button type="button" className={cn(CELL, extraActive ? "text-brand" : "text-muted-foreground")}>
                  <span className="flex h-6 items-center justify-center">
                    <EllipsisIcon className="size-5" strokeWidth={extraActive ? 2.4 : 2} />
                  </span>
                  Lainnya
                </button>
              </DropdownMenuTrigger>
              <DropdownMenuContent side="top" align="end" className="w-48">
                {extra.map((item) => (
                  <DropdownMenuItem key={item.page} asChild className={cn(isNavActive(item.page, page) && "text-brand")}>
                    <a href={href(item.page)}>
                      <item.icon /> {item.label}
                    </a>
                  </DropdownMenuItem>
                ))}
              </DropdownMenuContent>
            </DropdownMenu>
          </li>
        )}
      </ul>
    </nav>
  )
}
