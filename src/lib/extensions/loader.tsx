import * as React from "react"

import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card"
import { ExtensionBoundary } from "@/components/extension-boundary"
import type { WidgetDef } from "@/components/dashboard/widget-types"
import { useStore } from "@/lib/store-context"
import { useT } from "@/lib/i18n"
import type { ActiveExtension } from "@/lib/types"
import { closePanelTab, openPanelTab, pruneTabs } from "./panel"
import { EXTENSION_API_VERSION, type ExtPageDef, type ExtPanelDef, type ExtRouterActionDef, type ExtWidgetDef, type ExtensionModule } from "./sdk"
import { PuzzleIcon, extWidgetType, markExtensionsReady, setExtensionWidgets, type ExtPageEntry, type PanelEntry, type RouterActionEntry } from "./registry"

const DEFAULT_SIZE = { w: 8, h: 60, minW: 4, minH: 24 }

function Failed({ name }: { name: string }) {
  const t = useT()
  return <p className="p-3 text-sm text-muted-foreground">{t("Extension {name} gagal ditampilkan", { name })}</p>
}

function extApi(extId: string) {
  async function call<T>(method: string, path: string, body?: unknown): Promise<T> {
    const res = await fetch(`/api/ext/${encodeURIComponent(extId)}${path}`, {
      method,
      credentials: "same-origin",
      headers: body === undefined ? {} : { "Content-Type": "application/json" },
      body: body === undefined ? undefined : JSON.stringify(body),
    })
    let data: unknown = null
    try {
      data = await res.json()
    } catch {
      /* badan kosong */
    }
    if (!res.ok) throw new Error((data as { error?: { message?: string } } | null)?.error?.message ?? `HTTP ${res.status}`)
    return data as T
  }
  return { get: <T,>(p: string) => call<T>("GET", p), post: <T,>(p: string, b?: unknown) => call<T>("POST", p, b) }
}

/** Aktifkan satu extension: impor modulnya dan kumpulkan widget yang didaftarkannya. */
async function activateOne(ext: ActiveExtension): Promise<{ defs: WidgetDef[]; pages: ExtPageEntry[]; actions: RouterActionEntry[]; panels: PanelEntry[] }> {
  const mod = (await import(/* @vite-ignore */ ext.entry)) as { default?: ExtensionModule } & Partial<ExtensionModule>
  const activate = mod.default?.activate ?? mod.activate
  if (typeof activate !== "function") throw new Error("no activate()") // i18n-ignore: galat pengembang, hanya di konsol
  const registered: ExtWidgetDef[] = []
  const registeredPages: ExtPageDef[] = []
  const registeredActions: ExtRouterActionDef[] = []
  const registeredPanels: ExtPanelDef[] = []
  await activate({
    apiVersion: EXTENSION_API_VERSION,
    extension: { id: ext.id, version: ext.version },
    React,
    ui: { Card, CardHeader, CardTitle, CardContent },
    hasPermission: (id) => ext.permissions.includes(id),
    accessLevel: ext.access.level,
    can: (id) => ext.access.actions.includes(id),
    api: extApi(ext.id),
    sessionUrl: (sessionId, params = {}) => {
      const q = new URLSearchParams(Object.entries(params).map(([k, v]) => [k, String(v)]))
      const proto = window.location.protocol === "https:" ? "wss:" : "ws:"
      return `${proto}//${window.location.host}/api/ext-session/${encodeURIComponent(ext.id)}/${encodeURIComponent(sessionId)}?${q}`
    },
    registerWidget: (d) => registered.push(d),
    registerPage: (d) => registeredPages.push(d),
    registerRouterAction: (d) => registeredActions.push(d),
    registerPanel: (d) => registeredPanels.push(d),
    panel: {
      open: (panelId, o) => openPanelTab({ key: `${ext.id}:${o.key}`, extId: ext.id, panelId, title: o.title, params: o.params ?? {} }),
      close: (key) => closePanelTab(`${ext.id}:${key}`),
    },
  })
  const defs: WidgetDef[] = []
  for (const decl of ext.widgets) {
    const r = registered.find((x) => x.type === decl.type)
    if (!r) continue // hanya jenis yang dideklarasikan manifest dan didaftarkan modul
    const size = { ...DEFAULT_SIZE, ...r.size }
    const Inner = r.Component
    defs.push({
      type: extWidgetType(ext.id, decl.type),
      title: decl.title,
      description: decl.description || ext.name,
      icon: PuzzleIcon,
      w: size.w,
      h: size.h,
      minW: size.minW,
      minH: size.minH,
      resizable: true,
      Component: ({ widget }) => (
        <ExtensionBoundary name={ext.name} fallback={(n) => <Failed name={n} />}>
          <Inner config={widget.config ?? {}} widgetId={widget.id} />
        </ExtensionBoundary>
      ),
    })
  }
  const pages: ExtPageEntry[] = []
  for (const decl of ext.pages) {
    const r = registeredPages.find((x) => x.id === decl.id)
    if (r) pages.push({ extId: ext.id, extName: ext.name, id: decl.id, title: decl.title, kind: decl.kind, Component: r.Component })
  }
  const actions: RouterActionEntry[] = []
  for (const decl of ext.routerActions ?? []) {
    const r = registeredActions.find((x) => x.id === decl.id)
    if (r) actions.push({ extId: ext.id, extName: ext.name, id: decl.id, title: decl.title, action: decl.action, can: (a) => ext.access.actions.includes(a), run: r.run, disabled: r.disabled })
  }
  const panels: PanelEntry[] = []
  for (const decl of ext.panels ?? []) {
    const r = registeredPanels.find((x) => x.id === decl.id)
    if (r) panels.push({ extId: ext.id, extName: ext.name, id: decl.id, title: decl.title, Component: r.Component })
  }
  return { defs, pages, actions, panels }
}

let loading: Promise<void> | null = null
let reloadRequested = false

/** Muat ulang semua extension aktif (dipanggil saat mulai dan setelah pengaturan Extension berubah). */
export function reloadExtensions(load: () => Promise<ActiveExtension[]>): Promise<void> {
  if (loading) {
    reloadRequested = true
    return loading
  }
  loading = (async () => {
    do {
      reloadRequested = false
      const list = await load().catch(() => [] as ActiveExtension[])
      const defs: WidgetDef[] = []
      const pages: ExtPageEntry[] = []
      const actions: RouterActionEntry[] = []
      const panels: PanelEntry[] = []
      for (const ext of list) {
        try {
          const r = await activateOne(ext)
          defs.push(...r.defs)
          pages.push(...r.pages)
          actions.push(...r.actions)
          panels.push(...r.panels)
        } catch (e) {
          console.error(`Extension ${ext.id} gagal dimuat`, e)
        }
      }
      setExtensionWidgets(defs, pages, actions, panels)
      // Tab panel milik extension yang dimatikan, dihapus, atau tak lagi punya panelnya ikut ditutup.
      pruneTabs((extId, panelId) => panels.some((p) => p.extId === extId && p.id === panelId))
    } while (reloadRequested)
  })().finally(() => {
    loading = null
  })
  return loading
}

/** Memuat extension aktif sekali saat aplikasi mulai (di dalam StoreProvider). */
export function ExtensionsLoader() {
  const { api } = useStore()
  React.useEffect(() => {
    reloadExtensions(() => api.getActiveExtensions()).catch(() => markExtensionsReady())
  }, [api])
  return null
}
