import * as React from "react"

type Report = (id: string, px: number) => void

const AutoHeightContext = React.createContext<Report | null>(null)
export const AutoHeightProvider = AutoHeightContext.Provider

/**
 * Untuk widget yang tingginya mengikuti isi: pasang ref yang dikembalikan pada elemen isi (tanpa tinggi tetap).
 * Grid memakai tinggi terukur itu sebagai tinggi widget; tanpa grid (tampilan HP) tidak melakukan apa-apa.
 */
export function useAutoHeight(id: string) {
  const report = React.useContext(AutoHeightContext)
  const observer = React.useRef<ResizeObserver | null>(null)
  return React.useCallback(
    (node: HTMLElement | null) => {
      observer.current?.disconnect()
      observer.current = null
      if (!node || !report) return
      // ResizeObserver melaporkan ukuran awal saat mulai mengamati, jadi tidak perlu pembacaan terpisah.
      const ro = new ResizeObserver(() => report(id, Math.ceil(node.getBoundingClientRect().height)))
      ro.observe(node)
      observer.current = ro
    },
    [id, report]
  )
}
