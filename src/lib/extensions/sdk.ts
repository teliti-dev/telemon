import type * as React from "react"

/**
 * Kontrak antara Telemon dan modul UI extension (ADR 0028, apiVersion 1).
 *
 * Modul ES paket (`ui/index.js`) mengekspor default `{ activate(sdk) }`. Extension memakai React milik
 * aplikasi lewat `sdk.React` (jangan menyertakan salinan sendiri). Hanya extension **official** yang
 * dimuat langsung ke aplikasi; kodenya berjalan dengan sesi pengguna (bukan sandbox).
 *
 * Paket extension ada di repo terpisah; berkas ini adalah satu-satunya kontrak yang harus dijaga.
 */
export const EXTENSION_API_VERSION = 1

export interface ExtWidgetProps {
  /** Pengaturan widget ini (objek bebas, tersimpan di tata letak). */
  config: Record<string, unknown>
  /** Id widget di tata letak. */
  widgetId: string
}

export interface ExtWidgetDef {
  /** Harus cocok dengan salah satu `contributes.widgets[].type` di manifest. */
  type: string
  Component: React.ComponentType<ExtWidgetProps>
  /** Ukuran awal (kolom × baris grid); bawaan 8 × 60. */
  size?: { w: number; h: number; minW?: number; minH?: number }
}

export interface ExtensionSdk {
  apiVersion: typeof EXTENSION_API_VERSION
  extension: { id: string; version: string }
  /** React milik aplikasi: satu salinan untuk semua. */
  React: typeof React
  /** Komponen UI aplikasi yang boleh dipakai extension. */
  ui: {
    Card: React.ComponentType<React.ComponentProps<"div">>
    CardHeader: React.ComponentType<React.ComponentProps<"div">>
    CardTitle: React.ComponentType<React.ComponentProps<"div">>
    CardContent: React.ComponentType<React.ComponentProps<"div">>
  }
  /** Izin yang sedang diberikan admin (toggle di Pengaturan > Extension). */
  hasPermission(id: string): boolean
  registerWidget(def: ExtWidgetDef): void
}

export interface ExtensionModule {
  activate(sdk: ExtensionSdk): void | Promise<void>
}
