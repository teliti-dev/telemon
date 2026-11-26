import * as React from "react"
import { CornerDownLeftIcon, LayersIcon, PuzzleIcon, RadioTowerIcon, SearchIcon, SettingsIcon, UserRoundIcon, UsersIcon, type LucideIcon } from "lucide-react"

import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog"
import { useNavItems } from "@/hooks/use-nav-items"
import { useAuth } from "@/lib/auth-context"
import { closePalette, isMacPlatform, openPalette, togglePalette, usePaletteOpen } from "@/lib/command-palette"
import { extensionPages, useExtensionState } from "@/lib/extensions/registry"
import { tk, useT } from "@/lib/i18n"
import { navigate, navigatePath } from "@/lib/route"
import { rank } from "@/lib/search"
import { SEARCH_SOURCES, setSearchSource, useSearchPrefs, type SearchSource } from "@/lib/search-prefs"
import { useStore } from "@/lib/store-context"
import { cn } from "@/lib/utils"

export interface PaletteItem {
  id: string
  source: SearchSource
  title: string
  subtitle?: string
  icon: LucideIcon
  run: () => void
  /** Kolom yang dicari selain judul. */
  hay?: string[]
}

const SOURCE_LABEL: Record<SearchSource, string> = { menu: tk("Menu"), user: tk("User"), router: tk("Router"), other: tk("Lainnya") }
const LIMIT: Record<SearchSource, number> = { menu: 10, user: 8, router: 5, other: 6 }

/** Sumber yang boleh dipakai akun ini: mengikuti izin menunya. */
export function useSearchAvailability(): Record<SearchSource, boolean> {
  const { access } = useStore()
  const { session } = useAuth()
  const p = session?.permissions
  const ok = (r: "user" | "router" | "profile") => access === "desktop" || (p?.[r] ?? "none") !== "none"
  return { menu: true, user: ok("user"), router: ok("router"), other: true }
}

/** Saklar sumber pencarian (dipakai di palet dan di Appearance). Pilihan tersimpan di browser/aplikasi ini. */
export function SearchSourceToggles({ className }: { className?: string }) {
  const t = useT()
  const prefs = useSearchPrefs()
  const avail = useSearchAvailability()
  return (
    <div className={cn("flex flex-wrap gap-1.5", className)} role="group" aria-label={t("Sumber pencarian")}>
      {SEARCH_SOURCES.map((s) => (
        <button
          key={s}
          type="button"
          aria-pressed={prefs[s] && avail[s]}
          disabled={!avail[s]}
          title={avail[s] ? undefined : t("Peran Anda tidak punya akses")}
          onClick={() => setSearchSource(s, !prefs[s])}
          className={cn(
            "inline-flex h-7 items-center gap-1.5 rounded-full border px-2.5 text-xs font-medium transition-colors outline-none focus-visible:ring-3 focus-visible:ring-ring/50 disabled:opacity-40",
            prefs[s] && avail[s] ? "border-brand bg-brand/10 text-brand" : "text-muted-foreground hover:text-foreground"
          )}
        >
          <span className={cn("size-1.5 rounded-full", prefs[s] && avail[s] ? "bg-brand" : "bg-muted-foreground/40")} />
          {t(SOURCE_LABEL[s] /* i18n-ignore: kunci tk */)}
        </button>
      ))}
    </div>
  )
}

/** Tombol pencarian di header: kolom tampilan dengan petunjuk pintasan; di HP hanya ikon. */
export function SearchTrigger({ className }: { className?: string }) {
  const t = useT()
  return (
    <button
      type="button"
      onClick={openPalette}
      aria-label={t("Cari (Ctrl+K)")}
      aria-keyshortcuts="Control+K Meta+K"
      className={cn("inline-flex h-8 shrink-0 items-center gap-2 rounded-md border bg-background px-2 text-sm text-muted-foreground transition-colors outline-none hover:text-foreground focus-visible:ring-3 focus-visible:ring-ring/50 md:w-44 lg:w-56", className)}
    >
      <SearchIcon className="size-4 shrink-0" />
      <span className="hidden flex-1 truncate text-left md:inline">{t("Cari…")}</span>
      <kbd className="hidden rounded border bg-muted px-1.5 font-sans text-[10px] leading-4 md:inline">{isMacPlatform() ? "⌘K" : "Ctrl K"}</kbd>
    </button>
  )
}

