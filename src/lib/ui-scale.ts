/**
 * Ukuran tampilan yang bisa dipilih pengguna. Menskalakan font root (16px = 100%),
 * jadi seluruh ukuran berbasis rem ikut berubah, bukan hanya teks.
 */
import { tk } from "./i18n"

const KEY = "telemon.ui.scale"

export const SCALES = [
  { value: "90", label: tk("Kecil") },
  { value: "100", label: tk("Normal") },
  { value: "112", label: tk("Besar") },
  { value: "125", label: tk("Sangat besar") },
] as const

export type Scale = (typeof SCALES)[number]["value"]

export function getScale(): Scale {
  try {
    const v = localStorage.getItem(KEY)
    if (SCALES.some((s) => s.value === v)) return v as Scale
  } catch {
    /* penyimpanan browser tidak tersedia */
  }
  return "100"
}

export function applyScale(scale: Scale) {
  document.documentElement.style.fontSize = `${scale}%`
}

export function setScale(scale: Scale) {
  try {
    localStorage.setItem(KEY, scale)
  } catch {
    /* pilihan hanya berlaku sesi ini */
  }
  applyScale(scale)
}
