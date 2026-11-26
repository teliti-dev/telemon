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
  /** Usulan perubahan tata letak Ringkasan (ADR 0027): baru berlaku setelah pengguna menekan Terapkan. */
  | { kind: "proposal"; ops: LayoutOp[]; state?: "applied" | "dismissed" }

/** Satu perubahan tata letak yang diusulkan AI; `config` memakai id router. */
export type LayoutOp =
  | { op: "add"; type: string; config?: Record<string, unknown> }
  | { op: "remove"; id: string }
  | { op: "configure"; id: string; config: Record<string, unknown> }

/** Tampilan Ringkasan yang dikirim bersama pertanyaan AI, supaya AI tahu widget dan jenis yang ada. */
export interface AiUiContext {
  widgets: { id: string; type: string; config?: Record<string, unknown> }[]
  catalog: { type: string; title: string; description: string; configurable: boolean }[]
}

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
  /** Pengumpulan bandwidth per antarmuka untuk widget Ringkasan (ADR 0026). */
  bandwidth: {
    enabled: boolean
    /** Detik antar pembacaan counter antarmuka. */
    intervalSeconds: number
    retentionDays: number
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
  /** Sidik jari sertifikat api-ssl yang sudah dikonfirmasi pengguna (wajib bila TLS dinyalakan atau diganti). */
  fingerprint?: string
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
  /** Harga per 1 juta token (USD) untuk menghitung biaya; kosong = biaya tidak dihitung. */
  priceInPerMtok?: number
  priceOutPerMtok?: number
}

