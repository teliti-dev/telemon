import type { Access } from "@/lib/access"
import { useAuth } from "@/lib/auth-context"
import { t } from "@/lib/i18n"
import { RESOURCES } from "@/lib/permissions"
import { useStore } from "@/lib/store-context"
import type { Resource, RouterInfo, SessionInfo, ShareState } from "@/lib/types"

/**
 * Alasan peran akun tidak boleh melakukan sesuatu; null bila boleh.
 * Aplikasi desktop tidak dibatasi peran. Sesi jarak jauh mengikuti izin per menu.
 */
export function permissionReason(opts: {
  access: Access
  session: SessionInfo | null
  share: ShareState | undefined
  resource: Resource
  need: "read" | "update"
}): string | null {
  const { access, session, share, resource, need } = opts
  if (access === "desktop") return null
  const label = t(RESOURCES.find((r) => r.key === resource)?.label ?? resource) // i18n-ignore: etiket sudah berupa kunci (tk)
  const level = session?.permissions[resource] ?? "none"
  if (level === "none") return t("Peran Anda tidak punya akses ke {label}", { label })
  if (need === "update") {
    if (level !== "update") return t("Peran {role} hanya bisa membaca {label}", { role: session?.roleName ?? "", label })
    if (session?.via === "publik" && !share?.auth.allowPublicWrite) {
      return t("Tulis lewat akses publik dimatikan oleh pemilik Telemon")
    }
  }
  return null
}

/** Alasan router tidak bisa ditulis (hak write akun API dan status online). */
export function routerWriteReason(router: RouterInfo | undefined): string | null {
  if (!router) return t("Pilih router terlebih dulu")
  if (!router.canWrite) return t("Akun API {name} tidak punya hak write", { name: router.name })
  if (router.status !== "online") {
    return router.status === "offline"
      ? t("Router {name} sedang offline", { name: router.name })
      : t("Router {name} sedang error", { name: router.name })
  }
  return null
}

/** Izin Ubah pada satu menu (tanpa melihat router). Mengembalikan fungsi alasan. */
export function useUpdateGate(resource: Resource) {
  const { access, share } = useStore()
  const { session } = useAuth()
  return () => permissionReason({ access, session, share, resource, need: "update" })
}

/** Izin Ubah pada satu menu ditambah syarat router (hak write dan online). */
export function useWriteGate(resource: Resource) {
  const { routerById } = useStore()
  const update = useUpdateGate(resource)
  return (routerId?: string) => update() ?? routerWriteReason(routerId ? routerById.get(routerId) : undefined)
}
