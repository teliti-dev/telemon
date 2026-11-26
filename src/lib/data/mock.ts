/**
 * Adapter data contoh. Meniru perilaku router Mikrotik agar UI bisa dibangun
 * sebelum poller Rust ada: user PPPoE di beberapa router, log event, dan
 * event live acak setiap beberapa detik.
 */
import type {
  AiBudgetInput,
  AiDashboard,
  AiUiContext,
  ActiveExtension,
  ExtensionStore,
  ExtensionWorker,
  ExtensionAccess,
  ExtensionInfo,
  ExtensionInspect,
  ExtensionList,
  DashboardLayout,
  TrafficInterfaces,
  TrafficPoint,
  TrafficQuery,
  TrafficSeries,
  AiProvider,
  AiProviderInput,
  AiUsageDay,
  AiUsageRow,
  DeletedReason,
  ImportResult,
  NewRouter,
  PppEvent,
  PppEventType,
  PppProfile,
  PppUser,
  ProfileInput,
  ProfilePatch,
  RouterEdit,
  RouterInfo,
  AccountSummary,
  AiChat,
  AiChatSummary,
  AiMessage,
  AiStreamEvent,
  AppSettings,
  LoginResult,
  Permissions,
  PurgeCounts,
  Resource,
  Role,
  RoleInput,
  StorageInfo,
  SessionDevice,
  SessionInfo,
  ShareState,
  TwoFactorSetup,
  UserMeta,
  UserInput,
  UserMetaInput,
  UserPatch,
  UserStatus,
} from "../types"
import { answerQuestion, wantsBandwidth } from "./ai-engine"
import { DEFAULT_ROLES, DEFAULT_SETTINGS } from "../defaults"
import { t, tk } from "../i18n"
import { hasUpdate, RESOURCES } from "../permissions"
import type { LiveUpdate, Snapshot, TelemonApi } from "./api"

function mulberry32(seed: number) {
  return () => {
    seed = (seed + 0x6d2b79f5) | 0
    let t = Math.imul(seed ^ (seed >>> 15), 1 | seed)
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296
  }
}

const rand = mulberry32(20261118)
const pick = <T>(xs: readonly T[]) => xs[Math.floor(rand() * xs.length)]
const hex2 = () =>
  Math.floor(rand() * 256)
    .toString(16)
    .padStart(2, "0")
    .toUpperCase()
const mac = () => Array.from({ length: 6 }, hex2).join(":")
const delay = (ms: number) => new Promise((r) => setTimeout(r, ms))

const NAMES = [
  "budi", "siti", "agus", "dewi", "rina", "andi", "joko", "wati", "eko",
  "sri", "yuni", "hendra", "fajar", "dian", "rudi", "tika", "bayu", "nita",
  "adi", "lina", "wahyu", "ratna", "dodi", "maya", "imam", "sari", "teguh",
  "indah", "rizky", "putri", "warung", "masjid", "kantor", "toko", "kos",
] as const
const PLACES = ["rt", "gg", "blok", "ruko", "perum", "jl"] as const
const PROFILES = ["10Mbps", "20Mbps", "30Mbps", "50Mbps", "100Mbps"] as const
const DISCONNECT_REASONS = [
  "peer is not responding",
  "terminated by peer",
  "link down",
  "session timeout",
] as const

function makeRouters(now: number): RouterInfo[] {
  return [
    { id: "r1", canWrite: true, lastImportAt: now - 2 * 86400_000, name: "CCR-Pusat", host: "10.10.1.1", port: 8729, tls: true, model: "CCR2004-1G-12S+2XS", version: "7.16.2", status: "online", lastPollAt: now },
    { id: "r2", canWrite: true, lastImportAt: now - 2 * 86400_000, name: "RB-Timur", host: "10.10.2.1", port: 8729, tls: true, model: "RB4011iGS+", version: "7.15.3", status: "online", lastPollAt: now },
    { id: "r3", canWrite: false, lastImportAt: now - 5 * 86400_000, name: "hEX-Barat", host: "10.10.3.1", port: 8728, tls: false, model: "hEX S", version: "6.49.17", status: "online", lastPollAt: now },
    { id: "r4", canWrite: true, lastImportAt: now - 3 * 86400_000, name: "RB-Selatan", host: "10.10.4.1", port: 8728, tls: false, model: "RB1100AHx4", version: "7.14.3", status: "offline", lastPollAt: now - 47 * 60_000, error: "Tidak bisa terhubung ke 10.10.4.1:8728 (timeout 10 dtk)" },
  ]
}

function makeProfiles(routers: RouterInfo[]): PppProfile[] {
  return routers.flatMap((r) =>
    PROFILES.map((name) => {
      const mbps = parseInt(name, 10)
      return {
        id: `${r.id}:${name}`,
        routerId: r.id,
        name,
        rateLimit: `${mbps}M/${mbps}M`,
        localAddress: `100.64.${r.id.slice(1)}.1`,
        remoteAddress: `pool-pppoe-${r.id.slice(1)}`,
        onlyOne: "default" as const,
        comment: mbps >= 100 ? "Paket bisnis" : undefined,
      }
    })
  )
}

const USERS_PER_ROUTER: Record<string, number> = { r1: 64, r2: 46, r3: 27, r4: 21 }

function makeUsers(routers: RouterInfo[], now: number): PppUser[] {
  const users: PppUser[] = []
  const taken = new Set<string>()
  let ipSeq = 10
  for (const r of routers) {
    const n = USERS_PER_ROUTER[r.id] ?? 10
    const subnet = r.id.slice(1)
    for (let i = 0; i < n; i++) {
      let username: string
      do {
        username = `${pick(NAMES)}.${pick(PLACES)}${String(Math.floor(rand() * 40) + 1).padStart(2, "0")}`
      } while (taken.has(username))
      taken.add(username)
      const roll = rand()
      const status: UserStatus = roll < 0.76 ? "active" : roll < 0.93 ? "inactive" : "disabled"
      const lastSeenAt = now - Math.floor(rand() * 3 * 86400_000)
      const u: PppUser = {
        id: `${r.id}:${username}`,
        routerId: r.id,
        username,
        profile: pick(PROFILES),
        status,
        callerId: mac(),
        password: Array.from({ length: 8 }, () => "abcdefghjkmnpqrstuvwxyz23456789"[Math.floor(rand() * 31)]).join(""),
        lastSeenAt,
        comment: rand() < 0.2 ? pick(["Pelanggan baru", "Tunggakan", "Pindah alamat", "VIP"]) : undefined,
      }
      if (status === "active") {
        u.connectedAt = now - Math.floor(rand() * 6 * 86400_000)
        u.address = `100.64.${subnet}.${ipSeq++ % 250 + 2}`
        u.lastSeenAt = now
        u.lastEvent = "connected"
        u.lastEventAt = u.connectedAt
      } else if (status === "inactive") {
        u.lastEvent = "disconnected"
        u.lastEventAt = lastSeenAt
      }
      users.push(u)
    }
  }
  return users
}

let eventSeq = 0
function makeEvent(
  type: PppEventType,
  at: number,
  router: RouterInfo,
  user?: PppUser,
  extra?: string
): PppEvent {
  const u = user?.username
  const message: Record<PppEventType, string> = {
    auth_failed: `<pppoe-${u}>: user ${u} authentication failed${extra ? ` (${extra})` : ""}`,
    authenticated: `<pppoe-${u}>: authenticated`,
    connected: `<pppoe-${u}>: connected`,
    disconnected: `<pppoe-${u}>: disconnected${extra ? `, ${extra}` : ""}`,
    router_down: extra ?? `API ${router.host}:${router.port} tidak merespons`,
    router_up: `API ${router.host}:${router.port} terhubung kembali`,
  }
  return {
    id: `e${++eventSeq}`,
    at,
    routerId: router.id,
    type,
    username: u,
    address: type === "connected" ? user?.address : undefined,
    callerId: user?.callerId,
    message: message[type],
  }
}

function makeHistory(routers: RouterInfo[], users: PppUser[], now: number) {
  const events: PppEvent[] = []
  const byRouter = new Map(routers.map((r) => [r.id, r]))
  // Event sesi acak dalam 6 jam terakhir.
  for (let i = 0; i < 160; i++) {
    const u = pick(users)
    const r = byRouter.get(u.routerId)!
    if (r.status !== "online") continue
    const at = now - Math.floor(rand() * 6 * 3600_000)
    const t = pick<PppEventType>(["connected", "disconnected", "authenticated", "auth_failed"])
    if (t === "connected") {
      events.push(makeEvent("authenticated", at - 800, r, u))
    }
    events.push(makeEvent(t, at, r, u, t === "disconnected" ? pick(DISCONNECT_REASONS) : undefined))
  }
  // Dua user dengan password salah yang terus mencoba.
  for (const u of users.filter((x) => x.status === "inactive").slice(0, 2)) {
    const r = byRouter.get(u.routerId)!
    if (r.status !== "online") continue
    for (let k = 0; k < 9; k++) {
      events.push(makeEvent("auth_failed", now - k * 4 * 60_000 - Math.floor(rand() * 60_000), r, u))
    }
    u.lastEvent = "auth_failed"
    u.lastEventAt = now - Math.floor(rand() * 60_000)
  }
  // Router yang sedang offline.
  for (const r of routers.filter((x) => x.status !== "online")) {
    events.push(makeEvent("router_down", r.lastPollAt ?? now, r, undefined, r.error))
  }
  return events.sort((a, b) => b.at - a.at)
}

