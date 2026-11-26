import * as React from "react"

import { t } from "@/lib/i18n"
import { useStore } from "@/lib/store-context"
import type { TrafficPoint, TrafficQuery, TrafficRange, TrafficSeries } from "@/lib/types"

/** Antarmuka virtual: jumlah semua sesi PPPoE satu router (atau semua router bila router = `all`). */
export const PPPOE = "@pppoe"

export const RANGES: TrafficRange[] = ["1h", "6h", "24h", "7d", "30d"]

/** Dua arah yang digambar. PPPoE memakai sudut pandang pelanggan (unduh = `tx` router); antarmuka lain mengikuti router. */
export interface Directions {
  /** Deret utama (garis solid). */
  a: string
  b: string
  pick: (p: TrafficPoint) => { a: number; b: number }
}

export function directions(iface: string): Directions {
  if (iface === PPPOE) return { a: t("Unduh"), b: t("Unggah"), pick: (p) => ({ a: p.txBps, b: p.rxBps }) }
  return { a: t("Masuk (rx)"), b: t("Keluar (tx)"), pick: (p) => ({ a: p.rxBps, b: p.txBps }) }
}

export interface Stats {
  now: number
  avg: number
  peak: number
}

export function stats(values: number[]): Stats {
  if (values.length === 0) return { now: 0, avg: 0, peak: 0 }
  const sum = values.reduce((s, v) => s + v, 0)
  return { now: values[values.length - 1], avg: sum / values.length, peak: Math.max(...values) }
}

export function ifaceLabel(name: string) {
  return name === PPPOE ? t("Semua sesi PPPoE (pelanggan)") : name
}

export function rangeLabel(r: TrafficRange) {
  return { "1h": t("1 jam"), "6h": t("6 jam"), "24h": t("24 jam"), "7d": t("7 hari"), "30d": t("30 hari") }[r]
}

interface State {
  series?: TrafficSeries
  error?: string
  loading: boolean
}

interface Loaded {
  /** Kunci pertanyaan yang menghasilkan data ini; data milik pertanyaan lain dianggap belum ada. */
  key: string
  series?: TrafficSeries
  error?: string
}

/** Muat deret waktu dan segarkan sesuai interval pembacaan di Pengaturan (paling cepat 15 detik). */
export function useTrafficSeries(q: TrafficQuery): State {
  const { api } = useStore()
  const { router, iface, range } = q
  const key = `${router}|${iface}|${range}`
  const [loaded, setLoaded] = React.useState<Loaded | null>(null)
  React.useEffect(() => {
    let cancelled = false
    let timer: ReturnType<typeof setTimeout> | undefined
    const load = async () => {
      try {
        const series = await api.getTrafficSeries({ router, iface, range })
        if (cancelled) return
        setLoaded({ key, series })
        timer = setTimeout(() => void load(), Math.max(15, series.intervalSeconds) * 1000)
      } catch (e) {
        if (cancelled) return
        setLoaded({ key, error: e instanceof Error ? e.message : String(e) })
        timer = setTimeout(() => void load(), 60_000)
      }
    }
    void load()
    return () => {
      cancelled = true
      if (timer) clearTimeout(timer)
    }
  }, [api, router, iface, range, key])
  // Hasil milik sumber lain (router, antarmuka, atau rentang berganti) tidak ditampilkan: dianggap sedang dimuat.
  if (!loaded || loaded.key !== key) return { loading: true }
  return { series: loaded.series, error: loaded.error, loading: false }
}

/** Batas sumbu "enak dibaca" di atas nilai maksimum: 1, 2, 2.5, 5 atau 10 kali pangkat sepuluh. */
export function niceMax(v: number) {
  if (v <= 0) return 1000
  const p = 10 ** Math.floor(Math.log10(v))
  for (const m of [1, 2, 2.5, 5, 10]) if (v <= m * p) return m * p
  return 10 * p
}
