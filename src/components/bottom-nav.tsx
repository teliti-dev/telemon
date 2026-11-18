import { isNavActive, navFor } from "@/components/nav-items"
import { useNavBadges } from "@/hooks/use-nav-badges"
import { href, useRoute } from "@/lib/route"
import { useStore } from "@/lib/store-context"
import { cn } from "@/lib/utils"

/** Menu utama di bawah layar untuk HP (< 768px). */
export function BottomNav() {
  const { page } = useRoute()
  const badges = useNavBadges()
  const { access } = useStore()
  const items = navFor(access)

  return (
    <nav
      aria-label="Menu utama"
      className="fixed inset-x-0 bottom-0 z-30 border-t bg-background/95 pb-[env(safe-area-inset-bottom)] backdrop-blur supports-[backdrop-filter]:bg-background/85 md:hidden"
    >
      <ul className="grid" style={{ gridTemplateColumns: `repeat(${items.length}, minmax(0, 1fr))` }}>
        {items.map((item) => {
          const active = isNavActive(item.page, page)
          const n = badges[item.page]
          return (
            <li key={item.page}>
              <a
                href={href(item.page)}
                aria-current={active ? "page" : undefined}
                className={cn(
                  "flex h-14 flex-col items-center justify-center gap-1 text-[11px] font-medium outline-none focus-visible:bg-muted",
                  active ? "text-brand" : "text-muted-foreground"
                )}
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
      </ul>
    </nav>
  )
}
