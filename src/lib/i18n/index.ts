/**
 * Dua bahasa: Indonesia (sumber teks) dan English.
 *
 * - `t("Teks Indonesia")` mengembalikan teks sesuai bahasa aktif. Kunci = teks Indonesia persis.
 * - Sisipan: `t("Dihapus {n} user", { n })`. Bentuk jamak English: tulis "{n} user|{n} users"
 *   di kamus (tunggal|jamak); dipilih saat n === 1.
 * - Konstanta di tingkat modul yang berisi teks tampilan memakai `tk("...")` (penanda kunci;
 *   mengembalikan teks apa adanya) lalu diterjemahkan saat dirender: `t(item.label)`.
 * - Komponen yang menampilkan teks memanggil `useT()` agar ikut render ulang saat bahasa berganti.
 */
import * as React from "react"

import { EN } from "./en"

export type Lang = "id" | "en"

const KEY = "telemon.lang"

function detect(): Lang {
  try {
    const v = localStorage.getItem(KEY)
    if (v === "id" || v === "en") return v
  } catch {
    /* penyimpanan browser tidak tersedia */
  }
  try {
    const nav = typeof navigator !== "undefined" ? navigator.language : undefined
    if (nav && !nav.toLowerCase().startsWith("id")) return "en"
  } catch {
    /* abaikan */
  }
  return "id"
}

let current: Lang = detect()
const listeners = new Set<() => void>()

export const getLang = () => current

export function setLang(lang: Lang) {
  if (lang === current) return
  current = lang
  try {
    localStorage.setItem(KEY, lang)
  } catch {
    /* pilihan hanya berlaku sesi ini */
  }
  if (typeof document !== "undefined") document.documentElement.lang = lang
  listeners.forEach((f) => f())
}

export function subscribeLang(fn: () => void) {
  listeners.add(fn)
  return () => {
    listeners.delete(fn)
  }
}

export function t(key: string, vars?: Record<string, string | number>): string {
  let s = current === "en" ? (EN[key] ?? key) : key
  if (current === "en" && vars && typeof vars.n === "number" && s.includes("|")) {
    const [one, many] = s.split("|")
    s = vars.n === 1 ? one : many
  }
  if (vars) s = s.replace(/\{(\w+)\}/g, (m, k: string) => (k in vars ? String(vars[k]) : m))
  return s
}

/** Penanda kunci terjemahan untuk teks yang disimpan di konstanta; mengembalikan teks apa adanya. */
export const tk = (key: string) => key

/** Locale untuk Intl dan tanggal. */
export const locale = () => (current === "en" ? "en-US" : "id-ID")

export function useLang(): Lang {
  return React.useSyncExternalStore(subscribeLang, getLang, () => "id" as Lang)
}

/** Mengembalikan `t` dan memastikan komponen render ulang saat bahasa berganti. */
export function useT() {
  useLang()
  return t
}

if (typeof document !== "undefined") document.documentElement.lang = current
