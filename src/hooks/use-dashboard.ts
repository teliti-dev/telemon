import * as React from "react"

import { WIDGET_BY_TYPE } from "@/components/dashboard/registry"
import { DEFAULT_WIDGETS } from "@/lib/dashboard"
import { useStore } from "@/lib/store-context"
import type { DashboardWidget } from "@/lib/types"

/** Widget tersimpan yang jenisnya sudah tidak dikenal UI dibuang. */
function known(widgets: DashboardWidget[]) {
  return widgets.filter((w) => WIDGET_BY_TYPE.has(w.type))
}

/**
 * Tata letak widget Ringkasan (satu untuk semua akun). `widgets` null = sedang dimuat.
 * Gagal memuat atau belum ada tata letak tersimpan: susunan bawaan.
 */
export function useDashboard() {
  const { api } = useStore()
  const [widgets, setWidgets] = React.useState<DashboardWidget[] | null>(null)
  const [custom, setCustom] = React.useState(false)

  React.useEffect(() => {
    let cancelled = false
    api
      .getDashboard()
      .then((l) => {
        if (cancelled) return
        setCustom(!!l)
        setWidgets(l ? known(l.widgets) : DEFAULT_WIDGETS)
      })
      .catch(() => {
        if (cancelled) return
        setCustom(false)
        setWidgets(DEFAULT_WIDGETS)
      })
    return () => {
      cancelled = true
    }
  }, [api])

  /** Simpan untuk semua akun. Melempar galat dari server bila ditolak. */
  const save = React.useCallback(
    async (next: DashboardWidget[]) => {
      const saved = await api.saveDashboard({ version: 1, widgets: next })
      setCustom(!!saved)
      setWidgets(saved ? known(saved.widgets) : DEFAULT_WIDGETS)
    },
    [api]
  )

  /** Hapus tata letak tersimpan: semua akun kembali ke susunan bawaan. */
  const reset = React.useCallback(async () => {
    await api.saveDashboard(null)
    setCustom(false)
    setWidgets(DEFAULT_WIDGETS)
  }, [api])

  return { widgets, custom, save, reset }
}
