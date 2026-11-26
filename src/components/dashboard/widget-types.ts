import type * as React from "react"
import type { LucideIcon } from "lucide-react"

import type { DashboardWidget } from "@/lib/types"

export type WidgetConfig = Record<string, unknown>

export interface WidgetProps {
  widget: DashboardWidget
  onOpenUser: (id: string) => void
}

/** Formulir pengaturan satu widget (dibuka dari tombol roda di mode ubah). */
export interface ConfigProps {
  config: WidgetConfig
  onChange: (config: WidgetConfig) => void
}

export interface WidgetDef {
  type: string
  title: string
  description: string
  icon: LucideIcon
  /** Ukuran awal saat ditambahkan (kolom x baris). */
  w: number
  h: number
  minW?: number
  minH?: number
  /** false = ukuran tetap (kartu angka); true = bisa ditarik lebih besar atau kecil. */
  resizable: boolean
  /** Kartu angka kecil: di HP dua per baris. */
  tile?: boolean
  /** Di HP tinggi mengikuti isi (bukan tinggi grid). */
  stackAuto?: boolean
  /** Pengaturan awal saat widget ditambahkan. */
  defaultConfig?: WidgetConfig
  /** Ada formulir pengaturan (sumber data, rentang) bila diisi. */
  Config?: React.ComponentType<ConfigProps>
  Component: React.ComponentType<WidgetProps>
}
