import * as React from "react"

let open = false
const listeners = new Set<() => void>()
const set = (v: boolean) => {
  if (open === v) return
  open = v
  listeners.forEach((l) => l())
}

export const openPalette = () => set(true)
export const closePalette = () => set(false)
export const togglePalette = () => set(!open)

/** Status buka/tutup palet pencarian; dipakai tombol di header dan pintasan Ctrl/Cmd+K. */
export function usePaletteOpen() {
  return React.useSyncExternalStore(
    (cb) => {
      listeners.add(cb)
      return () => listeners.delete(cb)
    },
    () => open,
    () => false
  )
}

export const isMacPlatform = () => typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.platform || navigator.userAgent)
