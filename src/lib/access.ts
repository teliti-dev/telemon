/**
 * Cara UI diakses:
 * - "desktop": aplikasi Tauri di komputer pemilik. Tanpa login; boleh menulis ke router (ADR 0004).
 * - "remote": browser lewat Serve LAN atau Publik. Wajib login; hanya baca.
 */
export type Access = "desktop" | "remote"

const PREVIEW_KEY = "telemon.preview.access"

export function detectAccess(): Access {
  if ("__TAURI_INTERNALS__" in window) return "desktop"
  try {
    if (sessionStorage.getItem(PREVIEW_KEY) === "desktop") return "desktop"
  } catch {
    /* penyimpanan browser tidak tersedia */
  }
  return "remote"
}

/** Hanya untuk pratinjau dengan data contoh: berpindah antara tampilan desktop dan jarak jauh. */
export function setPreviewAccess(access: Access) {
  try {
    if (access === "desktop") sessionStorage.setItem(PREVIEW_KEY, "desktop")
    else sessionStorage.removeItem(PREVIEW_KEY)
  } catch {
    /* abaikan */
  }
  window.location.reload()
}
