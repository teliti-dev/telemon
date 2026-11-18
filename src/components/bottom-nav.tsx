import { SparklesIcon } from "lucide-react"

import { isNavActive } from "@/components/nav-items"
import { useNavBadges } from "@/hooks/use-nav-badges"
import { useNavItems } from "@/hooks/use-nav-items"
import { useAiChat } from "@/lib/ai-context"
import { useT } from "@/lib/i18n"
import { href, useRoute } from "@/lib/route"
import { cn } from "@/lib/utils"

const CELL =
  "flex h-14 w-full flex-col items-center justify-center gap-1 text-[11px] font-medium outline-none focus-visible:bg-muted"

/**
 * Menu utama di bawah layar untuk HP (< 768px). Menu kelola (Berbagi, Admin, Pengaturan)
 * ada di modal Profile pada header; tombol AI chat di ujung kanan.
 */
export function BottomNav() {
  const t = useT()
  const { page } = useRoute()
  const badges = useNavBadges()
  // Menu kelola (Berbagi, Admin, Pengaturan) ada di modal Profile pada header, bukan di sini.
  const main = useNavItems().filter((i) => !i.more)
  const ai = useAiChat()
  const cols = main.length + (ai.available ? 1 : 0)

  return (
    <nav
      aria-label={t("Menu utama")}
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
                      aria-label={t("{n} perlu perhatian", { n })}
                    >
                      {n > 99 ? "99+" : n}
                    </span>
                  ) : null}
                </span>
                {/* i18n-ignore: label bertanda tk() */}
                {t(item.label)}
              </a>
            </li>
          )
        })}
        {ai.available && (
          <li>
            <button
              type="button"
              className={cn(CELL, ai.open ? "text-brand" : "text-muted-foreground")}
              aria-label={t("Buka AI chat")}
              onClick={() => ai.setOpen(true)}
            >
              <span className="flex h-6 items-center justify-center">
                <SparklesIcon className="size-5" strokeWidth={ai.open ? 2.4 : 2} />
              </span>
              {t("AI")}
            </button>
          </li>
        )}
      </ul>
    </nav>
  )
}
