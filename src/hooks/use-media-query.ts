import * as React from "react"

/** True bila media query cocok; ikut berubah saat jendela diubah ukurannya. */
export function useMediaQuery(query: string) {
  return React.useSyncExternalStore(
    (cb) => {
      const m = window.matchMedia(query)
      m.addEventListener("change", cb)
      return () => m.removeEventListener("change", cb)
    },
    () => window.matchMedia(query).matches,
    () => false
  )
}

/** Lebar md ke atas (desktop/tablet): menu atas dan dropdown. Di bawahnya: menu bawah dan modal. */
export const useIsDesktop = () => useMediaQuery("(min-width: 768px)")
