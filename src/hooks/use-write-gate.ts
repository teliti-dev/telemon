import type { Access } from "@/lib/access"
import { useStore } from "@/lib/store-context"
import type { RouterInfo } from "@/lib/types"

/** Alasan router tidak bisa ditulis dari Telemon; null bila boleh. */
export function writeBlockReason(
  router: RouterInfo | undefined,
  access: Access
): string | null {
  if (access === "remote") return "Hanya bisa diubah dari aplikasi desktop"
  if (!router) return "Pilih router terlebih dulu"
  if (!router.canWrite) return `Akun API ${router.name} tidak punya hak write`
  if (router.status !== "online") {
    return `Router ${router.name} sedang ${router.status === "offline" ? "offline" : "error"}`
  }
  return null
}

export function useWriteGate() {
  const { routerById, access } = useStore()
  return (routerId?: string) =>
    writeBlockReason(routerId ? routerById.get(routerId) : undefined, access)
}
