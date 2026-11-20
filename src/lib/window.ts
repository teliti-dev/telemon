/**
 * Jendela tanpa bilah judul bawaan (hanya Windows, hanya aplikasi desktop).
 *
 * Aplikasi mulai dengan bilah bawaan. UI menghilangkannya sendiri hanya bila jembatan IPC ke jendela terbukti
 * berfungsi, sehingga bila izin Tauri tidak cocok jendela tetap punya tombol tutup. IPC dibuka sempit: hanya
 * perintah jendela untuk origin loopback (src-tauri/capabilities/window-controls.json).
 */
import * as React from "react"

type Invoke = (cmd: string, args?: Record<string, unknown>) => Promise<unknown>

function invokeFn(): Invoke | undefined {
  return (window as unknown as { __TAURI_INTERNALS__?: { invoke?: Invoke } }).__TAURI_INTERNALS__?.invoke
}

const LABEL = "main"
const call = (cmd: string, extra: Record<string, unknown> = {}) => {
  const invoke = invokeFn()
  return invoke ? invoke(`plugin:window|${cmd}`, { label: LABEL, ...extra }) : Promise.reject(new Error("no IPC")) /* i18n-ignore: galat internal, tidak ditampilkan */
}

let frameless = false
const listeners = new Set<() => void>()
const emit = () => listeners.forEach((l) => l())

/** Coba hilangkan bilah judul bawaan; true bila berhasil. Aman dipanggil berulang. */
let started = false
export function initFrameless() {
  if (started || !invokeFn() || !/Windows/i.test(navigator.userAgent)) return
  started = true
  call("set_decorations", { value: false })
    .then(() => {
      frameless = true
      emit()
    })
    .catch(() => {
      /* IPC tidak diizinkan: tetap pakai bilah bawaan */
    })
}

export function useFrameless() {
  return React.useSyncExternalStore(
    (cb) => {
      listeners.add(cb)
      return () => listeners.delete(cb)
    },
    () => frameless,
    () => false
  )
}

export const winMinimize = () => void call("minimize").catch(() => {})
export const winToggleMaximize = () => void call("toggle_maximize").catch(() => {})
export const winClose = () => void call("close").catch(() => {})
export const winStartDragging = () => void call("start_dragging").catch(() => {})
export const winIsMaximized = () => call("is_maximized").then((v) => v === true).catch(() => false)