const CUSTOMERS = ["Budi Santoso", "Siti Rahma", "Agus Wijaya", "Dewi Lestari", "Rina Marlina", "Andi Pratama", "Joko Susilo", "Warung Bu Tini", "Masjid Al-Ikhlas", "Kantor Desa", "Toko Berkah", "Kos Putri Melati"] as const
const NOTES = ["Rumah lantai 2, router di ruang tamu.", "Sering minta cek saat hujan.", "Bayar tiap tanggal 5.", "ONU diganti 10 Nov.", "Kabel drop dari tiang 14."] as const
const TAGS = ["VIP", "Tunggakan", "Bermasalah", "Kantor", "Baru"] as const

function makeMetas(users: PppUser[], now: number): Record<string, UserMeta> {
  const metas: Record<string, UserMeta> = {}
  for (const u of users) {
    if (rand() > 0.3) continue
    const tags = rand() < 0.5 ? [pick(TAGS)] : []
    metas[u.id] = {
      customerName: pick(CUSTOMERS),
      phone: `08${Math.floor(1_000_000_000 + rand() * 8_999_999_999)}`.slice(0, 12),
      note: rand() < 0.6 ? pick(NOTES) : undefined,
      tags,
      updatedAt: now - Math.floor(rand() * 30 * 86400_000),
    }
  }
  return metas
}

// ---- Akun jarak jauh tiruan. Kredensial di bawah hanya untuk data contoh, bukan rahasia. ----
const DEMO_PASSWORD = "demo-telemon"
const DEMO_TOTP = "123456"
const SESSION_KEY = "telemon.mock.account"
const B32 = "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567"
const CODE_CHARS = "abcdefghjkmnpqrstuvwxyz23456789"

interface MockRole {
  id: string
  name: string
  description?: string
  permissions: Permissions
  builtin: boolean
}

interface MockAccount {
  id: string
  username: string
  displayName: string
  roleId: string
  password: string
  totp: boolean
  pendingSecret?: string
  recovery: string[]
  disabled: boolean
  createdAt: number
  lastLoginAt?: number
}

interface MockSession {
  id: string
  accountId: string
  /** Asal sesi: dipakai untuk menolak tulis lewat akses publik bila dimatikan. */
  via: "lan" | "publik"
  device: string
  ip: string
  createdAt: number
  lastSeenAt: number
}

const newSecret = () => Array.from({ length: 16 }, () => B32[Math.floor(rand() * B32.length)]).join("")
const codePart = () => Array.from({ length: 5 }, () => CODE_CHARS[Math.floor(rand() * CODE_CHARS.length)]).join("")
const newRecoveryCodes = () => Array.from({ length: 8 }, () => `${codePart()}-${codePart()}`)

function makeAccounts(now: number): MockAccount[] {
  return [
    { id: "a1", username: "admin", displayName: "Admin Demo", roleId: "role-admin", password: DEMO_PASSWORD, totp: true, recovery: newRecoveryCodes(), disabled: false, createdAt: now - 30 * 86400_000, lastLoginAt: now - 3 * 3600_000 },
    { id: "a2", username: "operator", displayName: "Operator NOC", roleId: "role-operator", password: DEMO_PASSWORD, totp: false, recovery: [], disabled: false, createdAt: now - 12 * 86400_000, lastLoginAt: now - 2 * 86400_000 },
    { id: "a4", username: "pantau", displayName: "Layar Pantau", roleId: "role-monitor", password: DEMO_PASSWORD, totp: false, recovery: [], disabled: false, createdAt: now - 3 * 86400_000, lastLoginAt: now - 86400_000 },
    { id: "a3", username: "tamu", displayName: "Tamu (nonaktif)", roleId: "role-viewer", password: DEMO_PASSWORD, totp: false, recovery: [], disabled: true, createdAt: now - 5 * 86400_000 },
  ]
}

function makeSessions(now: number): MockSession[] {
  return [
    { id: "s-old1", accountId: "a1", via: "lan", device: "Chrome · Windows", ip: "10.10.3.193", createdAt: now - 2 * 86400_000, lastSeenAt: now - 3 * 3600_000 },
    { id: "s-old2", accountId: "a1", via: "lan", device: "Safari · iPhone", ip: "10.10.3.54", createdAt: now - 6 * 86400_000, lastSeenAt: now - 26 * 3600_000 },
  ]
}

const TUNNEL_WORDS = ["quiet", "river", "amber", "solar", "maple", "harbor", "nickel", "violet", "canyon", "pixel", "meadow", "tango"]

export class MockApi implements TelemonApi {
  readonly kind = "mock" as const
  private routers: RouterInfo[]
  private users: PppUser[]
  private profiles: PppProfile[]
  private importedOnce = new Set<string>()
  private events: PppEvent[]
  private metas: Record<string, UserMeta>
  private share: ShareState = {
    lan: { enabled: false, bind: "lan", port: 7878 },
    publik: { status: "off", durationHours: 8 },
    auth: { require2fa: false, allowPublicWrite: DEFAULT_SETTINGS.remote.allowPublicWrite },
    features: { ai: DEFAULT_SETTINGS.ai.enabled, aiMask: DEFAULT_SETTINGS.ai.maskSensitive },
  }
  private accounts: MockAccount[]
  /** Riwayat chat AI per akun ("desktop" untuk aplikasi desktop). Disimpan di data, bukan di browser. */
  private chats = new Map<string, AiChat[]>()
  private roles: MockRole[] = DEFAULT_ROLES.map((r) => ({ ...r, permissions: { ...r.permissions }, builtin: true }))
  private settings: AppSettings = structuredClone(DEFAULT_SETTINGS)
  private aiProviders: AiProvider[] = [
    { id: "p1", name: "Anthropic", kind: "anthropic", model: "claude-sonnet-5-5", keyHint: "sk-ant-…x7Qa", status: "ok", latencyMs: 820, checkedAt: Date.now() - 600_000, isDefault: true, balanceUsd: 38.42 },
    { id: "p2", name: "DeepSeek", kind: "deepseek", model: "deepseek-chat", keyHint: "sk-ds…92fD", status: "error", error: "401: kunci ditolak atau kedaluwarsa", checkedAt: Date.now() - 3600_000, isDefault: false, balanceUsd: 0 },
    { id: "p3", name: "9Router", kind: "ninerouter", model: "combo-cepat", baseUrl: "http://10.0.0.5:8080/v1", keyHint: "sk-9r…41cB", status: "ok", latencyMs: 1400, checkedAt: Date.now() - 900_000, isDefault: false, balanceUsd: null },
  ]
  private aiBudget: AiBudgetInput = { monthlyUsd: 25, alertPercent: 80, hardStop: false }
  private sessions: MockSession[]
  private currentSessionId?: string
  private challenges = new Map<string, { accountId: string; expires: number }>()
  private failures = new Map<string, { n: number; until: number }>()
  private listeners = new Set<(u: LiveUpdate) => void>()
  private timer?: ReturnType<typeof setInterval>
  private expiryTimer?: ReturnType<typeof setTimeout>

  constructor() {
    const now = Date.now()
    this.routers = makeRouters(now)
    this.users = makeUsers(this.routers, now)
    this.profiles = makeProfiles(this.routers)
    this.events = makeHistory(this.routers, this.users, now)
    this.metas = makeMetas(this.users, now)
    this.accounts = makeAccounts(now)
    this.sessions = makeSessions(now)
    this.seedChats(now)
  }

  async snapshot(): Promise<Snapshot> {
    await delay(300)
    const perms = this.currentSessionId ? this.roleOf(this.current().a).permissions : null
    const can = (r: Resource) => !perms || perms[r] !== "none"
    return {
      routers: this.routers.map((r) => ({ ...r })),
      users: can("user") ? this.users.map((u) => ({ ...u })) : [],
      profiles: can("profile") ? this.profiles.map((p) => ({ ...p })) : [],
      events: can("event") ? [...this.events] : [],
      metas: can("user") ? structuredClone(this.metas) : {},
      share: structuredClone(this.share),
    }
  }

  subscribe(onUpdate: (u: LiveUpdate) => void) {
    this.listeners.add(onUpdate)
    if (!this.timer) this.timer = setInterval(() => this.tick(), 2500)
    return () => {
      this.listeners.delete(onUpdate)
      if (this.listeners.size === 0 && this.timer) {
        clearInterval(this.timer)
        this.timer = undefined
      }
    }
  }

  private emit(u: LiveUpdate) {
    for (const l of this.listeners) l(u)
  }

  private push(event: PppEvent, user?: PppUser) {
    this.events.unshift(event)
    if (this.events.length > 2000) this.events.length = 2000
    if (user) {
      user.lastEvent = event.type
      user.lastEventAt = event.at
    }
    this.emit({ kind: "event", event, user: user ? { ...user } : undefined })
  }

  private tick() {
    const now = Date.now()
    for (const r of this.routers) {
      if (r.status === "online") r.lastPollAt = now
    }
    const online = new Set(this.routers.filter((r) => r.status === "online").map((r) => r.id))
    const candidates = this.users.filter((u) => online.has(u.routerId) && !u.deletedAt)
    if (candidates.length === 0) return
    const u = pick(candidates)
    const r = this.routers.find((x) => x.id === u.routerId)!

    if (u.status === "active") {
      if (rand() < 0.5) {
        u.status = "inactive"
        u.connectedAt = undefined
        u.address = undefined
        u.lastSeenAt = now
        this.push(makeEvent("disconnected", now, r, u, pick(DISCONNECT_REASONS)), u)
      }
    } else if (u.status === "disabled" || rand() < 0.35) {
      this.push(makeEvent("auth_failed", now, r, u, u.status === "disabled" ? "account disabled" : undefined), u)
    } else {
      this.push(makeEvent("authenticated", now - 400, r, u), u)
      u.status = "active"
      u.connectedAt = now
      u.lastSeenAt = now
      u.address = `100.64.${r.id.slice(1)}.${Math.floor(rand() * 240) + 10}`
      this.push(makeEvent("connected", now, r, u), u)
    }
  }

