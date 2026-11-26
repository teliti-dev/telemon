import * as React from "react"

/** Sumber hasil pencarian (palet perintah, Ctrl+K). Bawaan: hanya menu. */
export type SearchSource = "menu" | "user" | "router" | "other"
export const SEARCH_SOURCES: SearchSource[] = ["menu", "user", "router", "other"]

export type SearchPrefs = Record<SearchSource, boolean>
const KEY = "telemon.search.sources"
const DEFAULTS: SearchPrefs = { menu: true, user: false, router: false, other: false }

function load(): SearchPrefs {
  try {
    const raw = JSON.parse(localStorage.getItem(KEY) ?? "null") as Partial<SearchPrefs> | null
    if (raw && typeof raw === "object") return { ...DEFAULTS, ...Object.fromEntries(SEARCH_SOURCES.filter((s) => typeof raw[s] === "boolean").map((s) => [s, raw[s]])) }
  } catch {
    /* penyimpanan browser tidak tersedia atau rusak: pakai bawaan */
  }
  return { ...DEFAULTS }
}

let prefs: SearchPrefs = load()
const listeners = new Set<() => void>()

export function setSearchSource(source: SearchSource, on: boolean) {
  prefs = { ...prefs, [source]: on }
  try {
    localStorage.setItem(KEY, JSON.stringify(prefs))
  } catch {
    /* pilihan hanya berlaku sesi ini */
  }
  listeners.forEach((l) => l())
}

/** Pilihan sumber pencarian milik pengguna ini di browser/aplikasi ini (bukan pengaturan server). */
export function useSearchPrefs() {
  return React.useSyncExternalStore(
    (cb) => {
      listeners.add(cb)
      return () => listeners.delete(cb)
    },
    () => prefs,
    () => DEFAULTS
  )
}