export interface AiProviderInput {
  name: string
  kind: AiProviderKind
  model: string
  baseUrl?: string
  /** Kosong saat edit = kunci tetap. */
  apiKey?: string
  priceInPerMtok?: number
  priceOutPerMtok?: number
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

// ---- Pembaruan aplikasi (hanya aplikasi desktop; ADR 0023) ----
// ---- Bandwidth (widget Ringkasan) ----
export interface TrafficInterface {
  /** Nama antarmuka di router, atau `@pppoe` = jumlah semua sesi PPPoE router itu. */
  name: string
  kind: string
  running: boolean
  lastSeen: number
}

export interface TrafficInterfaces {
  interfaces: TrafficInterface[]
  /** Pengumpulan bandwidth menyala di Pengaturan. */
  enabled: boolean
  intervalSeconds: number
}

export type TrafficRange = "1h" | "6h" | "24h" | "7d" | "30d"

export interface TrafficPoint {
  /** Awal ember waktu (epoch ms). */
  at: number
  /** bit per detik, sudut pandang router: untuk PPPoE `tx` = unduhan pelanggan, `rx` = unggahan. */
  rxBps: number
  txBps: number
  /** Rata-rata jumlah sesi, hanya untuk `@pppoe`. */
  sessions?: number
}

export interface TrafficSeries {
  points: TrafficPoint[]
  from: number
  to: number
  bucketSeconds: number
  intervalSeconds: number
  enabled: boolean
}

export interface TrafficQuery {
  /** Id router, atau `all` (total semua router; hanya untuk `@pppoe`). */
  router: string
  iface: string
  range: TrafficRange
}

// ---- Tata letak widget Ringkasan (satu untuk semua akun) ----
export interface DashboardWidget {
  /** Id unik di dalam tata letak. */
  id: string
  /** Jenis widget; daftar jenis ada di UI (components/dashboard/registry). */
  type: string
  x: number
  y: number
  w: number
  h: number
  /** Pengaturan per widget (mis. sumber data); bentuknya ditentukan jenisnya. */
  config?: Record<string, unknown>
}

export interface DashboardLayout {
  version: 1
  widgets: DashboardWidget[]
}

export type UpdateChannel = "stable" | "rc"

export interface UpdateStatus {
  /** Aplikasi ini punya mekanisme pembaruan. */
  supported: boolean
  currentVersion?: string
  /** Bisa memasang sendiri (Linux: hanya AppImage; paket .deb dipasang manual). */
  canInstall: boolean
  channel: UpdateChannel
  checkedAt?: number
  available?: { version: string; notes?: string; date?: string }
  error?: string
}

// ---- Extension (ADR 0028) ----
export interface ExtensionPermission {
  id: string
  required: boolean
  sensitive: boolean
  description: string
  granted: boolean
}

export interface ExtensionWidgetDecl {
  type: string
  title: string
  description: string
}

export interface ExtensionRequirement {
  id: string
  /** Rentang versi yang dibutuhkan, mis. ^1.2.0. */
  needs: string
  optional: boolean
  state: "ok" | "missing" | "incompatible"
  installed?: string
}

export interface ExtensionInfo {
  id: string
  name: string
  version: string
  publisher: string
  description: string
  tier: "official" | "public"
  signed: boolean
  enabled: boolean
  /** disabled (mati), limited (izin wajib dimatikan), ok, broken (manifest rusak). */
  status: "disabled" | "limited" | "incompatible" | "ok" | "broken"
  widgets: ExtensionWidgetDecl[]
  pages: ExtensionPageDecl[]
  permissions: ExtensionPermission[]
  /** Rentang versi Telemon yang didukung (kosong = tidak dibatasi). */
  telemon?: string
  requires: ExtensionRequirement[]
  /** Syarat wajib yang belum terpenuhi (id extension): tidak dimuat. */
  blockedBy: string[]
  /** Extension lain yang membutuhkan ini. */
  dependents: string[]
  /** Punya data sendiri (migrasi database sudah terapan). */
  hasData: boolean
  installedAt: number
  updatedAt: number
}

export interface ExtensionList {
  /** false = extension mati di server ini (belum dinyalakan lewat TELEMON_EXTENSIONS). */
  available: boolean
  extensions: ExtensionInfo[]
}

/** Isi paket sebelum dipasang (hasil periksa). */
export interface ExtensionPackageSummary {
  id: string
  name: string
  version: string
  publisher: string
  description: string
  tier: "official" | "public"
  signed: boolean
  widgets: ExtensionWidgetDecl[]
  permissions: Omit<ExtensionPermission, "granted">[]
}

export interface ExtensionInspect {
  package: ExtensionPackageSummary
  installedVersion?: string
  requires: ExtensionRequirement[]
  /** Penjelasan masalah kompatibilitas; kosong = boleh dipasang. */
  problemTexts: string[]
}

export interface ExtensionPageDecl {
  id: string
  title: string
  /** dashboard = halaman utama extension; page = halaman biasa. */
  kind: "dashboard" | "page"
}

export interface ExtensionActionDecl {
  id: string
  label: string
  /** Level menu minimal peran agar aksi ini bisa diberikan. */
  level: "read" | "update"
  description: string
}

export type ExtensionLevel = "none" | "read" | "update"

/** Extension aktif yang dimuat UI (hanya yang boleh dilihat akun ini). */
export interface ActiveExtension {
  id: string
  name: string
  version: string
  entry: string
  widgets: ExtensionWidgetDecl[]
  pages: ExtensionPageDecl[]
  /** Izin (toggle) yang diberikan admin. */
  permissions: string[]
  /** Akses efektif akun ini pada extension. */
  access: { level: ExtensionLevel; actions: string[] }
}

export interface ExtensionRoleAccess {
  id: string
  name: string
  builtin: boolean
  level: ExtensionLevel
  actions: string[]
  /** Peran akun yang sedang masuk: tidak bisa diubah sendiri. */
  own: boolean
}

export interface ExtensionAccess {
  actions: ExtensionActionDecl[]
  roles: ExtensionRoleAccess[]
  me: { level: ExtensionLevel; actions: string[] }
}

/** Satu entri toko extension official (katalog bertanda tangan). */
export interface ExtensionStoreItem {
  id: string
  name: string
  version: string
  publisher: string
  description: string
  telemon?: string
  requires: { id: string; version: string; optional: boolean }[]
  size: number
  installedVersion?: string
  upToDate: boolean
  /** Extension yang ikut terpasang (dependensi), berurutan. */
  alsoInstalls: string[]
  /** Alasan tidak bisa dipasang; kosong = bisa. */
  problem?: string
}

export interface ExtensionStore {
  generated: string
  extensions: ExtensionStoreItem[]
}
