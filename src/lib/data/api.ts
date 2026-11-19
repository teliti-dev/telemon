import type {
  AccountSummary,
  AiChat,
  AiChatSummary,
  AiBudgetInput,
  AiDashboard,
  AiMessage,
  AiProvider,
  AiProviderInput,
  AiStreamEvent,
  AppSettings,
  PurgeCounts,
  Role,
  RoleInput,
  StorageInfo,
  ImportResult,
  LoginResult,
  SessionDevice,
  SessionInfo,
  TwoFactorSetup,
  NewRouter,
  PppProfile,
  ProfileInput,
  ProfilePatch,
  RouterEdit,
  UserInput,
  UserPatch,
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
  profiles: PppProfile[]
  events: PppEvent[]
  /** Catatan lokal per user, kunci = id user. */
  metas: Record<string, UserMeta>
  share: ShareState
}

export type LiveUpdate =
  | { kind: "event"; event: PppEvent; user?: PppUser; router?: RouterInfo }
  | { kind: "router"; router: RouterInfo }
  | { kind: "router-removed"; routerId: string }
  | { kind: "user"; user: PppUser }
  | { kind: "profile"; profile: PppProfile }
  /** Ganti seluruh user dan profil PPP milik satu router (setelah import). */
  | { kind: "inventory"; routerId: string; users: PppUser[]; profiles: PppProfile[] }
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
  /**
   * Ambil sidik jari sertifikat api-ssl router untuk dikonfirmasi pengguna (percaya-saat-pertama).
   * Hanya menyelesaikan handshake TLS; tidak mengirim kredensial.
   */
  fetchFingerprint(host: string, port: number): Promise<string>
  /**
   * Password PPPoE user bila tidak ikut di snapshot (server nyata: diambil saat ditampilkan dan tercatat
   * di riwayat). Adapter contoh menyertakannya di `PppUser.password`.
   */
  revealUserPassword?(userId: string): Promise<string>
  /** Ubah koneksi router. Username/password kosong = tetap. */
  updateRouter(id: string, input: RouterEdit): Promise<RouterInfo>
  /**
   * Tulis ke router (/ppp/secret). Mikrotik source of truth: setelah menulis,
   * state dibaca ulang dari router. Gagal bila router offline atau akun API
   * tanpa hak write.
   */
  createUser(input: UserInput): Promise<PppUser>
  updateUser(id: string, patch: UserPatch): Promise<PppUser>
  /** Soft delete: secret di router dinonaktifkan (tidak dihapus), ditandai terhapus di Telemon. */
  softDeleteUser(id: string): Promise<PppUser>
  restoreUser(id: string): Promise<PppUser>
  /** Tulis ke router (/ppp/profile). */
  createProfile(input: ProfileInput): Promise<PppProfile>
  updateProfile(id: string, patch: ProfilePatch): Promise<PppProfile>
  /** Soft delete lokal (profile RouterOS tidak punya flag disabled); ditolak bila masih dipakai user. */
  softDeleteProfile(id: string): Promise<PppProfile>
  restoreProfile(id: string): Promise<PppProfile>
  /** Baca /ppp/secret dan /ppp/profile router lalu samakan salinan Telemon (router menang). */
  importFromRouter(routerId: string): Promise<ImportResult>
  // ---- Login akses non-desktop (Serve LAN dan Publik). Aplikasi desktop tidak memakainya. ----
  /** Sesi saat ini, atau null bila belum login. */
  session(): Promise<SessionInfo | null>
  login(username: string, password: string): Promise<LoginResult>
  verify2fa(challengeId: string, code: string, opts?: { recovery?: boolean }): Promise<SessionInfo>
  /** Pendaftaran 2FA yang diwajibkan saat login; mengembalikan kode pemulihan. */
  complete2faSetup(
    challengeId: string,
    code: string
  ): Promise<{ session: SessionInfo; recoveryCodes: string[] }>
  logout(): Promise<void>
  // Akun sendiri
  updateMyProfile(input: { displayName: string }): Promise<SessionInfo>
  changeMyPassword(current: string, next: string): Promise<void>
  start2faSetup(): Promise<TwoFactorSetup>
  confirm2faSetup(code: string): Promise<{ recoveryCodes: string[] }>
  disable2fa(password: string): Promise<void>
  regenerateRecoveryCodes(password: string): Promise<string[]>
  listMySessions(): Promise<SessionDevice[]>
  revokeSession(id: string): Promise<void>
  // Kelola akun (hanya aplikasi desktop)
  listAccounts(): Promise<AccountSummary[]>
  createAccount(input: {
    username: string
    displayName: string
    password: string
    roleId: string
  }): Promise<AccountSummary>
  setAccountRole(id: string, roleId: string): Promise<AccountSummary>
  // Peran dan izin per menu (hanya aplikasi desktop)
  listRoles(): Promise<Role[]>
  createRole(input: RoleInput): Promise<Role>
  updateRole(id: string, input: RoleInput): Promise<Role>
  deleteRole(id: string): Promise<void>
  // Pengaturan (hanya aplikasi desktop)
  getSettings(): Promise<AppSettings>
  saveSettings(settings: AppSettings): Promise<AppSettings>
  // Dasbor AI. Kunci API write-only; server hanya mengembalikan petunjuk kunci.
  getAiDashboard(days: 7 | 30): Promise<AiDashboard>
  saveAiProvider(id: string | null, input: AiProviderInput): Promise<AiProvider>
  removeAiProvider(id: string): Promise<void>
  setDefaultAiProvider(id: string): Promise<void>
  testAiProvider(id: string): Promise<AiProvider>
  saveAiBudget(input: AiBudgetInput): Promise<void>
  getStorageInfo(): Promise<StorageInfo>
  purgeNow(): Promise<PurgeCounts>
  resetAccountPassword(id: string, password: string): Promise<void>
  setAccountDisabled(id: string, disabled: boolean): Promise<AccountSummary>
  resetAccount2fa(id: string): Promise<AccountSummary>
  deleteAccount(id: string): Promise<void>
  setRequire2fa(required: boolean): Promise<ShareState>
  /**
   * AI chat (hanya membaca): jawaban datang bertahap. Server menjawab dari data yang boleh
   * dibaca peran penanya dan menolak bila AI dimatikan atau peran tanpa akses.
   */
  askAi(messages: AiMessage[], opts?: { signal?: AbortSignal }): AsyncGenerator<AiStreamEvent>
  // Riwayat chat AI: disimpan di data Telemon per akun (bukan di browser), mengikuti retensi.
  listAiChats(): Promise<AiChatSummary[]>
  getAiChat(id: string): Promise<AiChat>
  saveAiChat(chat: AiChat): Promise<AiChatSummary>
  deleteAiChat(id: string): Promise<void>
  /** Menyimpan catatan lokal. Tidak menulis apa pun ke router. */
  saveUserMeta(userId: string, meta: UserMetaInput): Promise<UserMeta>
  setLanShare(opts: {
    enabled: boolean
    bind: ShareBind
    port: number
  }): Promise<ShareState>
  setPublicShare(opts: {
    enabled: boolean
    durationHours: number
  }): Promise<ShareState>
}

export type ApiKind = "mock" | "tauri" | "http"

