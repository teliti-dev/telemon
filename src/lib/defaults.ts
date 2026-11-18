/**
 * Nilai bawaan Telemon: pengaturan dan peran. Satu sumber untuk adapter data dan UI
 * ("Kembalikan ke bawaan"). Dicatat juga di ADR 0006.
 */
import type { AppSettings, Permissions } from "./types"

export const DEFAULT_SETTINGS: AppSettings = {
  retention: {
    eventDays: 30,
    maxEvents: 200_000,
    deletedDays: 90,
    auditDays: 365,
  },
  polling: {
    sessionSeconds: 30,
    logSeconds: 30,
    autoImportHours: 24,
    timeoutSeconds: 10,
    retries: 2,
  },
  remote: {
    sessionHours: 12,
    maxFailures: 5,
    lockMinutes: 5,
    allowPublicWrite: false,
  },
}

export interface DefaultRole {
  id: string
  name: string
  description: string
  permissions: Permissions
}

/** Peran bawaan dan izin menunya. Ringkasan dan Event hanya punya Tanpa akses atau Baca. */
export const DEFAULT_ROLES: DefaultRole[] = [
  {
    id: "role-admin",
    name: "Administrator",
    description: "Semua menu, termasuk Admin dan Pengaturan; boleh mengubah semuanya.",
    permissions: { ringkasan: "read", user: "update", profile: "update", event: "read", router: "update", admin: "update", pengaturan: "update" },
  },
  {
    id: "role-operator",
    name: "Operator",
    description: "Mengelola user dan profil PPP; router hanya dilihat.",
    permissions: { ringkasan: "read", user: "update", profile: "update", event: "read", router: "read", admin: "none", pengaturan: "none" },
  },
  {
    id: "role-viewer",
    name: "Viewer",
    description: "Hanya melihat semua menu.",
    permissions: { ringkasan: "read", user: "read", profile: "read", event: "read", router: "read", admin: "none", pengaturan: "none" },
  },
  {
    id: "role-monitor",
    name: "Monitor",
    description: "Layar pantau: Ringkasan, Event, dan Router saja.",
    permissions: { ringkasan: "read", user: "none", profile: "none", event: "read", router: "read", admin: "none", pengaturan: "none" },
  },
]
