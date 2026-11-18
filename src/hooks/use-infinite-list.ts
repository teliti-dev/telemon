import * as React from "react"

/**
 * Daftar yang bertambah saat pengguna menggulir ke bawah (scroll tanpa henti).
 * Pasang `sentinelRef` pada elemen di akhir daftar; saat mendekati layar, daftar
 * ditambah satu halaman. `resetKey` berubah (filter, urutan) = kembali ke halaman pertama.
 */
export function useInfiniteList(total: number, pageSize: number, resetKey: unknown) {
  const [count, setCount] = React.useState(pageSize)
  const [prevKey, setPrevKey] = React.useState(resetKey)
  const sentinelRef = React.useRef<HTMLDivElement | null>(null)

  if (prevKey !== resetKey) {
    setPrevKey(resetKey)
    setCount(pageSize)
  }

  const hasMore = count < total

  React.useEffect(() => {
    const node = sentinelRef.current
    if (!node || !hasMore) return
    let timer: ReturnType<typeof setTimeout> | undefined
    const io = new IntersectionObserver(
      (entries) => {
        if (entries.some((e) => e.isIntersecting) && !timer) {
          // Jeda singkat supaya penambahan terasa mulus dan kerangka shimmer sempat terlihat.
          timer = setTimeout(() => {
            timer = undefined
            setCount((c) => Math.min(total, c + pageSize))
          }, 180)
        }
      },
      { rootMargin: "500px 0px" }
    )
    io.observe(node)
    return () => {
      io.disconnect()
      clearTimeout(timer)
    }
  }, [hasMore, count, total, pageSize])

  return { count, hasMore, sentinelRef }
}
