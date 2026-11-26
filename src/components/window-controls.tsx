import * as React from "react"
import { CopyIcon, MinusIcon, SquareIcon, XIcon } from "lucide-react"

import { Button } from "@/components/ui/button"
import { useT } from "@/lib/i18n"
import { cn } from "@/lib/utils"
import { winClose, winIsMaximized, winMinimize, winStartResize, winToggleMaximize, type ResizeDir } from "@/lib/window"

function Btn({ label, onClick, className, children, compact }: { label: string; onClick: () => void; className?: string; children: React.ReactNode; compact?: boolean }) {
  return (
    <Button type="button" variant="ghost" size="icon" aria-label={label} title={label} onClick={onClick} className={cn(compact && "size-6 md:size-6 [&_svg]:size-3.5", className)}>
      {children}
    </Button>
  )
}

/**
 * Tombol jendela di header (hanya mode tanpa bilah bawaan). `side="right"` = gaya Windows (perkecil, perbesar, tutup);
 * `side="left"` = gaya Cutefish/macOS (tutup, perkecil, perbesar) di ujung kiri.
 */
export function WindowControls({ side = "right", compact }: { side?: "left" | "right"; compact?: boolean }) {
  const t = useT()
  const [maximized, setMaximized] = React.useState(false)

  React.useEffect(() => {
    let alive = true
    let timer: number | undefined
    const refresh = () => {
      window.clearTimeout(timer)
      timer = window.setTimeout(() => winIsMaximized().then((m) => alive && setMaximized(m)), 120)
    }
    refresh()
    window.addEventListener("resize", refresh)
    return () => {
      alive = false
      window.clearTimeout(timer)
      window.removeEventListener("resize", refresh)
    }
  }, [])

  const min = (
    <Btn key="min" compact={compact} label={t("Perkecil jendela")} onClick={winMinimize}>
      <MinusIcon />
    </Btn>
  )
  const max = (
    <Btn
      key="max"
      compact={compact}
      label={maximized ? t("Pulihkan jendela") : t("Perbesar jendela")}
      onClick={() => {
        winToggleMaximize()
        // Perbarui ikon segera; event resize dari jendela menyusul bila ukuran benar-benar berubah.
        window.setTimeout(() => winIsMaximized().then(setMaximized), 250)
      }}
    >
      {maximized ? <CopyIcon className="size-3.5 -scale-x-100" /> : <SquareIcon className="size-3.5" />}
    </Btn>
  )
  const close = (
    <Btn key="close" compact={compact} label={t("Tutup jendela")} onClick={winClose} className="hover:bg-destructive/10 hover:text-destructive">
      <XIcon />
    </Btn>
  )

  return (
    <div className={cn("flex shrink-0 items-center gap-0.5", compact ? "py-1" : "py-2", side === "left" ? (compact ? "pr-2 pl-2" : "border-r pr-2 pl-2") : "border-l pr-2 pl-2")} data-window-controls>
      {side === "left" ? [close, min, max] : [min, max, close]}
    </div>
  )
}

const EDGE = 5
const HANDLES: { dir: ResizeDir; style: React.CSSProperties; cursor: string }[] = [
  { dir: "North", style: { top: 0, left: EDGE * 2, right: EDGE * 2, height: EDGE }, cursor: "ns-resize" },
  { dir: "South", style: { bottom: 0, left: EDGE * 2, right: EDGE * 2, height: EDGE }, cursor: "ns-resize" },
  { dir: "West", style: { left: 0, top: EDGE * 2, bottom: EDGE * 2, width: EDGE }, cursor: "ew-resize" },
  { dir: "East", style: { right: 0, top: EDGE * 2, bottom: EDGE * 2, width: EDGE }, cursor: "ew-resize" },
  { dir: "NorthWest", style: { top: 0, left: 0, width: EDGE * 2, height: EDGE * 2 }, cursor: "nwse-resize" },
  { dir: "NorthEast", style: { top: 0, right: 0, width: EDGE * 2, height: EDGE * 2 }, cursor: "nesw-resize" },
  { dir: "SouthWest", style: { bottom: 0, left: 0, width: EDGE * 2, height: EDGE * 2 }, cursor: "nesw-resize" },
  { dir: "SouthEast", style: { bottom: 0, right: 0, width: EDGE * 2, height: EDGE * 2 }, cursor: "nwse-resize" },
]

/** Tepi dan sudut tak terlihat untuk mengubah ukuran jendela tanpa bilah bawaan (Linux). */
export function ResizeHandles() {
  return (
    <>
      {HANDLES.map((h) => (
        <div
          key={h.dir}
          aria-hidden
          data-window-controls
          style={{ position: "fixed", zIndex: 100, cursor: h.cursor, ...h.style }}
          onMouseDown={(e) => {
            if (e.button !== 0) return
            e.preventDefault()
            winStartResize(h.dir)
          }}
        />
      ))}
    </>
  )
}
