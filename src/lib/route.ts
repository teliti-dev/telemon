import * as React from "react"

export const PAGES = [
  "ringkasan",
  "user",
  "profile",
  "event",
  "router",
  "berbagi",
  "admin",
  "pengaturan",
  "tentang",
  "akun",
  "import",
  "appearance",
  "ai",
  "ext",
] as const
export type Page = (typeof PAGES)[number]

function subscribe(cb: () => void) {
  window.addEventListener("hashchange", cb)
  return () => window.removeEventListener("hashchange", cb)
}

const getHash = () => window.location.hash

function decode(s: string) {
  try {
    return decodeURIComponent(s)
  } catch {
    return s
  }
}

/**
 * Routing berbasis hash agar jalan sama di Tauri dan mode serve.
 * `#/user?status=active` = daftar; `#/user/<id>` = detail; `#/user/<id>/edit`; `#/user/new`.
 * `segments` = bagian setelah nama halaman, sudah di-decode (id boleh memuat ":").
 */
export function useRoute() {
  const hash = React.useSyncExternalStore(subscribe, getHash, () => "")
  return React.useMemo(() => {
    const [path, query = ""] = hash.replace(/^#\/?/, "").split("?")
    const [first, ...rest] = path.split("/")
    const page = (PAGES as readonly string[]).includes(first) ? (first as Page) : "ringkasan"
    const segments = (page === first ? rest : []).filter(Boolean).map(decode)
    return { page, segments, params: new URLSearchParams(query), hash }
  }, [hash])
}

export function href(page: Page, params?: Record<string, string>) {
  const q = params ? new URLSearchParams(params).toString() : ""
  return `#/${page}${q ? `?${q}` : ""}`
}

/** Alamat halaman bertingkat, mis. pathHref("user", [id, "edit"]) = "#/user/<id>/edit". */
export function pathHref(page: Page, segments: string[] = [], params?: Record<string, string>) {
  const q = params && Object.keys(params).length ? new URLSearchParams(params).toString() : ""
  return `#/${[page, ...segments.map(encodeURIComponent)].join("/")}${q ? `?${q}` : ""}`
}

export function navigate(page: Page, params?: Record<string, string>) {
  window.location.hash = href(page, params)
}

export function navigatePath(page: Page, segments: string[] = [], params?: Record<string, string>) {
  window.location.hash = pathHref(page, segments, params)
}
