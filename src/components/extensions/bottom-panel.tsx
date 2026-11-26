import * as React from "react"
import { ChevronDownIcon, ChevronUpIcon, Maximize2Icon, Minimize2Icon, PuzzleIcon, XIcon } from "lucide-react"

import { AI_DOCK_WIDTH } from "@/components/ai-chat-dock"
import { ExtensionBoundary } from "@/components/extension-boundary"
import { useAiChat } from "@/lib/ai-context"
import { activatePanelTab, closePanelTab, findPanel, pruneTabs, setPanelCollapsed, setPanelHeight, setPanelMaximized, usePanelState } from "@/lib/extensions/panel"
import { useT } from "@/lib/i18n"
import { cn } from "@/lib/utils"

const BAR = 36 // tinggi bilah tab (px)
const MIN_H = 160

/**
 * Panel bawah bertab seperti VS Code (ADR 0028): extension membuka tab (mis. terminal SSH ke satu router). Tab tetap hidup
 * saat berpindah tab atau panel dilipat, jadi sesi tidak putus. Tinggi bisa ditarik dari tepi atas dan diingat; Ctrl+` melipat
 * atau membuka. Di HP panel berada di atas menu bawah.
 */
export function BottomPanel() {
  const t = useT()
  const st = usePanelState()
  const ai = useAiChat()
  const dragging = React.useRef(false)
  const has = st.tabs.length > 0

  React.useEffect(() => {
    if (!has) return
    const onKey = (e: KeyboardEvent) => {
      if (e.ctrlKey && e.key === "`") {
        e.preventDefault()
        setPanelCollapsed(!st.collapsed)
      }
    }
    window.addEventListener("keydown", onKey)
    return () => window.removeEventListener("keydown", onKey)
  }, [has, st.collapsed])

  if (!has) return null
  const total = st.collapsed ? BAR : st.maximized ? undefined : st.height
  // Penahan ruang di alur halaman supaya isi halaman tidak tertutup panel.
  const spacer = st.collapsed ? BAR : st.maximized ? 0 : st.height

  function startDrag(e: React.PointerEvent) {
    e.preventDefault()
    dragging.current = true
    const bottomOffset = window.innerWidth < 768 ? 56 : 0
    const move = (ev: PointerEvent) => {
      if (!dragging.current) return
      const h = window.innerHeight - ev.clientY - bottomOffset
      setPanelHeight(Math.max(MIN_H, Math.min(h, window.innerHeight * 0.85)))
    }
    const up = () => {
      dragging.current = false
      window.removeEventListener("pointermove", move)
      window.removeEventListener("pointerup", up)
    }
    window.addEventListener("pointermove", move)
    window.addEventListener("pointerup", up)
  }

  return (
    <>
      <div aria-hidden style={{ height: spacer }} className="shrink-0" />
      <section
        aria-label={t("Panel bawah")}
        className="fixed bottom-14 left-0 z-40 flex flex-col border-t bg-background shadow-[0_-4px_12px_rgba(0,0,0,0.08)] md:bottom-0"
        style={{ right: ai.docked ? AI_DOCK_WIDTH : 0, height: total ?? "calc(100svh - 6.5rem)" }}
      >
        {!st.collapsed && !st.maximized && <div role="separator" aria-orientation="horizontal" aria-label={t("Ubah tinggi panel")} onPointerDown={startDrag} className="absolute inset-x-0 -top-1 z-10 h-2 cursor-row-resize hover:bg-brand/30" />}
        <div className="flex h-9 shrink-0 items-center gap-1 border-b px-1.5" role="tablist" aria-label={t("Tab panel")}>
          <div className="flex min-w-0 flex-1 items-center gap-0.5 overflow-x-auto">
            {st.tabs.map((tab) => {
              const on = tab.key === st.active
              return (
                <div key={tab.key} className={cn("flex h-9 shrink-0 items-center gap-1 border-b-2 pr-0.5 pl-2 text-sm", on ? "border-brand text-foreground" : "border-transparent text-muted-foreground hover:text-foreground")}>
                  <button type="button" role="tab" aria-selected={on} className="flex max-w-48 items-center gap-1.5 truncate outline-none focus-visible:underline" onClick={() => activatePanelTab(tab.key)}>
                    <PuzzleIcon className="size-3.5 shrink-0" />
                    <span className="truncate">{tab.title}</span>
                  </button>
                  <button type="button" aria-label={t("Tutup tab {title}", { title: tab.title })} className="rounded p-0.5 hover:bg-muted" onClick={() => closePanelTab(tab.key)}>
                    <XIcon className="size-3.5" />
                  </button>
                </div>
              )
            })}
          </div>
          <button type="button" aria-label={st.maximized ? t("Kecilkan panel") : t("Besarkan panel")} className="rounded p-1.5 text-muted-foreground hover:bg-muted" onClick={() => setPanelMaximized(!st.maximized)}>
            {st.maximized ? <Minimize2Icon className="size-4" /> : <Maximize2Icon className="size-4" />}
          </button>
          <button type="button" aria-label={st.collapsed ? t("Buka panel") : t("Lipat panel")} className="rounded p-1.5 text-muted-foreground hover:bg-muted" onClick={() => setPanelCollapsed(!st.collapsed)}>
            {st.collapsed ? <ChevronUpIcon className="size-4" /> : <ChevronDownIcon className="size-4" />}
          </button>
          <button type="button" aria-label={t("Tutup panel")} className="rounded p-1.5 text-muted-foreground hover:bg-muted" onClick={() => pruneTabs(() => false)}>
            <XIcon className="size-4" />
          </button>
        </div>
        <div className={cn("min-h-0 flex-1", st.collapsed && "hidden")}>
          {st.tabs.map((tab) => {
            const def = findPanel(tab.extId, tab.panelId)
            return (
              <div key={tab.key} className={cn("h-full overflow-auto", tab.key !== st.active && "hidden")}>
                {def ? (
                  <ExtensionBoundary name={def.extName} fallback={(n) => <p className="p-3 text-sm text-muted-foreground">{t("Extension {name} gagal ditampilkan", { name: n })}</p>}>
                    <def.Component params={tab.params} close={() => closePanelTab(tab.key)} />
                  </ExtensionBoundary>
                ) : null}
              </div>
            )
          })}
        </div>
      </section>
    </>
  )
}
