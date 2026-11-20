import * as React from "react"
import { CopyIcon, MinusIcon, SquareIcon, XIcon } from "lucide-react"

import { Button } from "@/components/ui/button"
import { useT } from "@/lib/i18n"
import { winClose, winIsMaximized, winMinimize, winToggleMaximize } from "@/lib/window"

function Btn({ label, onClick, className, children }: { label: string; onClick: () => void; className?: string; children: React.ReactNode }) {
  return (
    <Button type="button" variant="ghost" size="icon" aria-label={label} title={label} onClick={onClick} className={className}>
      {children}
    </Button>
  )
}

/** Tombol minimize, maximize/pulihkan, dan tutup gaya Windows di ujung kanan header (hanya mode tanpa bilah bawaan). */
export function WindowControls() {
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

  return (
    <div className="flex shrink-0 items-center gap-0.5 border-l py-2 pr-2 pl-2" data-window-controls>
      <Btn label={t("Perkecil jendela")} onClick={winMinimize}>
        <MinusIcon />
      </Btn>
      <Btn
        label={maximized ? t("Pulihkan jendela") : t("Perbesar jendela")}
        onClick={() => {
          winToggleMaximize()
          // Perbarui ikon segera; event resize dari jendela menyusul bila ukuran benar-benar berubah.
          window.setTimeout(() => winIsMaximized().then(setMaximized), 250)
        }}
      >
        {maximized ? <CopyIcon className="size-3.5 -scale-x-100" /> : <SquareIcon className="size-3.5" />}
      </Btn>
      <Btn label={t("Tutup jendela")} onClick={winClose} className="hover:bg-destructive/10 hover:text-destructive">
        <XIcon />
      </Btn>
    </div>
  )
}
