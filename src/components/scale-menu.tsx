import * as React from "react"
import { ZoomInIcon } from "lucide-react"

import { Button } from "@/components/ui/button"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuLabel,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import { getScale, SCALES, setScale, type Scale } from "@/lib/ui-scale"

/** Pilihan ukuran tampilan (90-125%); diingat di browser. */
export function ScaleMenu({ className }: { className?: string }) {
  const [scale, setScaleState] = React.useState<Scale>(getScale)
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button variant="ghost" size="icon" className={className} aria-label="Ukuran tampilan" title="Ukuran tampilan">
          <ZoomInIcon />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="w-44">
        <DropdownMenuLabel>Ukuran tampilan</DropdownMenuLabel>
        <DropdownMenuSeparator />
        <DropdownMenuRadioGroup
          value={scale}
          onValueChange={(v) => {
            setScaleState(v as Scale)
            setScale(v as Scale)
          }}
        >
          {SCALES.map((s) => (
            <DropdownMenuRadioItem key={s.value} value={s.value}>
              {s.label} <span className="ml-auto text-xs text-muted-foreground">{s.value}%</span>
            </DropdownMenuRadioItem>
          ))}
        </DropdownMenuRadioGroup>
      </DropdownMenuContent>
    </DropdownMenu>
  )
}
