import type { ApiKind } from "@/lib/data/api"
import { useStore } from "@/lib/store-context"
import type { RouterInfo } from "@/lib/types"

/** Alasan router tidak bisa ditulis dari Telemon; null bila boleh. */
export function writeBlockReason(
  router: RouterInfo | undefined,
  apiKind: ApiKind
): string | null {
  if (apiKind === "http") return "Hanya bisa diubah dari aplikasi desktop"
  if (!router) return "Pilih router terlebih dulu"
  if (!router.canWrite) return `Akun API ${router.name} tidak punya hak write`
  if (router.status !== "online") {
    return `Router ${router.name} sedang ${router.status === "offline" ? "offline" : "error"}`
  }
  return null
}

export function useWriteGate() {
  const { routerById, api } = useStore()
  return (routerId?: string) =>
    writeBlockReason(routerId ? routerById.get(routerId) : undefined, api.kind)
}
