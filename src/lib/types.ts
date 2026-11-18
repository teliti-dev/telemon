export type RouterStatus = "online" | "offline" | "error"

export interface RouterInfo {
  id: string
  name: string
  host: string
  port: number
  tls: boolean
  model: string
  version: string
  status: RouterStatus
  /** Waktu polling terakhir yang berhasil (epoch ms). */
  lastPollAt?: number
  /** Akun API router punya hak write, jadi user/profile boleh diedit dari Telemon. */
  canWrite: boolean
  /** Import user/profile terakhir (epoch ms). */
  lastImportAt?: number
  error?: string
}

export type UserStatus = "active" | "inactive" | "disabled"

export interface PppUser {
  id: string
  routerId: string
  username: string
  profile: string
  status: UserStatus
  address?: string
  callerId?: string
  /** Awal sesi aktif (epoch ms), hanya bila status active. */
  connectedAt?: number
  lastSeenAt?: number
  lastEvent?: PppEventType
  lastEventAt?: number
  comment?: string
  /** Remote address statis di secret (opsional). */
  staticAddress?: string
  /** Soft delete: secret di router dinonaktifkan, data tetap ada di Telemon. */
  deletedAt?: number
  deletedReason?: DeletedReason
}

export type DeletedReason = "manual" | "router"

export type OnlyOne = "default" | "yes" | "no"

export interface PppProfile {
  /** "routerId:name" */
  id: string
  routerId: string
  name: string
  /** Format RouterOS rx/tx, mis. "10M/10M". */
  rateLimit?: string
  localAddress?: string
  /** Nama pool atau alamat. */
  remoteAddress?: string
  onlyOne: OnlyOne
  comment?: string
  deletedAt?: number
  deletedReason?: DeletedReason
}

/** Input tulis ke router (/ppp/secret). Password tidak pernah dibaca balik dari router. */
export interface UserInput {
  routerId: string
  username: string
  password?: string
  profile: string
  staticAddress?: string
  comment?: string
  disabled: boolean
}
export type UserPatch = Partial<Omit<UserInput, "routerId" | "username">>

/** Input tulis ke router (/ppp/profile). */
export interface ProfileInput {
  routerId: string
  name: string
  rateLimit?: string
  localAddress?: string
  remoteAddress?: string
  onlyOne: OnlyOne
  comment?: string
}
export type ProfilePatch = Partial<Omit<ProfileInput, "routerId" | "name">>

export interface ImportCounts {
  added: number
  updated: number
  /** Hilang dari router, ditandai soft delete di Telemon. */
  deleted: number
}

export interface ImportResult {
  routerId: string
  at: number
  users: ImportCounts
  profiles: ImportCounts
}

/**
 * Catatan lokal per user, disimpan di Telemon (bukan di router).
 * Kunci: id user ("routerId:username").
 */
export interface UserMeta {
  customerName?: string
  phone?: string
  note?: string
  tags: string[]
  updatedAt: number
}

export type UserMetaInput = Omit<UserMeta, "updatedAt">

export type PppEventType =
  | "auth_failed"
  | "authenticated"
  | "connected"
  | "disconnected"
  | "router_down"
  | "router_up"

export interface PppEvent {
  id: string
  at: number
  routerId: string
  type: PppEventType
  username?: string
  address?: string
  callerId?: string
  /** Baris log asli dari router (atau ringkasan dari Telemon). */
  message: string
}

export type ShareBind = "lan" | "localhost"

export interface ShareState {
  lan: {
    enabled: boolean
    bind: ShareBind
    port: number
    url?: string
  }
  publik: {
    status: "off" | "connecting" | "on" | "error"
    url?: string
    durationHours: number
    expiresAt?: number
    error?: string
  }
  auth: ShareAuth
}

export interface ShareAuth {
  /** Wajib 2FA untuk semua akun jarak jauh (otomatis wajib saat akses Publik aktif). */
  require2fa: boolean
}

/** Akun untuk akses non-desktop (browser lewat Serve LAN dan Publik). Semua akun hanya baca. */
export interface AccountSummary {
  id: string
  username: string
  displayName: string
  twoFactor: boolean
  disabled: boolean
  createdAt: number
  lastLoginAt?: number
}

export interface SessionInfo {
  id: string
  userId: string
  username: string
  displayName: string
  twoFactor: boolean
}

export interface SessionDevice {
  id: string
  device: string
  ip: string
  createdAt: number
  lastSeenAt: number
  current: boolean
}

export type LoginResult =
  | { status: "ok"; session: SessionInfo }
  | { status: "2fa_required"; challengeId: string }
  | { status: "2fa_setup_required"; challengeId: string; secret: string; otpauthUrl: string }

export interface TwoFactorSetup {
  secret: string
  otpauthUrl: string
}

export interface NewRouter {
  /** Izinkan edit user/profile (akun API punya hak write). */
  allowWrite: boolean
  name: string
  host: string
  port: number
  tls: boolean
  username: string
  password: string
}