  async addRouter(input: NewRouter): Promise<RouterInfo> {
    this.authorize("router")
    await delay(900)
    if (!input.username || !input.password) {
      throw new Error(t("Username dan password API wajib diisi"))
    }
    const router: RouterInfo = {
      id: `r${Date.now().toString(36)}`,
      name: input.name,
      host: input.host,
      port: input.port,
      tls: input.tls,
      model: "Belum diketahui",
      version: "-",
      status: "error",
      canWrite: input.allowWrite,
      error: "Data contoh: router baru tidak benar-benar dihubungi",
    }
    this.routers.push(router)
    this.emit({ kind: "router", router: { ...router } })
    return router
  }

  // ---- Login akses non-desktop ----

  private roleOf(a: MockAccount): MockRole {
    return this.roles.find((r) => r.id === a.roleId) ?? this.roles[this.roles.length - 1]
  }

  private info(a: MockAccount, sid: string): SessionInfo {
    const role = this.roleOf(a)
    return {
      id: sid, userId: a.id, username: a.username, displayName: a.displayName,
      roleName: role.name, permissions: { ...role.permissions }, twoFactor: a.totp,
      via: this.sessions.find((x) => x.id === sid)?.via ?? "lan",
    }
  }

  /** 2FA wajib: pengaturan pemilik, akses publik aktif, atau peran dengan izin Ubah. */
  private must2fa(a: MockAccount) {
    return this.share.auth.require2fa || this.share.publik.status === "on" || hasUpdate(this.roleOf(a).permissions)
  }

  /**
   * Meniru penegakan di server. Tanpa sesi = aplikasi desktop (boleh semua).
   * Sesi jarak jauh: butuh izin Ubah pada menu terkait, dan tulis lewat akses publik
   * ditolak bila dimatikan. "desktop" = hanya aplikasi desktop.
   */
  private authorize(resource: Resource | "desktop", need: "read" | "update" = "update") {
    if (!this.currentSessionId) return
    const { a, s } = this.current()
    if (resource === "desktop") throw new Error(t("Hanya bisa dilakukan dari aplikasi desktop"))
    const level = this.roleOf(a).permissions[resource]
    if (need === "read") {
      if (level === "none") throw new Error(t("Peran Anda tidak punya akses ke data ini"))
      return
    }
    if (level !== "update") throw new Error(t("Peran Anda tidak boleh mengubah data ini"))
    if (s.via === "publik" && !this.share.auth.allowPublicWrite) {
      throw new Error(t("Tulis lewat akses publik dimatikan oleh pemilik Telemon"))
    }
  }

  private caller(): MockAccount | null {
    return this.currentSessionId ? this.current().a : null
  }

  /** Pengaman anti-kenaikan izin: sesi jarak jauh tidak bisa memberi izin melebihi miliknya. */
  private assertGrantable(perms: Permissions) {
    const me = this.caller()
    if (!me) return
    const rank = { none: 0, read: 1, update: 2 } as const
    const mine = this.roleOf(me).permissions
    for (const r of RESOURCES) {
      if (rank[perms[r.key]] > rank[mine[r.key]]) throw new Error(t("Anda tidak bisa memberi izin melebihi peran Anda"))
    }
  }

  /** Akun sendiri dan akun berizin lebih tinggi tidak bisa dikelola dari Admin oleh sesi jarak jauh. */
  private assertManageable(target: MockAccount) {
    const me = this.caller()
    if (!me) return
    if (me.id === target.id) throw new Error(t("Tidak bisa mengubah akun Anda sendiri dari sini; pakai halaman Akun saya"))
    this.assertGrantable(this.roleOf(target).permissions)
  }

  private assertNotOwnRole(roleId: string) {
    const me = this.caller()
    if (me && me.roleId === roleId) throw new Error(t("Tidak bisa mengubah peran yang sedang Anda pakai"))
  }

  private openSession(a: MockAccount, touch = true): SessionInfo {
    const now = Date.now()
    const s: MockSession = {
      id: `s${now.toString(36)}`, accountId: a.id, via: this.share.publik.status === "on" ? "publik" : "lan", device: "Browser ini", ip: "10.10.3.193", createdAt: now, lastSeenAt: now,
    }
    this.sessions.push(s)
    this.currentSessionId = s.id
    if (touch) a.lastLoginAt = now
    try {
      sessionStorage.setItem(SESSION_KEY, a.id)
    } catch {
      /* abaikan */
    }
    return this.info(a, s.id)
  }

  private current(): { a: MockAccount; s: MockSession } {
    const s = this.sessions.find((x) => x.id === this.currentSessionId)
    const a = s && this.accounts.find((x) => x.id === s.accountId && !x.disabled)
    if (!s || !a) throw new Error(t("Belum login"))
    return { a, s }
  }

  private fail(key: string) {
    const f = this.failures.get(key) ?? { n: 0, until: 0 }
    f.n++
    if (f.n >= this.settings.remote.maxFailures) {
      f.until = Date.now() + this.settings.remote.lockMinutes * 60_000
      f.n = 0
    }
    this.failures.set(key, f)
  }

  private newChallenge(a: MockAccount) {
    const id = `c${Date.now().toString(36)}${Math.floor(rand() * 1e6).toString(36)}`
    this.challenges.set(id, { accountId: a.id, expires: Date.now() + 5 * 60_000 })
    return id
  }

  private takeChallenge(id: string) {
    const c = this.challenges.get(id)
    const a = c && this.accounts.find((x) => x.id === c.accountId)
    if (!c || !a || c.expires < Date.now()) throw new Error(t("Sesi login kedaluwarsa. Masuk ulang"))
    return a
  }

  private otpauth(a: MockAccount, secret: string) {
    return `otpauth://totp/Telemon:${a.username}?secret=${secret}&issuer=Telemon`
  }

  private checkPassword(a: MockAccount, password: string) {
    if (a.password !== password) throw new Error(t("Password salah"))
  }

  async session(): Promise<SessionInfo | null> {
    await delay(200)
    if (!this.currentSessionId) {
      let id: string | null = null
      try {
        id = sessionStorage.getItem(SESSION_KEY)
      } catch {
        /* abaikan */
      }
      const a = this.accounts.find((x) => x.id === id && !x.disabled)
      return a ? this.openSession(a, false) : null
    }
    const { a, s } = this.current()
    return this.info(a, s.id)
  }

  async login(username: string, password: string): Promise<LoginResult> {
    await delay(500)
    const key = username.trim().toLowerCase()
    const f = this.failures.get(key)
    if (f && f.until > Date.now()) {
      throw new Error(t("Terlalu banyak percobaan. Coba lagi dalam {n} menit", { n: Math.ceil((f.until - Date.now()) / 60_000) }))
    }
    const a = this.accounts.find((x) => x.username === key)
    if (!a || a.password !== password) {
      this.fail(key)
      throw new Error(t("Username atau password salah"))
    }
    if (a.disabled) throw new Error(t("Akun dinonaktifkan. Hubungi pemilik Telemon"))
    this.failures.delete(key)
    if (a.totp) return { status: "2fa_required", challengeId: this.newChallenge(a) }
    if (this.must2fa(a)) {
      const secret = newSecret()
      a.pendingSecret = secret
      return { status: "2fa_setup_required", challengeId: this.newChallenge(a), secret, otpauthUrl: this.otpauth(a, secret) }
    }
    return { status: "ok", session: this.openSession(a) }
  }

  async verify2fa(challengeId: string, code: string, opts?: { recovery?: boolean }): Promise<SessionInfo> {
    await delay(400)
    const a = this.takeChallenge(challengeId)
    const clean = code.replace(/\s/g, "").toLowerCase()
    let ok = false
    if (opts?.recovery) {
      const i = a.recovery.indexOf(clean)
      if (i >= 0) {
        a.recovery.splice(i, 1)
        ok = true
      }
    } else {
      ok = clean === DEMO_TOTP
    }
    if (!ok) {
      this.fail(a.username)
      throw new Error(opts?.recovery ? t("Kode pemulihan salah atau sudah dipakai") : t("Kode salah"))
    }
    this.challenges.delete(challengeId)
    return this.openSession(a)
  }

  async complete2faSetup(challengeId: string, code: string) {
    await delay(500)
    const a = this.takeChallenge(challengeId)
    if (code.replace(/\s/g, "") !== DEMO_TOTP) throw new Error(t("Kode salah"))
    this.challenges.delete(challengeId)
    a.totp = true
    a.pendingSecret = undefined
    a.recovery = newRecoveryCodes()
    return { session: this.openSession(a), recoveryCodes: [...a.recovery] }
  }

  async logout() {
    await delay(150)
    this.sessions = this.sessions.filter((x) => x.id !== this.currentSessionId)
    this.currentSessionId = undefined
    try {
      sessionStorage.removeItem(SESSION_KEY)
    } catch {
      /* abaikan */
    }
  }

  async updateMyProfile(input: { displayName: string }) {
    await delay(300)
    const { a, s } = this.current()
    const name = input.displayName.trim()
    if (!name || name.length > 60) throw new Error(t("Nama 1-60 karakter"))
    a.displayName = name
    return this.info(a, s.id)
  }

