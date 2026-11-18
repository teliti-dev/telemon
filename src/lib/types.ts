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
}

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
    token?: string
  }
  publik: {
    status: "off" | "connecting" | "on" | "error"
    url?: string
    pinSet: boolean
    durationHours: number
    expiresAt?: number
    error?: string
  }
}

export interface NewRouter {
  name: string
  host: string
  port: number
  tls: boolean
  username: string
  password: string
}