/**
 * Palet pencarian ala Spotlight (Ctrl/Cmd+K): menu, user, router, dan lainnya; tiap sumber bisa dinyalakan atau dimatikan
 * (bawaan: hanya menu). Panah atas/bawah memilih, Enter membuka, Esc menutup.
 */
export function CommandPalette({ onOpenUser }: { onOpenUser: (id: string) => void }) {
  const t = useT()
  const open = usePaletteOpen()
  const prefs = useSearchPrefs()
  const avail = useSearchAvailability()
  const { liveUsers, routers, profiles, metas, routerById } = useStore()
  const { session } = useAuth()
  const navItems = useNavItems()
  useExtensionState()
  const [query, setQuery] = React.useState("")
  const [active, setActive] = React.useState(0)
  const listRef = React.useRef<HTMLUListElement>(null)

  React.useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && !e.altKey && e.key.toLowerCase() === "k") {
        e.preventDefault()
        togglePalette()
      }
    }
    window.addEventListener("keydown", onKey)
    return () => window.removeEventListener("keydown", onKey)
  }, [])

  const on = (s: SearchSource) => prefs[s] && avail[s]
  const pagesAllowed = navItems.some((i) => i.page === "pengaturan")

  const results = React.useMemo(() => {
    const q = query.trim()
    const all: PaletteItem[] = []
    if (on("menu")) {
      const menu: PaletteItem[] = navItems.map((i) => ({ id: `menu:${i.page}`, source: "menu", title: t(i.label /* i18n-ignore: kunci tk */), subtitle: t("Buka menu"), icon: i.icon, run: () => navigate(i.page), hay: [i.label] }))
      if (pagesAllowed) {
        menu.push(
          { id: "menu:set-umum", source: "menu", title: t("Pengaturan umum"), subtitle: t("Pengaturan"), icon: SettingsIcon, run: () => navigate("pengaturan"), hay: ["Pengaturan umum", "Pengaturan", "settings general"] },
          { id: "menu:set-ai", source: "menu", title: t("Pengaturan AI"), subtitle: t("Pengaturan"), icon: SettingsIcon, run: () => navigatePath("pengaturan", ["ai"]), hay: ["Pengaturan AI", "ai provider budget anggaran"] },
          { id: "menu:set-ext", source: "menu", title: t("Extension"), subtitle: t("Pengaturan"), icon: PuzzleIcon, run: () => navigatePath("pengaturan", ["extension"]), hay: ["Extension", "plugin ekstensi"] },
          { id: "menu:set-store", source: "menu", title: t("Toko extension"), subtitle: t("Pengaturan"), icon: PuzzleIcon, run: () => navigatePath("pengaturan", ["extension", "store"]), hay: ["Toko extension", "store marketplace plugin"] }
        )
      }
      if (session) menu.push({ id: "menu:akun", source: "menu", title: t("Akun saya"), subtitle: t("Buka menu"), icon: UserRoundIcon, run: () => navigate("akun"), hay: ["Akun saya", "account profile profil password 2fa"] })
      all.push(...pick(menu, q, LIMIT.menu))
    }
    if (on("user") && q) {
      const users: PaletteItem[] = liveUsers.map((u) => {
        const m = metas[u.id]
        return {
          id: `user:${u.id}`,
          source: "user",
          title: u.username,
          subtitle: [routerById.get(u.routerId)?.name, u.profile, u.address].filter(Boolean).join(" · "),
          icon: UsersIcon,
          run: () => onOpenUser(u.id),
          hay: [u.address ?? "", u.callerId ?? "", u.profile, m?.customerName ?? u.comment ?? "", m?.phone ?? "", routerById.get(u.routerId)?.name ?? ""],
        }
      })
      all.push(...pick(users, q, LIMIT.user))
    }
    if (on("router") && q) {
      const rs: PaletteItem[] = routers.map((r) => ({ id: `router:${r.id}`, source: "router", title: r.name, subtitle: `${r.host}:${r.port}`, icon: RadioTowerIcon, run: () => navigate("user", { router: r.id }), hay: [r.host, r.model, r.version] }))
      all.push(...pick(rs, q, LIMIT.router))
    }
    if (on("other") && q) {
      const other: PaletteItem[] = [
        ...profiles.map((p): PaletteItem => ({ id: `profile:${p.id}`, source: "other", title: p.name, subtitle: t("Profil PPP"), icon: LayersIcon, run: () => navigate("profile"), hay: [p.rateLimit ?? ""] })),
        ...extensionPages().map((p): PaletteItem => ({ id: `ext:${p.extId}/${p.id}`, source: "other", title: p.extName, subtitle: p.title, icon: PuzzleIcon, run: () => navigatePath("ext", [p.extId, p.id]) })),
      ]
      all.push(...pick(other, q, LIMIT.other))
    }
    return all
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [query, prefs, avail.user, avail.router, navItems, liveUsers, routers, profiles, metas, routerById, pagesAllowed, session, onOpenUser, t])

  const sel = Math.min(active, Math.max(results.length - 1, 0))
  React.useEffect(() => {
    listRef.current?.querySelector<HTMLElement>(`[data-index="${sel}"]`)?.scrollIntoView({ block: "nearest" })
  }, [sel, results])

  const run = (item?: PaletteItem) => {
    if (!item) return
    closePalette()
    setQuery("")
    item.run()
  }
  const onKeyDown = (e: React.KeyboardEvent) => {
    const n = results.length
    if (e.key === "ArrowDown" || (e.ctrlKey && e.key.toLowerCase() === "n")) {
      e.preventDefault()
      if (n) setActive((sel + 1) % n)
    } else if (e.key === "ArrowUp" || (e.ctrlKey && e.key.toLowerCase() === "p")) {
      e.preventDefault()
      if (n) setActive((sel - 1 + n) % n)
    } else if (e.key === "Home" && e.altKey) {
      e.preventDefault()
      setActive(0)
    } else if (e.key === "Enter" && !e.nativeEvent.isComposing) {
      e.preventDefault()
      run(results[sel])
    }
  }

  const anyOn = SEARCH_SOURCES.some(on)
  const groups = SEARCH_SOURCES.filter((s) => results.some((r) => r.source === s))
  let index = -1
  return (
    <Dialog
      open={open}
      onOpenChange={(o) => {
        if (o) openPalette()
        else {
          closePalette()
          setQuery("")
          setActive(0)
        }
      }}
    >
      <DialogContent showCloseButton={false} className="top-[12svh] translate-y-0 gap-0 overflow-hidden p-0 sm:max-w-xl">
        <DialogTitle className="sr-only">{t("Pencarian")}</DialogTitle>
        <DialogDescription className="sr-only">{t("Ketik untuk mencari. Panah atas dan bawah memilih, Enter membuka, Esc menutup.")}</DialogDescription>
        <div className="flex items-center gap-2 border-b px-3">
          <SearchIcon className="size-4 shrink-0 text-muted-foreground" />
          <input
            autoFocus
            role="combobox"
            aria-expanded
            aria-controls="palette-list"
            aria-activedescendant={results[sel] ? `palette-opt-${sel}` : undefined}
            aria-label={t("Cari")}
            value={query}
            onChange={(e) => {
              setQuery(e.target.value)
              setActive(0)
            }}
            onKeyDown={onKeyDown}
            placeholder={anyOn ? placeholder(prefs, avail, t) : t("Nyalakan sumber pencarian di bawah")}
            className="h-12 w-full bg-transparent text-sm outline-none placeholder:text-muted-foreground"
          />
          <kbd className="hidden rounded border bg-muted px-1.5 font-sans text-[10px] leading-4 text-muted-foreground sm:inline">esc</kbd>
        </div>

        <ul id="palette-list" role="listbox" ref={listRef} className="max-h-[50svh] overflow-y-auto p-1.5">
          {results.length === 0 && (
            <li role="presentation" className="px-3 py-8 text-center text-sm text-muted-foreground">
              {!anyOn ? t("Tidak ada sumber pencarian yang menyala.") : query.trim() ? t("Tidak ada hasil untuk “{q}”.", { q: query.trim() }) : t("Ketik untuk mencari user, router, atau lainnya.")}
            </li>
          )}
          {groups.map((g) => (
            <React.Fragment key={g}>
              <li role="presentation" className="px-2 pt-2 pb-1 text-[11px] font-medium tracking-wide text-muted-foreground uppercase">
                {t(SOURCE_LABEL[g] /* i18n-ignore: kunci tk */)}
              </li>
              {results
                .filter((r) => r.source === g)
                .map((r) => {
                  index += 1
                  const i = index
                  const isActive = i === sel
                  return (
                    <li key={r.id} id={`palette-opt-${i}`} role="option" aria-selected={isActive} data-index={i}>
                      <button
                        type="button"
                        tabIndex={-1}
                        onMouseMove={() => setActive(i)}
                        onClick={() => run(r)}
                        className={cn("flex w-full items-center gap-3 rounded-lg px-2.5 py-2 text-left text-sm", isActive ? "bg-brand/10 text-foreground" : "text-foreground/90")}
                      >
                        <r.icon className={cn("size-4 shrink-0", isActive ? "text-brand" : "text-muted-foreground")} />
                        <span className="min-w-0 flex-1">
                          <span className="block truncate font-medium">{r.title}</span>
                          {r.subtitle && <span className="block truncate text-xs text-muted-foreground">{r.subtitle}</span>}
                        </span>
                        {isActive && <CornerDownLeftIcon className="size-3.5 shrink-0 text-muted-foreground" />}
                      </button>
                    </li>
                  )
                })}
            </React.Fragment>
          ))}
        </ul>

        <div className="flex flex-wrap items-center gap-x-4 gap-y-2 border-t bg-muted/30 px-3 py-2">
          <span className="hidden text-xs text-muted-foreground sm:inline">
            <kbd className="font-sans">↑↓</kbd> {t("pilih")} · <kbd className="font-sans">↵</kbd> {t("buka")} · <kbd className="font-sans">esc</kbd> {t("tutup")}
          </span>
          <SearchSourceToggles className="ml-auto" />
        </div>
      </DialogContent>
    </Dialog>
  )
}

/** Urutkan kecocokan dan ambil `limit` teratas; tanpa kata kunci, semuanya dalam urutan asal. */
function pick(items: PaletteItem[], q: string, limit: number): PaletteItem[] {
  if (!q) return items.slice(0, limit)
  return items
    .map((it) => ({ it, s: rank(q, [it.title, it.subtitle, ...(it.hay ?? [])]) }))
    .filter((x) => x.s > 0)
    .sort((a, b) => b.s - a.s)
    .slice(0, limit)
    .map((x) => x.it)
}

function placeholder(prefs: Record<SearchSource, boolean>, avail: Record<SearchSource, boolean>, t: ReturnType<typeof useT>) {
  const parts = SEARCH_SOURCES.filter((s) => prefs[s] && avail[s]).map((s) => t(SOURCE_LABEL[s] /* i18n-ignore: kunci tk */).toLowerCase())
  return t("Cari {list}…", { list: parts.join(", ") })
}
