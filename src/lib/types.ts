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
  /** Akun API router punya hak write, jadi user/profil PPP boleh diedit dari Telemon. */
  canWrite: boolean
  /** Import user/profil PPP terakhir (epoch ms). */
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
  /** Password secret PPPoE. Hanya untuk yang berizin Ubah pada Users; jangan dicatat di log. */
  password?: string
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
  /** Fitur yang dinyalakan pemilik; diketahui semua klien. */
  features: { ai: boolean; aiMask: boolean }
}

export interface ShareAuth {
  /** Wajib 2FA untuk semua akun jarak jauh (otomatis wajib saat akses Publik aktif). */
  require2fa: boolean
  /** Peran dengan izin Ubah boleh menulis lewat akses Publik (bawaan mati). */
  allowPublicWrite: boolean
}

/** Menu yang bisa diatur izinnya per peran. Berbagi (menyalakan serve/publik) hanya dari aplikasi desktop. */
export type Resource = "ringkasan" | "user" | "profile" | "event" | "router" | "admin" | "pengaturan" | "ai"

/** Tanpa akses (bawaan), lihat, atau ubah (tambah, edit, soft delete, pulihkan, import). Ubah mencakup lihat. */
export type Level = "none" | "read" | "update"

export type Permissions = Record<Resource, Level>

export interface Role {
  id: string
  name: string
  description?: string
  permissions: Permissions
  accountCount: number
  /** Peran bawaan: tidak bisa dihapus, izinnya bisa dikembalikan ke bawaan. */
  builtin: boolean
}

export interface RoleInput {
  name: string
  description?: string
  permissions: Permissions
}

/** Tautan ke halaman Telemon (hash route), mis. { page: "event", params: { type: "auth_failed" } }. */
export interface AiLink {
  page: string
  params?: Record<string, string>
}

export type AiTone = "good" | "warn" | "bad" | "muted"

/** Komponen visual di dalam jawaban AI. Berisi data jadi, bukan HTML. */
export type AiWidget =
  | { kind: "stats"; items: { label: string; value: string; tone?: AiTone }[] }
  | { kind: "bars"; title: string; data: { label: string; value: number }[] }
  | { kind: "list"; title?: string; rows: { primary: string; secondary?: string; trailing?: string; tone?: AiTone; to?: AiLink }[] }
  | { kind: "user"; username: string; router: string; status: string; tone: AiTone; fields: { label: string; value: string }[]; to: AiLink }

/** Tombol pintasan di bawah jawaban: buka halaman, atau salin teks. */
export interface AiAction {
  id: string
  label: string
  to?: AiLink
  copy?: string
}

export interface AiMessage {
  id: string
  role: "user" | "assistant"
  text: string
  at: number
  /** Jawaban gagal dibuat; text berisi alasannya. */
  error?: boolean
  widgets?: AiWidget[]
  actions?: AiAction[]
  /** Saran pertanyaan lanjutan (kunci terjemahan atau teks jadi). */
  followUps?: string[]
  /** Data yang dipakai AI untuk menjawab. */
  sources?: string[]
  /** Langkah yang dikerjakan AI (transparansi). */
  steps?: string[]
  feedback?: "up" | "down"
}

/** Potongan jawaban yang dialirkan bertahap. */
export type AiStreamEvent =
  | { type: "step"; text: string }
  | { type: "text"; text: string }
  | { type: "widget"; widget: AiWidget }
  | { type: "actions"; actions: AiAction[] }
  | { type: "followups"; items: string[] }
  | { type: "sources"; items: string[] }

export interface AiChat {
  id: string
  title: string
  pinned: boolean
  createdAt: number
  updatedAt: number
  messages: AiMessage[]
}

export interface AiChatSummary {
  id: string
  title: string
  pinned: boolean
  updatedAt: number
  messageCount: number
  /** Awal jawaban terakhir, untuk pratinjau di riwayat. */
  preview: string
}

