import ReactGridLayout, { useContainerWidth, type LayoutItem } from "react-grid-layout"
import "react-grid-layout/css/styles.css"
import "react-resizable/css/styles.css"
import { XIcon } from "lucide-react"

import { WIDGET_BY_TYPE, type WidgetDef } from "@/components/dashboard/registry"
import { GRID_COLS, GRID_MARGIN, ROW_HEIGHT, readingOrder } from "@/lib/dashboard"
import { useT } from "@/lib/i18n"
import type { DashboardWidget } from "@/lib/types"
import { cn } from "@/lib/utils"

interface Props {
  widgets: DashboardWidget[]
  editing: boolean
  onChange: (widgets: DashboardWidget[]) => void
  onRemove: (id: string) => void
  onOpenUser: (id: string) => void
}

const px = (rows: number) => rows * ROW_HEIGHT + (rows - 1) * GRID_MARGIN[1]

function layoutItem(w: DashboardWidget, def: WidgetDef): LayoutItem {
  return {
    i: w.id,
    x: w.x,
    y: w.y,
    w: def.resizable ? w.w : def.w,
    h: def.resizable ? w.h : def.h,
    minW: def.resizable ? (def.minW ?? 1) : def.w,
    maxW: def.resizable ? undefined : def.w,
    minH: def.resizable ? (def.minH ?? 1) : def.h,
    maxH: def.resizable ? undefined : def.h,
    isResizable: def.resizable,
  }
}

/** Desktop dan tablet: grid yang bisa digeser dan diubah ukurannya saat mode ubah. */
export function DashboardGrid({ widgets, editing, onChange, onRemove, onOpenUser }: Props) {
  const t = useT()
  const { width, containerRef, mounted } = useContainerWidth()
  const known = widgets.filter((w) => WIDGET_BY_TYPE.has(w.type))
  const layout = known.map((w) => layoutItem(w, WIDGET_BY_TYPE.get(w.type)!))

  function onLayoutChange(next: readonly LayoutItem[]) {
    if (!editing) return
    let changed = false
    const updated = widgets.map((w) => {
      const it = next.find((l) => l.i === w.id)
      if (!it || (it.x === w.x && it.y === w.y && it.w === w.w && it.h === w.h)) return w
      changed = true
      return { ...w, x: it.x, y: it.y, w: it.w, h: it.h }
    })
    if (changed) onChange(updated)
  }

  return (
    <div ref={containerRef} className="w-full">
      {mounted && (
        <ReactGridLayout
          width={width}
          layout={layout}
          gridConfig={{ cols: GRID_COLS, rowHeight: ROW_HEIGHT, margin: GRID_MARGIN, containerPadding: [0, 0] }}
          dragConfig={{ enabled: editing, cancel: ".widget-nodrag" }}
          resizeConfig={{ enabled: editing }}
          onLayoutChange={onLayoutChange}
        >
          {known.map((w) => {
            const def = WIDGET_BY_TYPE.get(w.type)!
            return (
              <div key={w.id} className={cn("relative", editing && "cursor-grab rounded-xl outline-2 outline-dashed outline-border active:cursor-grabbing")}>
                <div className={cn("h-full min-h-0", editing && "pointer-events-none select-none")}>
                  <def.Component widget={w} onOpenUser={onOpenUser} />
                </div>
                {editing && (
                  <button
                    type="button"
                    aria-label={t("Hapus widget {name}", { name: t(def.title) /* i18n-ignore: kunci tk */ })}
                    title={t("Hapus widget")}
                    onClick={() => onRemove(w.id)}
                    onMouseDown={(e) => e.stopPropagation()}
                    onTouchStart={(e) => e.stopPropagation()}
                    className="widget-nodrag absolute top-1 right-1 z-10 flex size-6 items-center justify-center rounded-full border bg-background text-muted-foreground shadow-sm hover:bg-destructive hover:text-white focus-visible:ring-3 focus-visible:ring-ring/50"
                  >
                    <XIcon className="size-3.5" />
                  </button>
                )}
              </div>
            )
          })}
        </ReactGridLayout>
      )}
    </div>
  )
}

/** HP: satu kolom menurut urutan baca; kartu angka berurutan dikelompokkan dua per baris. */
export function DashboardStack({ widgets, onOpenUser }: Pick<Props, "widgets" | "onOpenUser">) {
  const ordered = readingOrder(widgets).filter((w) => WIDGET_BY_TYPE.has(w.type))
  const groups: { tile: boolean; items: DashboardWidget[] }[] = []
  for (const w of ordered) {
    const tile = !!WIDGET_BY_TYPE.get(w.type)!.tile
    const last = groups[groups.length - 1]
    if (tile && last?.tile) last.items.push(w)
    else groups.push({ tile, items: [w] })
  }
  return (
    <div className="flex flex-col gap-2 md:gap-3">
      {groups.map((g) =>
        g.tile ? (
          <div key={g.items[0].id} className="grid grid-cols-2 gap-2">
            {g.items.map((w, i) => {
              const def = WIDGET_BY_TYPE.get(w.type)!
              // Kartu pertama yang sendirian di baris mengisi lebar penuh.
              const lone = i === g.items.length - 1 && g.items.length % 2 === 1
              return (
                <div key={w.id} className={lone ? "col-span-2" : undefined} style={{ height: px(def.h) }}>
                  <def.Component widget={w} onOpenUser={onOpenUser} />
                </div>
              )
            })}
          </div>
        ) : (
          g.items.map((w) => {
            const def = WIDGET_BY_TYPE.get(w.type)!
            return (
              <div key={w.id} style={def.stackAuto ? undefined : { height: px(Math.min(w.h, 9)) }}>
                <def.Component widget={w} onOpenUser={onOpenUser} />
              </div>
            )
          })
        )
      )}
    </div>
  )
}