  async changeMyPassword(current: string, next: string) {
    await delay(500)
    const { a } = this.current()
    this.checkPassword(a, current)
    if (next.length < 8) throw new Error(t("Password baru minimal 8 karakter"))
    if (next === current) throw new Error(t("Password baru harus berbeda"))
    a.password = next
  }

  async start2faSetup(): Promise<TwoFactorSetup> {
    await delay(300)
    const { a } = this.current()
    if (a.totp) throw new Error(t("2FA sudah aktif"))
    const secret = newSecret()
    a.pendingSecret = secret
    return { secret, otpauthUrl: this.otpauth(a, secret) }
  }

  async confirm2faSetup(code: string) {
    await delay(500)
    const { a } = this.current()
    if (!a.pendingSecret) throw new Error(t("Mulai pendaftaran 2FA terlebih dulu"))
    if (code.replace(/\s/g, "") !== DEMO_TOTP) throw new Error(t("Kode salah"))
    a.totp = true
    a.pendingSecret = undefined
    a.recovery = newRecoveryCodes()
    return { recoveryCodes: [...a.recovery] }
  }

  async disable2fa(password: string) {
    await delay(500)
    const { a } = this.current()
    this.checkPassword(a, password)
    if (this.must2fa(a)) throw new Error(t("2FA diwajibkan untuk akun ini"))
    a.totp = false
    a.recovery = []
  }

  async regenerateRecoveryCodes(password: string) {
    await delay(500)
    const { a } = this.current()
    this.checkPassword(a, password)
    if (!a.totp) throw new Error(t("Aktifkan 2FA terlebih dulu"))
    a.recovery = newRecoveryCodes()
    return [...a.recovery]
  }

  async listMySessions(): Promise<SessionDevice[]> {
    await delay(250)
    const { a } = this.current()
    return this.sessions
      .filter((x) => x.accountId === a.id)
      .map((x) => ({ id: x.id, device: x.device, ip: x.ip, createdAt: x.createdAt, lastSeenAt: x.lastSeenAt, current: x.id === this.currentSessionId }))
      .sort((x, y) => Number(y.current) - Number(x.current) || y.lastSeenAt - x.lastSeenAt)
  }

  async revokeSession(id: string) {
    await delay(300)
    if (id === this.currentSessionId) throw new Error(t("Gunakan Keluar untuk sesi ini"))
    this.sessions = this.sessions.filter((x) => x.id !== id)
  }

  // ---- Kelola akun (aplikasi desktop) ----

  private summary(a: MockAccount): AccountSummary {
    return {
      id: a.id, username: a.username, displayName: a.displayName, roleId: a.roleId, roleName: this.roleOf(a).name,
      twoFactor: a.totp, disabled: a.disabled, createdAt: a.createdAt, lastLoginAt: a.lastLoginAt,
    }
  }

  private accountOrThrow(id: string) {
    const a = this.accounts.find((x) => x.id === id)
    if (!a) throw new Error(t("Akun tidak ditemukan"))
    return a
  }

  async listAccounts() {
    this.authorize("admin", "read")
    await delay(250)
    return this.accounts.map((a) => this.summary(a))
  }

  async createAccount(input: { username: string; displayName: string; password: string; roleId: string }) {
    this.authorize("admin")
    await delay(500)
    const grant = this.roles.find((r) => r.id === input.roleId)
    if (!grant) throw new Error(t("Pilih peran untuk akun ini"))
    this.assertGrantable(grant.permissions)
    const username = input.username.trim().toLowerCase()
    if (!/^[a-z0-9._-]{3,30}$/.test(username)) throw new Error(t("Username 3-30 karakter: huruf kecil, angka, titik, garis bawah, atau strip"))
    if (this.accounts.some((a) => a.username === username)) throw new Error(t("Username sudah dipakai"))
    if (!input.displayName.trim()) throw new Error(t("Nama wajib diisi"))
    if (input.password.length < 8) throw new Error(t("Password minimal 8 karakter"))
    const a: MockAccount = {
      id: `a${Date.now().toString(36)}`, username, displayName: input.displayName.trim(), roleId: input.roleId, password: input.password,
      totp: false, recovery: [], disabled: false, createdAt: Date.now(),
    }
    this.accounts.push(a)
    return this.summary(a)
  }

  async resetAccountPassword(id: string, password: string) {
    this.authorize("admin")
    await delay(400)
    if (password.length < 8) throw new Error(t("Password minimal 8 karakter"))
    const a = this.accountOrThrow(id)
    this.assertManageable(a)
    a.password = password
    this.sessions = this.sessions.filter((x) => x.accountId !== id)
  }

  async setAccountDisabled(id: string, disabled: boolean) {
    this.authorize("admin")
    await delay(300)
    const a = this.accountOrThrow(id)
    this.assertManageable(a)
    a.disabled = disabled
    if (disabled) this.sessions = this.sessions.filter((x) => x.accountId !== id)
    return this.summary(a)
  }

  async resetAccount2fa(id: string) {
    this.authorize("admin")
    await delay(300)
    const a = this.accountOrThrow(id)
    this.assertManageable(a)
    a.totp = false
    a.recovery = []
    this.sessions = this.sessions.filter((x) => x.accountId !== id)
    return this.summary(a)
  }

  async deleteAccount(id: string) {
    this.authorize("admin")
    await delay(300)
    const a = this.accountOrThrow(id)
    this.assertManageable(a)
    const rest = this.accounts.filter((x) => x.id !== a.id && !x.disabled)
    if (rest.length === 0 && this.share.publik.status === "on") {
      throw new Error(t("Akses publik sedang aktif; matikan dulu sebelum menghapus akun terakhir"))
    }
    this.accounts = this.accounts.filter((x) => x.id !== id)
    this.sessions = this.sessions.filter((x) => x.accountId !== id)
  }

  async setRequire2fa(required: boolean) {
    this.authorize("admin")
    await delay(300)
    if (!required && this.share.publik.status === "on") throw new Error(t("Wajib selama akses publik aktif"))
    this.share.auth.require2fa = required
    return this.shareChanged()
  }

  // ---- AI chat (hanya membaca) ----

  private aiContext(can: (r: Resource) => boolean, now = Date.now()) {
    return {
      routers: this.routers.map((r) => ({ ...r })),
      users: can("user") ? this.users.filter((u) => !u.deletedAt) : [],
      profiles: can("profile") ? this.profiles.filter((p) => !p.deletedAt) : [],
      events: can("event") ? this.events : [],
      metas: can("user") ? this.metas : {},
      now,
      mask: this.settings.ai.maskSensitive,
      can,
    }
  }

  async *askAi(messages: AiMessage[], opts?: { signal?: AbortSignal; ui?: AiUiContext }): AsyncGenerator<AiStreamEvent> {
    this.authorize("ai", "read")
    if (!this.settings.ai.enabled) throw new Error(t("AI dimatikan oleh pemilik Telemon"))
    const question = [...messages].reverse().find((m) => m.role === "user")?.text ?? ""
    const perms = this.currentSessionId ? this.roleOf(this.current().a).permissions : null
    const can = (r: Resource) => !perms || perms[r] !== "none"
    const series = wantsBandwidth(question) && can("router") ? await this.getTrafficSeries({ router: "all", iface: "@pppoe", range: "24h" }).then((s) => s.points).catch(() => undefined) : undefined
    const canEditLayout = !perms || perms.ringkasan === "update"
    const ans = answerQuestion(question, { ...this.aiContext(can), ui: opts?.ui, canEditLayout, series })
    const stopped = () => !!opts?.signal?.aborted
    for (const text of ans.steps ?? []) {
      if (stopped()) return
      yield { type: "step", text }
      await delay(230)
    }
    await delay(150)
    for (let i = 0; i < ans.text.length; i += 4) {
      if (stopped()) return
      yield { type: "text", text: ans.text.slice(i, i + 4) }
      await delay(14)
    }
    for (const widget of ans.widgets ?? []) {
      if (stopped()) return
      await delay(160)
      yield { type: "widget", widget }
    }
    if (stopped()) return
    if (ans.actions?.length) yield { type: "actions", actions: ans.actions }
    if (ans.sources?.length) yield { type: "sources", items: ans.sources }
    if (ans.followUps?.length) yield { type: "followups", items: ans.followUps }
  }

  // ---- Riwayat chat AI ----

  private chatKey() {
    return this.currentSessionId ? this.current().a.id : "desktop"
  }

  private chatList(): AiChat[] {
    const key = this.chatKey()
    const days = this.settings.retention.aiChatDays
    let list = this.chats.get(key) ?? []
    if (days !== null) list = list.filter((c) => c.updatedAt >= Date.now() - days * 86400_000)
    this.chats.set(key, list)
    return list
  }

