const norm = (s: string) => s.toLowerCase().normalize("NFD").replace(/[̀-ͯ]/g, "")

function subsequence(needle: string, hay: string) {
  let i = 0
  for (const c of hay) if (c === needle[i] && ++i === needle.length) return true
  return false
}

/**
 * Nilai kecocokan `query` terhadap beberapa kolom (kolom pertama paling berbobot); 0 = tidak cocok.
 * Setiap kata di `query` harus cocok dengan salah satu kolom: persis > awal kata > mengandung > urutan huruf (kata >= 3 huruf).
 */
export function rank(query: string, fields: (string | undefined)[]): number {
  const tokens = norm(query).split(/\s+/).filter(Boolean)
  if (tokens.length === 0) return 1
  const hay = fields.filter((f): f is string => !!f).map(norm)
  let total = 0
  for (const tok of tokens) {
    let best = 0
    hay.forEach((f, i) => {
      const w = i === 0 ? 1 : 0.6
      let s = 0
      if (f === tok) s = 5
      else if (f.startsWith(tok)) s = 4
      else if (new RegExp(`[\\s\\-_./:@]${tok.replace(/[^a-z0-9]/g, "\\$&")}`).test(f)) s = 3
      else if (f.includes(tok)) s = 2
      // Urutan huruf (samar) hanya untuk judul: pada kata kunci tambahan menghasilkan terlalu banyak kecocokan semu.
      else if (i === 0 && tok.length >= 3 && f.length <= 40 && subsequence(tok, f)) s = 1
      best = Math.max(best, s * w)
    })
    if (best === 0) return 0
    total += best
  }
  return total
}