export interface AppSettings {
  retention: {
    /** Hari; null = selamanya. */
    eventDays: number | null
    /** Riwayat chat AI disimpan N hari; null = selamanya. */
    aiChatDays: number | null
    maxEvents: number
    /** Hapus permanen item soft delete setelah N hari; null = tidak pernah. */
    deletedDays: number | null
    auditDays: number | null
  }
  polling: {
    sessionSeconds: number
    logSeconds: number
    /** Jam; null = hanya manual. */
    autoImportHours: number | null
    timeoutSeconds: number
    retries: number
  }
  remote: {
    sessionHours: number
    maxFailures: number
    lockMinutes: number
    allowPublicWrite: boolean
  }
  ai: {
    /** Tombol dan jawaban AI chat; mati = disembunyikan untuk semua. */
    enabled: boolean
    /** Samarkan IP dan MAC pelanggan dalam jawaban. */
    maskSensitive: boolean
  }
}

export interface PurgeCounts {
  events: number
  users: number
  profiles: number
  chats: number
}

export interface StorageInfo {
  path: string
  sizeBytes: number
  events: number
  users: number
  deletedUsers: number
  deletedProfiles: number
  chats: number
  /** Yang akan dihapus bila "Bersihkan sekarang" dijalankan dengan pengaturan tersimpan. */
  purgeable: PurgeCounts
}

/** Akun untuk akses non-desktop (browser lewat Serve LAN dan Publik). Apa yang boleh dilakukan mengikuti peran. */
export interface AccountSummary {
  id: string
  username: string
  displayName: string
  roleId: string
  roleName: string
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
  roleName: string
  permissions: Permissions
  /** Asal sesi; tulis lewat "publik" bisa dimatikan pemilik. */
  via: "lan" | "publik"
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

/** Ubah router: username dan password kosong berarti tetap. */
export type RouterEdit = Omit<NewRouter, "username" | "password"> & { username?: string; password?: string }

export interface NewRouter {
  /** Izinkan edit user/profil PPP (akun API punya hak write). */
  allowWrite: boolean
  name: string
  host: string
  port: number
  tls: boolean
  username: string
  password: string
}

// ---- Dasbor AI (Pengaturan): provider, token, saldo, anggaran ----
/** 9Router = gateway OpenAI-compatible milik sendiri (perlu URL dasar); lainnya memakai alamat resmi penyedia. */
export type AiProviderKind = "ninerouter" | "anthropic" | "openai" | "deepseek" | "gemini"
export type AiProviderStatus = "ok" | "error" | "unconfigured"

/** Kunci API write-only: server hanya mengembalikan petunjuk (`keyHint`), tidak pernah kuncinya. */
export interface AiProvider {
  id: string
  name: string
  kind: AiProviderKind
  model: string
  baseUrl?: string
  keyHint?: string
  status: AiProviderStatus
  error?: string
  latencyMs?: number
  checkedAt?: number
  isDefault: boolean
  /** Sisa saldo (USD); null = penyedia tidak melaporkan saldo (mis. model lokal). */
  balanceUsd: number | null
}

export interface AiProviderInput {
  name: string
  kind: AiProviderKind
  model: string
  baseUrl?: string
  /** Kosong saat edit = kunci tetap. */
  apiKey?: string
}

export interface AiUsageDay {
  /** Awal hari (epoch ms). */
  day: number
  inputTokens: number
  outputTokens: number
  costUsd: number
  requests: number
}

export interface AiBudget {
  /** Batas biaya per bulan (USD); null = tanpa batas. */
  monthlyUsd: number | null
  /** Peringatan saat pemakaian mencapai persen ini. */
  alertPercent: number
  /** Bila true, AI berhenti menjawab saat batas tercapai. */
  hardStop: boolean
  spentMonthUsd: number
  resetAt: number
}

export interface AiUsageRow {
  key: string
  label: string
  requests: number
  inputTokens: number
  outputTokens: number
  costUsd: number
}

export interface AiDashboard {
  providers: AiProvider[]
  budget: AiBudget
  usage: AiUsageDay[]
  byModel: AiUsageRow[]
  byAccount: AiUsageRow[]
  errorRate: number
  p50Ms: number
}

export type AiBudgetInput = Pick<AiBudget, "monthlyUsd" | "alertPercent" | "hardStop">
