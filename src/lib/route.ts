import * as React from "react"

export const PAGES = ["ringkasan", "user", "event", "router", "berbagi"] as const
export type Page = (typeof PAGES)[number]

function subscribe(cb: () => void) {
  window.addEventListener("hashchange", cb)
  return () => window.removeEventListener("hashchange", cb)
}

const getHash = () => window.location.hash

/** Routing berbasis hash (#/user?status=active) agar jalan sama di Tauri dan mode serve. */
export function useRoute() {
  const hash = React.useSyncExternalStore(subscribe, getHash, () => "")
  return React.useMemo(() => {
    const [path, query = ""] = hash.replace(/^#\/?/, "").split("?")
    const page = (PAGES as readonly string[]).includes(path)
      ? (path as Page)
      : "ringkasan"
    return { page, params: new URLSearchParams(query), hash }
  }, [hash])
}

export function href(page: Page, params?: Record<string, string>) {
  const q = params ? new URLSearchParams(params).toString() : ""
  return `#/${page}${q ? `?${q}` : ""}`
}

export function navigate(page: Page, params?: Record<string, string>) {
  window.location.hash = href(page, params)
}
