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

export interface ExtPageProps {
  /** Id halaman (`contributes.pages[].id` di manifest). */
  pageId: string
}

export interface ExtPageDef {
  /** Harus cocok dengan salah satu `contributes.pages[].id` di manifest. */
  id: string
  Component: React.ComponentType<ExtPageProps>
}

/** Router yang dipilih pengguna saat menjalankan aksi router. */
export interface ExtRouterRef {
  id: string
  name: string
  /** Router boleh ditulis dari Telemon (hak write dan diizinkan pemilik). */
  canWrite: boolean
  status: string
}

export interface ExtRouterActionDef {
  /** Harus cocok dengan salah satu `contributes.routerActions[].id` di manifest (judulnya dari manifest). */
  id: string
  run(router: ExtRouterRef): void
  /** Alasan entri dinonaktifkan untuk router ini (teks untuk pengguna), atau `null` bila bisa dijalankan. */
  disabled?(router: ExtRouterRef): string | null
}

export interface ExtPanelProps {
  /** Parameter yang diberikan saat panel dibuka (`sdk.panel.open`), mis. `{ router, name }`. */
  params: Record<string, unknown>
  /** Tutup tab ini. */
  close(): void
}

export interface ExtPanelDef {
  /** Harus cocok dengan salah satu `contributes.panels[].id` di manifest. */
  id: string
  Component: React.ComponentType<ExtPanelProps>
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
  /** Level akses peran akun ini pada extension (halaman Akses): none, read, atau update. */
  accessLevel: "none" | "read" | "update"
  /** Apakah peran akun ini boleh melakukan aksi yang dideklarasikan manifest (`actions[]`). */
  can(actionId: string): boolean
  /**
   * Rute API backend extension ini (`/api/ext/<id><path>`), memakai sesi pengguna. Mengembalikan `body` jawaban
   * backend; status bukan 2xx melempar Error dengan pesan dari server.
   */
  api: {
    get<T = unknown>(path: string): Promise<T>
    post<T = unknown>(path: string, body?: unknown): Promise<T>
  }
  /**
   * Alamat WebSocket sesi interaktif yang dideklarasikan manifest (`contributes.sessions`), mis. terminal SSH:
   * `new WebSocket(sdk.sessionUrl("ssh", { router, cols, rows }))`. Core menegakkan izin, pin kunci host, dan batas.
   */
  sessionUrl(sessionId: string, params?: Record<string, string | number>): string
  /**
   * Panel bawah bertab seperti VS Code. `open` membuka satu tab (atau memfokuskan tab dengan `key` yang sama); komponen
   * panel tetap hidup selama tab terbuka (mis. sesi terminal tidak putus saat pindah tab).
   */
  panel: {
    open(panelId: string, opts: { key: string; title: string; params?: Record<string, unknown> }): void
    close(key: string): void
  }
  registerRouterAction(def: ExtRouterActionDef): void
  registerPanel(def: ExtPanelDef): void
  registerWidget(def: ExtWidgetDef): void
  registerPage(def: ExtPageDef): void
}

export interface ExtensionModule {
  activate(sdk: ExtensionSdk): void | Promise<void>
}
