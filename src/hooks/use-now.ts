import * as React from "react"

/** Waktu sekarang yang diperbarui berkala, untuk teks relatif dan uptime. */
export function useNow(intervalMs = 15_000) {
  const [now, setNow] = React.useState(() => Date.now())
  React.useEffect(() => {
    const id = setInterval(() => setNow(Date.now()), intervalMs)
    return () => clearInterval(id)
  }, [intervalMs])
  return now
}
