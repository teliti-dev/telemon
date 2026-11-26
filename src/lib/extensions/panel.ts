import * as React from "react"

import type { ExtPanelProps } from "./sdk"
import { findPanel } from "./registry"

export interface PanelTab {
  key: string
  extId: string
  panelId: string
  title: string
  params: Record<string, unknown>
}

interface PanelState {
  tabs: PanelTab[]
  active: string | null
  /** Panel dilipat: hanya bilah tab yang terlihat. */
  collapsed: boolean
  maximized: boolean
  height: number
}

const HEIGHT_KEY = "telemon.panel.height"
const MIN_H = 160

function loadHeight() {
  try {
    const n = Number(localStorage.getItem(HEIGHT_KEY))
    if (n >= MIN_H) return n
  } catch {
    /* penyimpanan browser tidak tersedia */
  }
  return 340
}

let state: PanelState = { tabs: [], active: null, collapsed: false, maximized: false, height: loadHeight() }
const listeners = new Set<() => void>()
const set = (next: Partial<PanelState>) => {
  state = { ...state, ...next }
  listeners.forEach((l) => l())
}
const subscribe = (cb: () => void) => {
  listeners.add(cb)
  return () => listeners.delete(cb)
}

export const usePanelState = () => React.useSyncExternalStore(subscribe, () => state)

/** Buka tab (atau fokuskan yang sudah ada dengan `key` sama). */
export function openPanelTab(tab: PanelTab) {
  const exists = state.tabs.some((t) => t.key === tab.key)
  set({ tabs: exists ? state.tabs : [...state.tabs, tab], active: tab.key, collapsed: false })
}

export function closePanelTab(key: string) {
  const tabs = state.tabs.filter((t) => t.key !== key)
  const active = state.active === key ? (tabs[tabs.length - 1]?.key ?? null) : state.active
  set({ tabs, active, maximized: tabs.length ? state.maximized : false })
}

export const activatePanelTab = (key: string) => set({ active: key, collapsed: false })
export const setPanelCollapsed = (collapsed: boolean) => set({ collapsed })
export const setPanelMaximized = (maximized: boolean) => set({ maximized, collapsed: false })

export function setPanelHeight(height: number) {
  set({ height })
  try {
    localStorage.setItem(HEIGHT_KEY, String(Math.round(height)))
  } catch {
    /* penyimpanan browser tidak tersedia */
  }
}

/** Tutup semua tab milik satu extension (dipanggil saat extension dimatikan, dihapus, atau dimuat ulang tanpa panelnya). */
export function pruneTabs(valid: (extId: string, panelId: string) => boolean) {
  const tabs = state.tabs.filter((t) => valid(t.extId, t.panelId))
  if (tabs.length !== state.tabs.length) set({ tabs, active: tabs.some((t) => t.key === state.active) ? state.active : (tabs[tabs.length - 1]?.key ?? null) })
}

export { findPanel }
export type { ExtPanelProps }