  private chatSummary(c: AiChat): AiChatSummary {
    const last = [...c.messages].reverse().find((m) => m.role === "assistant" && m.text)
    return {
      id: c.id,
      title: c.title,
      pinned: c.pinned,
      updatedAt: c.updatedAt,
      messageCount: c.messages.length,
      preview: (last?.text ?? "").replace(/[*`]/g, "").split("\n").find((l) => l.trim()) ?? "",
    }
  }

  private seedChats(now: number) {
    const perms = null
    const ctx = this.aiContext(() => true, now)
    const mk = (id: string, title: string, ageMs: number, questions: string[], pinned = false): AiChat => {
      let at = now - ageMs
      const messages: AiMessage[] = []
      for (const q of questions) {
        const a = answerQuestion(q, { ...ctx, now: at })
        messages.push({ id: `${id}-u${messages.length}`, role: "user", text: q, at })
        at += 4000
        messages.push({ id: `${id}-a${messages.length}`, role: "assistant", text: a.text, at, widgets: a.widgets, actions: a.actions, followUps: a.followUps, sources: a.sources, steps: a.steps })
        at += 60_000
      }
      return { id, title, pinned, createdAt: now - ageMs, updatedAt: at, messages }
    }
    void perms
    const seed = [
      mk("c-seed-1", "Ringkasan jaringan pagi", 3 * 3600_000, ["Berapa user aktif sekarang?"], true),
      mk("c-seed-2", "Router bermasalah", 26 * 3600_000, ["Router mana yang bermasalah?", "Tampilkan event terbaru"]),
      mk("c-seed-3", "User gagal auth", 4 * 86400_000, ["Siapa yang sering gagal auth?"]),
    ]
    this.chats.set("desktop", seed)
    this.chats.set("a1", seed.map((c) => structuredClone(c)))
  }

  async listAiChats(): Promise<AiChatSummary[]> {
    this.authorize("ai", "read")
    await delay(180)
    return this.chatList()
      .map((c) => this.chatSummary(c))
      .sort((a, b) => Number(b.pinned) - Number(a.pinned) || b.updatedAt - a.updatedAt)
  }

  async getAiChat(id: string): Promise<AiChat> {
    this.authorize("ai", "read")
    await delay(120)
    const c = this.chatList().find((x) => x.id === id)
    if (!c) throw new Error(t("Percakapan tidak ditemukan"))
    return structuredClone(c)
  }

  async saveAiChat(chat: AiChat): Promise<AiChatSummary> {
    this.authorize("ai", "read")
    const list = this.chatList()
    const title = chat.title.trim().slice(0, 80) || t("Percakapan baru")
    const next: AiChat = { ...structuredClone(chat), title, messages: structuredClone(chat.messages).slice(-200) }
    const i = list.findIndex((x) => x.id === chat.id)
    if (i >= 0) list[i] = next
    else list.push(next)
    this.chats.set(this.chatKey(), list.slice(-100))
    return this.chatSummary(next)
  }

  async deleteAiChat(id: string): Promise<void> {
    this.authorize("ai", "read")
    await delay(120)
    this.chats.set(this.chatKey(), this.chatList().filter((x) => x.id !== id))
  }

  // ---- Peran, pengaturan, dan penyimpanan (hanya aplikasi desktop) ----

  private toRole(r: MockRole): Role {
    return {
      id: r.id, name: r.name, description: r.description, permissions: { ...r.permissions }, builtin: r.builtin,
      accountCount: this.accounts.filter((a) => a.roleId === r.id).length,
    }
  }

  private validateRole(input: RoleInput, ignoreId?: string) {
    const name = input.name.trim()
    if (name.length < 2 || name.length > 30) throw new Error(t("Nama peran 2-30 karakter"))
    if (this.roles.some((r) => r.id !== ignoreId && r.name.toLowerCase() === name.toLowerCase())) {
      throw new Error(t("Nama peran sudah dipakai"))
    }
    for (const res of RESOURCES) {
      // i18n-ignore: label ditandai tk() di permissions
      if (!res.levels.includes(input.permissions[res.key])) throw new Error(t("Izin untuk {label} tidak valid", { label: t(res.label) }))
    }
    return name
  }

  async listRoles(): Promise<Role[]> {
    this.authorize("admin", "read")
    await delay(200)
    return this.roles.map((r) => this.toRole(r))
  }

  async createRole(input: RoleInput): Promise<Role> {
    this.authorize("admin")
    await delay(400)
    this.assertGrantable(input.permissions)
    const name = this.validateRole(input)
    const role: MockRole = {
      id: `role-${Date.now().toString(36)}`, name, description: input.description?.trim() || undefined,
      permissions: { ...input.permissions }, builtin: false,
    }
    this.roles.push(role)
    return this.toRole(role)
  }

  async updateRole(id: string, input: RoleInput): Promise<Role> {
    this.authorize("admin")
    await delay(400)
    const role = this.roles.find((r) => r.id === id)
    if (!role) throw new Error(t("Peran tidak ditemukan"))
    this.assertNotOwnRole(id)
    this.assertGrantable(input.permissions)
    role.name = this.validateRole(input, id)
    role.description = input.description?.trim() || undefined
    role.permissions = { ...input.permissions }
    return this.toRole(role)
  }

  async deleteRole(id: string) {
    this.authorize("admin")
    await delay(300)
    const role = this.roles.find((r) => r.id === id)
    if (!role) throw new Error(t("Peran tidak ditemukan"))
    this.assertNotOwnRole(id)
    if (role.builtin) throw new Error(t("Peran bawaan tidak bisa dihapus"))
    const used = this.accounts.filter((a) => a.roleId === id).length
    if (used > 0) throw new Error(t("Peran masih dipakai {n} akun; pindahkan akun-nya dulu", { n: used }))
    this.roles = this.roles.filter((r) => r.id !== id)
  }

  async setAccountRole(id: string, roleId: string) {
    this.authorize("admin")
    await delay(300)
    const a = this.accountOrThrow(id)
    const target = this.roles.find((r) => r.id === roleId)
    if (!target) throw new Error(t("Peran tidak ditemukan"))
    this.assertManageable(a)
    this.assertGrantable(target.permissions)
    a.roleId = roleId
    return this.summary(a)
  }

  private checkRange(label: string, v: number | null, min: number, max: number, nullable = false) {
    if (v === null) {
      if (nullable) return
      // i18n-ignore: label ditandai tk() di pemanggil
      throw new Error(t("{label} wajib diisi", { label: t(label) }))
    }
    // i18n-ignore: label ditandai tk() di pemanggil
    if (!Number.isInteger(v) || v < min || v > max) throw new Error(t("{label} harus {min}-{max}", { label: t(label), min, max }))
  }

  // ---- Dasbor AI ----
  async getAiDashboard(days: 7 | 30): Promise<AiDashboard> {
    this.authorize("pengaturan", "read")
    await delay(250)
    const now = Date.now()
    const dayMs = 86400_000
    const today = now - (now % dayMs)
    const seed = mulberry32(77)
    const usage: AiUsageDay[] = []
    for (let i = 29; i >= 0; i--) {
      const weekend = [0, 6].includes(new Date(today - i * dayMs).getUTCDay())
      const requests = Math.floor((weekend ? 8 : 30) + seed() * (weekend ? 10 : 45))
      const inputTokens = requests * Math.floor(1800 + seed() * 2600)
      const outputTokens = requests * Math.floor(250 + seed() * 500)
      const costUsd = (inputTokens * 3 + outputTokens * 15) / 1e6
      usage.push({ day: today - i * dayMs, inputTokens, outputTokens, costUsd: Math.round(costUsd * 100) / 100, requests })
    }
    const monthStart = new Date(now)
    monthStart.setUTCDate(1)
    monthStart.setUTCHours(0, 0, 0, 0)
    const spent = usage.filter((u) => u.day >= monthStart.getTime()).reduce((s, u) => s + u.costUsd, 0)
    const resetAt = Date.UTC(monthStart.getUTCFullYear(), monthStart.getUTCMonth() + 1, 1)
    const window = usage.slice(-days)
    const sum = (f: (u: AiUsageDay) => number) => window.reduce((s, u) => s + f(u), 0)
    const split = (parts: [string, string, number][]): AiUsageRow[] =>
      parts.map(([key, label, share]) => ({
        key,
        label,
        requests: Math.round(sum((u) => u.requests) * share),
        inputTokens: Math.round(sum((u) => u.inputTokens) * share),
        outputTokens: Math.round(sum((u) => u.outputTokens) * share),
        costUsd: Math.round(sum((u) => u.costUsd) * share * 100) / 100,
      }))
    return {
      providers: structuredClone(this.aiProviders),
      budget: { ...this.aiBudget, spentMonthUsd: Math.round(spent * 100) / 100, resetAt },
      usage: window,
      byModel: split([["claude-sonnet-5-5", "claude-sonnet-5-5", 0.86], ["combo-cepat", "combo-cepat", 0.14]]),
      byAccount: split([["admin", "admin", 0.52], ["operator", "operator", 0.38], ["pantau", "pantau", 0.1]]),
      errorRate: 0.031,
      p50Ms: 910,
    }
  }

  private providerOrThrow(id: string) {
    const p = this.aiProviders.find((x) => x.id === id)
    if (!p) throw new Error(t("Provider tidak ditemukan"))
    return p
  }

  async saveAiProvider(id: string | null, input: AiProviderInput): Promise<AiProvider> {
    this.authorize("pengaturan")
    await delay(500)
    const name = input.name.trim()
    const model = input.model.trim()
    if (!name || !model) throw new Error(t("Nama dan model wajib diisi"))
    const needsKey = true
    const key = input.apiKey?.trim()
    if (!id && needsKey && !key) throw new Error(t("Kunci API wajib diisi"))
    if (input.kind === "ninerouter") {
      if (!/^https?:\/\/\S+$/.test(input.baseUrl?.trim() ?? "")) throw new Error(t("URL dasar harus diawali http:// atau https://"))
    }
    if (this.aiProviders.some((p) => p.id !== id && p.name.toLowerCase() === name.toLowerCase())) throw new Error(t("Nama provider sudah dipakai"))
    const hint = key ? `${key.slice(0, Math.min(6, key.length - 4))}…${key.slice(-4)}` : undefined
    let p: AiProvider
    if (id) {
      p = this.providerOrThrow(id)
      Object.assign(p, { name, kind: input.kind, model, baseUrl: input.baseUrl?.trim() || undefined })
      if (hint) Object.assign(p, { keyHint: hint, status: "unconfigured" as const, error: undefined })
    } else {
      p = { id: `p${Date.now()}`, name, kind: input.kind, model, baseUrl: input.baseUrl?.trim() || undefined, keyHint: hint, status: "unconfigured", isDefault: this.aiProviders.length === 0, balanceUsd: null }
      this.aiProviders.push(p)
    }
    return structuredClone(p)
  }

  async removeAiProvider(id: string): Promise<void> {
    this.authorize("pengaturan")
    await delay(300)
    const p = this.providerOrThrow(id)
    if (p.isDefault && this.aiProviders.length > 1) throw new Error(t("Jadikan provider lain sebagai bawaan dulu"))
    this.aiProviders = this.aiProviders.filter((x) => x.id !== id)
  }

  async setDefaultAiProvider(id: string): Promise<void> {
    this.authorize("pengaturan")
    await delay(250)
    const p = this.providerOrThrow(id)
    if (p.status === "error") throw new Error(t("Provider sedang error; uji dulu sebelum dijadikan bawaan"))
    this.aiProviders.forEach((x) => (x.isDefault = x.id === id))
  }

  async testAiProvider(id: string): Promise<AiProvider> {
    this.authorize("pengaturan")
    await delay(900)
    const p = this.providerOrThrow(id)
    p.checkedAt = Date.now()
    if (p.keyHint === "sk-ds…92fD") {
      p.status = "error"
      p.error = "401: kunci ditolak atau kedaluwarsa"
    } else {
      p.status = "ok"
      p.error = undefined
      p.latencyMs = Math.round(500 + Math.random() * 1500)
      if (p.kind === "anthropic" || p.kind === "deepseek") p.balanceUsd = p.balanceUsd ?? 10
    }
    return structuredClone(p)
  }

  async saveAiBudget(input: AiBudgetInput): Promise<void> {
    this.authorize("pengaturan")
    await delay(300)
    if (input.monthlyUsd !== null && (!(input.monthlyUsd > 0) || input.monthlyUsd > 100000)) throw new Error(t("Batas bulanan harus 0-100000"))
    if (!Number.isInteger(input.alertPercent) || input.alertPercent < 10 || input.alertPercent > 100) throw new Error(t("Peringatan harus 10-100%"))
    this.aiBudget = { ...input }
  }

  private dashboard: DashboardLayout | null = null

  // ---- Bandwidth tiruan: bergantung jumlah user aktif dan jam (puncak malam), tanpa penyimpanan ----

  private trafficAt(routerId: string, iface: string, at: number) {
    const active = this.users.filter((u) => u.routerId === routerId && u.status === "active").length
    const hash = (s: string) => {
      let h = 2166136261
      for (const c of s) h = Math.imul(h ^ c.charCodeAt(0), 16777619)
      return ((h >>> 0) % 1000) / 1000
    }
    const hour = (((at / 3_600_000) % 24) + 7) % 24
    const wave = 0.35 + 0.65 * (0.5 + 0.5 * Math.cos((2 * Math.PI * (hour - 20.5)) / 24))
    const noise = 0.75 + 0.5 * hash(`${routerId}:${Math.floor(at / 60_000)}`)
    const down = active * 1_400_000 * wave * noise
    const up = down * 0.12
    if (iface === "@pppoe") return { rxBps: Math.round(up), txBps: Math.round(down), sessions: active }
    if (iface === "ether1") return { rxBps: Math.round(down * 1.03), txBps: Math.round(up * 1.03) }
    return { rxBps: Math.round(up * 0.9), txBps: Math.round(down * 0.9) }
  }

  async getTrafficInterfaces(routerId: string): Promise<TrafficInterfaces> {
    this.authorize("router", "read")
    await delay(150)
    if (!this.routers.some((r) => r.id === routerId)) throw new Error(t("Router tidak ditemukan"))
    const now = Date.now()
    const mk = (name: string, kind: string) => ({ name, kind, running: true, lastSeen: now })
    return {
      interfaces: [mk("@pppoe", "pppoe-aggregate"), mk("bridge-lan", "bridge"), mk("ether1", "ether"), mk("ether2", "ether")],
      enabled: this.settings.bandwidth.enabled,
      intervalSeconds: this.settings.bandwidth.intervalSeconds,
    }
  }

  async getTrafficSeries(q: TrafficQuery): Promise<TrafficSeries> {
    this.authorize("router", "read")
    await delay(200)
    const spans: Record<string, number> = { "1h": 3_600_000, "6h": 21_600_000, "24h": 86_400_000, "7d": 604_800_000, "30d": 2_592_000_000 }
    const span = spans[q.range]
    if (!span) throw new Error(t("Rentang tidak valid"))
    if (q.router === "all" && q.iface !== "@pppoe") throw new Error(t("Total semua router hanya untuk antarmuka @pppoe"))
    const ids = q.router === "all" ? this.routers.map((r) => r.id) : [q.router]
    if (q.router !== "all" && !this.routers.some((r) => r.id === q.router)) throw new Error(t("Router tidak ditemukan"))
    const { enabled, intervalSeconds } = this.settings.bandwidth
    const bucket = Math.max(intervalSeconds * 1000, Math.ceil(span / 240))
    const to = Date.now()
    const from = to - span
    const points: TrafficPoint[] = []
    if (enabled) {
      for (let at = Math.ceil(from / bucket) * bucket; at <= to; at += bucket) {
        const rows = ids.map((id) => this.trafficAt(id, q.iface, at))
        const p: TrafficPoint = { at, rxBps: rows.reduce((s, r) => s + r.rxBps, 0), txBps: rows.reduce((s, r) => s + r.txBps, 0) }
        if (q.iface === "@pppoe") p.sessions = rows.reduce((s, r) => s + (r.sessions ?? 0), 0)
        points.push(p)
      }
    }
    return { points, from, to, bucketSeconds: Math.round(bucket / 1000), intervalSeconds, enabled }
  }

  // Extension: data contoh tidak memuat extension (butuh server, ADR 0028).
  async getExtensions(): Promise<ExtensionList> {
    return { available: false, extensions: [] }
  }
  async getActiveExtensions(): Promise<ActiveExtension[]> {
    return []
  }
  private noExtensions(): never {
    throw new Error(t("Extension tidak tersedia pada data contoh"))
  }
  async inspectExtension(): Promise<ExtensionInspect> {
    return this.noExtensions()
  }
  async installExtension(): Promise<ExtensionInfo> {
    return this.noExtensions()
  }
  async updateExtension(): Promise<ExtensionInfo> {
    return this.noExtensions()
  }
  async getExtensionStore(): Promise<ExtensionStore> {
    return this.noExtensions()
  }
  async installFromStore(): Promise<ExtensionInfo[]> {
    return this.noExtensions()
  }
  async runExtensionWorker(): Promise<{ ok: boolean; error?: string; workers: ExtensionWorker[] }> {
    return this.noExtensions()
  }
  async getExtensionLogs(): Promise<string[]> {
    return this.noExtensions()
  }
  async getExtensionAccess(): Promise<ExtensionAccess> {
    return this.noExtensions()
  }
  async setExtensionAccess(): Promise<void> {
    return this.noExtensions()
  }
  async removeExtension(): Promise<void> {
    return this.noExtensions()
  }

  async getDashboard(): Promise<DashboardLayout | null> {
    this.authorize("ringkasan", "read")
    await delay(120)
    return this.dashboard ? structuredClone(this.dashboard) : null
  }

  async saveDashboard(layout: DashboardLayout | null): Promise<DashboardLayout | null> {
    this.authorize("ringkasan")
    await delay(250)
    if (layout && (layout.version !== 1 || layout.widgets.length > 60)) throw new Error(t("Tata letak tidak valid"))
    this.dashboard = layout ? structuredClone(layout) : null
    return this.dashboard ? structuredClone(this.dashboard) : null
  }

  async getSettings(): Promise<AppSettings> {
    this.authorize("pengaturan", "read")
    await delay(200)
    return structuredClone(this.settings)
  }

  async saveSettings(next: AppSettings): Promise<AppSettings> {
    this.authorize("pengaturan")
    await delay(400)
    if (this.currentSessionId && next.remote.allowPublicWrite !== this.settings.remote.allowPublicWrite) {
      throw new Error(t("Izin tulis lewat akses publik hanya bisa diubah dari aplikasi desktop"))
    }
    const { retention: r, polling: p, remote: m } = next
    this.checkRange(tk("Simpan event (hari)"), r.eventDays, 1, 3650, true)
    this.checkRange(tk("Simpan riwayat chat AI (hari)"), r.aiChatDays, 1, 3650, true)
    this.checkRange(tk("Maksimum event"), r.maxEvents, 1000, 5_000_000)
    this.checkRange(tk("Hapus permanen item terhapus (hari)"), r.deletedDays, 1, 3650, true)
    this.checkRange(tk("Simpan riwayat perubahan (hari)"), r.auditDays, 30, 3650, true)
    this.checkRange(tk("Interval sesi aktif (detik)"), p.sessionSeconds, 10, 600)
    this.checkRange(tk("Interval log router (detik)"), p.logSeconds, 10, 600)
    this.checkRange(tk("Import otomatis (jam)"), p.autoImportHours, 1, 168, true)
    this.checkRange(tk("Timeout koneksi (detik)"), p.timeoutSeconds, 3, 60)
    this.checkRange(tk("Percobaan ulang"), p.retries, 0, 5)
    this.checkRange(tk("Masa sesi login (jam)"), m.sessionHours, 1, 720)
    this.checkRange(tk("Batas gagal login"), m.maxFailures, 3, 20)
    this.checkRange(tk("Lama penguncian (menit)"), m.lockMinutes, 1, 1440)
    this.checkRange(tk("Interval bandwidth (detik)"), next.bandwidth.intervalSeconds, 10, 3600)
    this.checkRange(tk("Simpan data bandwidth (hari)"), next.bandwidth.retentionDays, 1, 365)
    this.settings = structuredClone(next)
    this.share.auth.allowPublicWrite = m.allowPublicWrite
    this.share.features.ai = next.ai.enabled
    this.share.features.aiMask = next.ai.maskSensitive
    this.shareChanged()
    return structuredClone(this.settings)
  }

  private purgeCounts(): PurgeCounts {
    const now = Date.now()
    const r = this.settings.retention
    let events = 0
    if (r.eventDays !== null) events = this.events.filter((e) => e.at < now - r.eventDays! * 86400_000).length
    events += Math.max(0, this.events.length - events - r.maxEvents)
    const old = (t?: number) => r.deletedDays !== null && t !== undefined && t < now - r.deletedDays * 86400_000
    const chatCutoff = r.aiChatDays === null ? null : now - r.aiChatDays * 86400_000
    const chats = chatCutoff === null ? 0 : [...this.chats.values()].flat().filter((c) => c.updatedAt < chatCutoff).length
    return {
      events,
      users: this.users.filter((u) => old(u.deletedAt)).length,
      profiles: this.profiles.filter((p) => old(p.deletedAt)).length,
      chats,
    }
  }

  async getStorageInfo(): Promise<StorageInfo> {
    this.authorize("pengaturan", "read")
    await delay(200)
    return {
      path: "<folder data aplikasi>/telemon.db",
      sizeBytes: 2_400_000 + this.events.length * 420 + this.users.length * 900,
      events: this.events.length,
      users: this.users.length,
      deletedUsers: this.users.filter((u) => u.deletedAt).length,
      deletedProfiles: this.profiles.filter((p) => p.deletedAt).length,
      chats: [...this.chats.values()].flat().length,
      purgeable: this.purgeCounts(),
    }
  }

  async purgeNow(): Promise<PurgeCounts> {
    this.authorize("pengaturan")
    await delay(800)
    const counts = this.purgeCounts()
    const now = Date.now()
    const r = this.settings.retention
    if (r.eventDays !== null) this.events = this.events.filter((e) => e.at >= now - r.eventDays! * 86400_000)
    this.events = this.events.slice(0, r.maxEvents)
    const keep = (x: { deletedAt?: number }) => !(r.deletedDays !== null && x.deletedAt !== undefined && x.deletedAt < now - r.deletedDays * 86400_000)
    const goneUsers = this.users.filter((u) => !keep(u))
    this.users = this.users.filter(keep)
    this.profiles = this.profiles.filter(keep)
    for (const u of goneUsers) delete this.metas[u.id]
    if (r.aiChatDays !== null) {
      const cutoff = now - r.aiChatDays * 86400_000
      for (const [k, list] of this.chats) this.chats.set(k, list.filter((c) => c.updatedAt >= cutoff))
    }
    for (const router of this.routers) {
      this.emit({
        kind: "inventory",
        routerId: router.id,
        users: this.users.filter((u) => u.routerId === router.id).map((u) => ({ ...u })),
        profiles: this.profiles.filter((p) => p.routerId === router.id).map((p) => ({ ...p })),
      })
    }
    return counts
  }

  // ---- Tulis ke router (data contoh: router "menjawab" setelah jeda) ----

  private writableRouter(routerId: string): RouterInfo {
    const r = this.routers.find((x) => x.id === routerId)
    if (!r) throw new Error(t("Router tidak ditemukan"))
    if (!r.canWrite) throw new Error(t("Akun API {name} tidak punya hak write", { name: r.name }))
    if (r.status !== "online") {
      throw new Error(t("Router {name} sedang {status}; tidak bisa menulis", { name: r.name, status: r.status === "offline" ? "offline" : "error" }))
    }
    return r
  }

  private userOrThrow(id: string) {
    const u = this.users.find((x) => x.id === id)
    if (!u) throw new Error(t("User tidak ditemukan"))
    return u
  }

  private profileOrThrow(id: string) {
    const p = this.profiles.find((x) => x.id === id)
    if (!p) throw new Error(t("Profil PPP tidak ditemukan"))
    return p
  }

  private checkProfileExists(routerId: string, name: string) {
    const p = this.profiles.find((x) => x.routerId === routerId && x.name === name)
    if (!p) throw new Error(t("Profil PPP \"{name}\" tidak ada di router", { name }))
    if (p.deletedAt) throw new Error(t("Profil PPP \"{name}\" sudah dihapus; pulihkan dulu", { name }))
  }

  private emitUser(u: PppUser) {
    this.emit({ kind: "user", user: { ...u } })
    return { ...u }
  }

  private emitProfile(p: PppProfile) {
    this.emit({ kind: "profile", profile: { ...p } })
    return { ...p }
  }

  private kickIfActive(u: PppUser, r: RouterInfo, why: string) {
    if (u.status !== "active") return
    const now = Date.now()
    u.connectedAt = undefined
    u.address = undefined
    u.lastSeenAt = now
    this.push(makeEvent("disconnected", now, r, u, why), u)
  }

  private validateRate(v?: string) {
    if (v && !/^\d+[kKmMgG]?(\/\d+[kKmMgG]?)?$/.test(v.trim())) {
      throw new Error(t("Rate limit tidak valid. Contoh: 10M/10M atau 5M"))
    }
  }

  async createUser(input: UserInput): Promise<PppUser> {
    this.authorize("user")
    await delay(800)
    const r = this.writableRouter(input.routerId)
    const username = input.username.trim()
    if (!/^[\w.@-]{2,40}$/.test(username)) {
      throw new Error(t("Username 2-40 karakter: huruf, angka, titik, garis bawah, strip, atau @"))
    }
    if (this.users.some((u) => u.routerId === r.id && u.username === username)) {
      throw new Error(t("Username sudah ada di router ini (cek juga yang sudah dihapus)"))
    }
    if (!input.password || input.password.length < 6) throw new Error(t("Password minimal 6 karakter"))
    this.checkProfileExists(r.id, input.profile)
    const u: PppUser = {
      id: `${r.id}:${username}`,
      routerId: r.id,
      username,
      profile: input.profile,
      status: input.disabled ? "disabled" : "inactive",
      password: input.password,
      staticAddress: input.staticAddress?.trim() || undefined,
      comment: input.comment?.trim() || undefined,
    }
    this.users.push(u)
    return this.emitUser(u)
  }

  async updateUser(id: string, patch: UserPatch): Promise<PppUser> {
    this.authorize("user")
    await delay(800)
    const u = this.userOrThrow(id)
    if (u.deletedAt) throw new Error(t("User sudah dihapus; pulihkan dulu"))
    const r = this.writableRouter(u.routerId)
    if (patch.password && patch.password.length < 6) throw new Error(t("Password minimal 6 karakter"))
    if (patch.profile !== undefined) {
      this.checkProfileExists(r.id, patch.profile)
      u.profile = patch.profile
    }
    if (patch.password) u.password = patch.password
    if ("staticAddress" in patch) u.staticAddress = patch.staticAddress?.trim() || undefined
    if ("comment" in patch) u.comment = patch.comment?.trim() || undefined
    if (patch.disabled === true && u.status !== "disabled") {
      this.kickIfActive(u, r, "secret dinonaktifkan")
      u.status = "disabled"
    } else if (patch.disabled === false && u.status === "disabled") {
      u.status = "inactive"
    }
    return this.emitUser(u)
  }

  async softDeleteUser(id: string): Promise<PppUser> {
    this.authorize("user")
    await delay(700)
    const u = this.userOrThrow(id)
    if (u.deletedAt) return { ...u }
    const r = this.writableRouter(u.routerId)
    this.kickIfActive(u, r, "secret dinonaktifkan")
    u.status = "disabled"
    u.deletedAt = Date.now()
    u.deletedReason = "manual" satisfies DeletedReason
    return this.emitUser(u)
  }

  async restoreUser(id: string): Promise<PppUser> {
    this.authorize("user")
    await delay(700)
    const u = this.userOrThrow(id)
    this.writableRouter(u.routerId)
    u.deletedAt = undefined
    u.deletedReason = undefined
    u.status = "inactive"
    return this.emitUser(u)
  }

  async createProfile(input: ProfileInput): Promise<PppProfile> {
    this.authorize("profile")
    await delay(800)
    const r = this.writableRouter(input.routerId)
    const name = input.name.trim()
    if (!/^[\w.-]{1,40}$/.test(name)) {
      throw new Error(t("Nama profil PPP 1-40 karakter: huruf, angka, titik, garis bawah, atau strip"))
    }
    if (this.profiles.some((p) => p.routerId === r.id && p.name === name)) {
      throw new Error(t("Nama profil PPP sudah ada di router ini (cek juga yang sudah dihapus)"))
    }
    this.validateRate(input.rateLimit)
    const p: PppProfile = {
      id: `${r.id}:${name}`,
      routerId: r.id,
      name,
      rateLimit: input.rateLimit?.trim() || undefined,
      localAddress: input.localAddress?.trim() || undefined,
      remoteAddress: input.remoteAddress?.trim() || undefined,
      onlyOne: input.onlyOne,
      comment: input.comment?.trim() || undefined,
    }
    this.profiles.push(p)
    return this.emitProfile(p)
  }

  async updateProfile(id: string, patch: ProfilePatch): Promise<PppProfile> {
    this.authorize("profile")
    await delay(800)
    const p = this.profileOrThrow(id)
    if (p.deletedAt) throw new Error(t("Profil PPP sudah dihapus; pulihkan dulu"))
    this.writableRouter(p.routerId)
    if ("rateLimit" in patch) {
      this.validateRate(patch.rateLimit)
      p.rateLimit = patch.rateLimit?.trim() || undefined
    }
    if ("localAddress" in patch) p.localAddress = patch.localAddress?.trim() || undefined
    if ("remoteAddress" in patch) p.remoteAddress = patch.remoteAddress?.trim() || undefined
    if (patch.onlyOne) p.onlyOne = patch.onlyOne
    if ("comment" in patch) p.comment = patch.comment?.trim() || undefined
    return this.emitProfile(p)
  }

  async softDeleteProfile(id: string): Promise<PppProfile> {
    this.authorize("profile")
    await delay(400)
    const p = this.profileOrThrow(id)
    const used = this.users.filter((u) => u.routerId === p.routerId && u.profile === p.name && !u.deletedAt).length
    if (used > 0) throw new Error(t("Profil PPP masih dipakai {n} user; pindahkan user-nya dulu", { n: used }))
    p.deletedAt = Date.now()
    p.deletedReason = "manual"
    return this.emitProfile(p)
  }

  async restoreProfile(id: string): Promise<PppProfile> {
    this.authorize("profile")
    await delay(400)
    const p = this.profileOrThrow(id)
    p.deletedAt = undefined
    p.deletedReason = undefined
    return this.emitProfile(p)
  }

  async importFromRouter(routerId: string): Promise<ImportResult> {
    this.authorize("router")
    await delay(1400)
    const r = this.routers.find((x) => x.id === routerId)
    if (!r) throw new Error(t("Router tidak ditemukan"))
    if (r.status !== "online") {
      throw new Error(t("Router {name} sedang {status}; tidak bisa import", { name: r.name, status: r.status === "offline" ? "offline" : "error" }))
    }
    const now = Date.now()
    const result: ImportResult = {
      routerId,
      at: now,
      users: { added: 0, updated: 0, deleted: 0 },
      profiles: { added: 0, updated: 0, deleted: 0 },
    }
    // Simulasi: pada import pertama sesi ini router punya perubahan yang belum diketahui Telemon.
    // Router source of truth: selisih selalu dimenangkan router.
    if (!this.importedOnce.has(routerId)) {
      this.importedOnce.add(routerId)
      const subnet = routerId.slice(1)
      if (!this.profiles.some((p) => p.routerId === routerId && p.name === "5Mbps")) {
        this.profiles.push({
          id: `${routerId}:5Mbps`, routerId, name: "5Mbps", rateLimit: "5M/5M",
          localAddress: `100.64.${subnet}.1`, remoteAddress: `pool-pppoe-${subnet}`, onlyOne: "default",
        })
        result.profiles.added++
      }
      const cached = this.profiles.find((p) => p.routerId === routerId && p.name === "20Mbps")
      if (cached) {
        cached.rateLimit = "20M/20M"
        cached.comment = "Disesuaikan di router"
        result.profiles.updated++
      }
      for (const name of ["pelanggan.baru1", "pelanggan.baru2"]) {
        if (this.users.some((u) => u.routerId === routerId && u.username === name)) continue
        this.users.push({
          id: `${routerId}:${name}`, routerId, username: name, profile: "10Mbps", status: "inactive",
          callerId: mac(), lastSeenAt: now - 86400_000, comment: "Dibuat langsung di router",
        })
        result.users.added++
      }
      const gone = this.users.find(
        (u) => u.routerId === routerId && u.status === "inactive" && !u.deletedAt && !u.username.startsWith("pelanggan.baru")
      )
      if (gone) {
        gone.deletedAt = now
        gone.deletedReason = "router"
        gone.status = "disabled"
        result.users.deleted++
      }
      result.users.updated += 3
    }
    r.lastImportAt = now
    this.emit({ kind: "router", router: { ...r } })
    this.emit({
      kind: "inventory",
      routerId,
      users: this.users.filter((u) => u.routerId === routerId).map((u) => ({ ...u })),
      profiles: this.profiles.filter((p) => p.routerId === routerId).map((p) => ({ ...p })),
    })
    return result
  }

  async updateRouter(id: string, input: RouterEdit): Promise<RouterInfo> {
    this.authorize("router")
    await delay(700)
    const r = this.routers.find((x) => x.id === id)
    if (!r) throw new Error(t("Router tidak ditemukan"))
    const name = input.name.trim()
    const host = input.host.trim()
    if (!name || name.length > 40) throw new Error(t("Nama router 1-40 karakter"))
    if (!host) throw new Error(t("Host/IP wajib diisi"))
    if (!Number.isInteger(input.port) || input.port < 1 || input.port > 65535) throw new Error(t("Port 1-65535"))
    Object.assign(r, { name, host, port: input.port, tls: input.tls, canWrite: input.allowWrite })
    this.emit({ kind: "router", router: { ...r } })
    return { ...r }
  }

  private updateChannel: "stable" | "rc" = "stable"
  private updateChecked: number | undefined

  private updateStatus() {
    return {
      supported: true,
      currentVersion: "0.1.0",
      canInstall: true,
      channel: this.updateChannel,
      checkedAt: this.updateChecked,
      available: this.updateChecked ? { version: this.updateChannel === "rc" ? "0.2.0-rc.1" : "0.1.1", notes: "Contoh catatan rilis: perbaikan kecil dan peningkatan kecepatan." } : undefined,
    }
  }

  async getUpdateStatus() {
    return this.updateStatus()
  }

  async checkUpdate() {
    await delay(700)
    this.updateChecked = Date.now()
    return this.updateStatus()
  }

  async setUpdateChannel(channel: "stable" | "rc") {
    this.updateChannel = channel
    this.updateChecked = undefined
    return this.updateStatus()
  }

  async installUpdate(): Promise<{ installed: string; backup: string }> {
    await delay(1500)
    throw new Error(t("Data contoh: pembaruan tidak benar-benar dipasang"))
  }

  async fetchFingerprint(host: string, port: number): Promise<string> {
    this.authorize("router")
    await delay(700)
    // Data contoh: sidik jari tetap per host dan port, bukan dari router sungguhan.
    let h = 2166136261
    const bytes = Array.from({ length: 32 }, (_, i) => {
      for (const c of `${host}:${port}:${i}`) h = Math.imul(h ^ c.charCodeAt(0), 16777619)
      return ((h >>> 0) & 255).toString(16).padStart(2, "0").toUpperCase()
    })
    return bytes.join(":")
  }

  async removeRouter(id: string) {
    this.authorize("router")
    await delay(300)
    this.routers = this.routers.filter((r) => r.id !== id)
    this.users = this.users.filter((u) => u.routerId !== id)
    this.profiles = this.profiles.filter((p) => p.routerId !== id)
    this.emit({ kind: "router-removed", routerId: id })
  }

  async saveUserMeta(userId: string, input: UserMetaInput): Promise<UserMeta> {
    this.authorize("user")
    await delay(300)
    if (!this.users.some((u) => u.id === userId)) throw new Error(t("User tidak ditemukan"))
    const clean = (v?: string) => v?.trim() || undefined
    const meta: UserMeta = {
      customerName: clean(input.customerName),
      phone: clean(input.phone),
      note: clean(input.note),
      tags: [...new Set(input.tags.map((t) => t.trim()).filter(Boolean))].slice(0, 8),
      updatedAt: Date.now(),
    }
    this.metas[userId] = meta
    this.emit({ kind: "user-meta", userId, meta })
    return meta
  }

  private shareChanged() {
    const s = structuredClone(this.share)
    this.emit({ kind: "share", share: s })
    return s
  }

  async setLanShare(opts: { enabled: boolean; bind: "lan" | "localhost"; port: number }) {
    this.authorize("desktop")
    await delay(400)
    const lan = this.share.lan
    lan.bind = opts.bind
    lan.port = opts.port
    lan.enabled = opts.enabled
    if (opts.enabled) {
      const host = opts.bind === "lan" ? "192.168.1.23" : "127.0.0.1"
      lan.url = `http://${host}:${opts.port}/`
    } else {
      lan.url = undefined
      this.stopPublic()
    }
    return this.shareChanged()
  }

  private stopPublic() {
    clearTimeout(this.expiryTimer)
    Object.assign(this.share.publik, { status: "off", url: undefined, expiresAt: undefined, error: undefined })
  }

  async setPublicShare(opts: { enabled: boolean; durationHours: number }) {
    this.authorize("desktop")
    const p = this.share.publik
    p.durationHours = opts.durationHours
    if (!opts.enabled) {
      await delay(300)
      this.stopPublic()
      return this.shareChanged()
    }
    if (!this.share.lan.enabled) throw new Error(t("Nyalakan Serve ke browser terlebih dulu"))
    if (this.accounts.every((a) => a.disabled)) throw new Error(t("Buat minimal satu akun jarak jauh terlebih dulu"))
    this.share.auth.require2fa = true
    p.status = "connecting"
    p.error = undefined
    this.shareChanged()
    await delay(1800)
    const words = [pick(TUNNEL_WORDS), pick(TUNNEL_WORDS), pick(TUNNEL_WORDS)].join("-")
    p.status = "on"
    p.url = `https://${words}.trycloudflare.com`
    p.expiresAt = Date.now() + opts.durationHours * 3600_000
    clearTimeout(this.expiryTimer)
    this.expiryTimer = setTimeout(() => {
      this.stopPublic()
      this.shareChanged()
    }, opts.durationHours * 3600_000)
    return this.shareChanged()
  }
}
