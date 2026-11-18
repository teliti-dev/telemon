import type {
  NewRouter,
  PppEvent,
  PppUser,
  RouterInfo,
  ShareBind,
  ShareState,
  UserMeta,
  UserMetaInput,
} from "../types"

export interface Snapshot {
  routers: RouterInfo[]
  users: PppUser[]
  events: PppEvent[]
  /** Catatan lokal per user, kunci = id user. */
  metas: Record<string, UserMeta>
  share: ShareState
}

export type LiveUpdate =
  | { kind: "event"; event: PppEvent; user?: PppUser; router?: RouterInfo }
  | { kind: "router"; router: RouterInfo }
  | { kind: "router-removed"; routerId: string }
  | { kind: "user-meta"; userId: string; meta: UserMeta }
  | { kind: "share"; share: ShareState }

/**
 * Satu kontrak data untuk semua lingkungan:
 * - "tauri": aplikasi desktop, lewat command/event Tauri.
 * - "http":  dibuka dari browser lewat mode serve/publik (JSON + WebSocket).
 * - "mock":  data contoh untuk pengembangan UI.
 */
export interface TelemonApi {
  readonly kind: ApiKind
  snapshot(): Promise<Snapshot>
  subscribe(onUpdate: (u: LiveUpdate) => void): () => void
  addRouter(input: NewRouter): Promise<RouterInfo>
  removeRouter(id: string): Promise<void>
  /** Menyimpan catatan lokal. Tidak menulis apa pun ke router. */
  saveUserMeta(userId: string, meta: UserMetaInput): Promise<UserMeta>
  setLanShare(opts: {
    enabled: boolean
    bind: ShareBind
    port: number
  }): Promise<ShareState>
  regenerateToken(): Promise<ShareState>
  setPublicPin(pin: string): Promise<ShareState>
  setPublicShare(opts: {
    enabled: boolean
    durationHours: number
  }): Promise<ShareState>
}

export type ApiKind = "mock" | "tauri" | "http"

/** Lingkungan tempat UI berjalan (bukan sumber datanya). */
export function detectRuntime(): "tauri" | "browser" {
  return "__TAURI_INTERNALS__" in window ? "tauri" : "browser"
}
