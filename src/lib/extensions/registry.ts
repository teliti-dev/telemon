import * as React from "react"
import { PuzzleIcon } from "lucide-react"

import { WIDGETS, WIDGET_BY_TYPE } from "@/components/dashboard/registry"
import type { WidgetDef } from "@/components/dashboard/widget-types"

/** Jenis widget extension di tata letak: `ext:<id>:<jenis>`. */
export const extWidgetType = (extId: string, type: string) => `ext:${extId}:${type}`
export const isExtWidget = (type: string) => type.startsWith("ext:")

interface State {
  ready: boolean
  version: number
}

let state: State = { ready: false, version: 0 }
const listeners = new Set<() => void>()

function set(next: Partial<State>) {
  state = { ...state, ...next }
  listeners.forEach((l) => l())
}

export interface ExtPageEntry {
  extId: string
  extName: string
  id: string
  title: string
  kind: "dashboard" | "page"
  Component: React.ComponentType<{ pageId: string }>
}

let pages: ExtPageEntry[] = []

export const extensionPages = () => pages
export function findExtensionPage(extId: string, pageId?: string) {
  const mine = pages.filter((p) => p.extId === extId)
  return (pageId ? mine.find((p) => p.id === pageId) : (mine.find((p) => p.kind === "dashboard") ?? mine[0])) ?? null
}

/** Ganti semua widget extension di registri Ringkasan dengan `defs`, lalu beri tahu pendengar. */
export function setExtensionWidgets(defs: WidgetDef[], nextPages: ExtPageEntry[] = []) {
  pages = nextPages
  for (let i = WIDGETS.length - 1; i >= 0; i--) {
    if (isExtWidget(WIDGETS[i].type)) {
      WIDGET_BY_TYPE.delete(WIDGETS[i].type)
      WIDGETS.splice(i, 1)
    }
  }
  for (const d of defs) {
    WIDGETS.push(d)
    WIDGET_BY_TYPE.set(d.type, d)
  }
  set({ ready: true, version: state.version + 1 })
}

export function markExtensionsReady() {
  if (!state.ready) set({ ready: true })
}

const subscribe = (cb: () => void) => {
  listeners.add(cb)
  return () => listeners.delete(cb)
}

/** `ready`: pemuatan awal selesai (berhasil atau tidak); `version` naik tiap daftar widget berubah. */
export function useExtensionState() {
  return React.useSyncExternalStore(subscribe, () => state)
}

export { PuzzleIcon }
