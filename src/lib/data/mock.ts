/**
 * Adapter data contoh. Meniru perilaku router Mikrotik agar UI bisa dibangun
 * sebelum poller Rust ada: user PPPoE di beberapa router, log event, dan
 * event live acak setiap beberapa detik.
 */
import type {
  DeletedReason,
  ImportResult,
  NewRouter,
  PppEvent,
  PppEventType,
  PppProfile,
  PppUser,
  ProfileInput,
  ProfilePatch,
  RouterInfo,
  AccountSummary,
  LoginResult,
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

interface MockAccount {
  id: string
  username: string
  displayName: string
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
    { id: "a1", username: "admin", displayName: "Gilang", password: DEMO_PASSWORD, totp: true, recovery: newRecoveryCodes(), disabled: false, createdAt: now - 30 * 86400_000, lastLoginAt: now - 3 * 3600_000 },
    { id: "a2", username: "operator", displayName: "Operator NOC", password: DEMO_PASSWORD, totp: false, recovery: [], disabled: false, createdAt: now - 12 * 86400_000, lastLoginAt: now - 2 * 86400_000 },
    { id: "a3", username: "tamu", displayName: "Tamu (nonaktif)", password: DEMO_PASSWORD, totp: false, recovery: [], disabled: true, createdAt: now - 5 * 86400_000 },
  ]
}

function makeSessions(now: number): MockSession[] {
  return [
    { id: "s-old1", accountId: "a1", device: "Chrome · Windows", ip: "10.10.3.193", createdAt: now - 2 * 86400_000, lastSeenAt: now - 3 * 3600_000 },
    { id: "s-old2", accountId: "a1", device: "Safari · iPhone", ip: "10.10.3.54", createdAt: now - 6 * 86400_000, lastSeenAt: now - 26 * 3600_000 },
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
    auth: { require2fa: false },
  }
  private accounts: MockAccount[]
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
  }

  async snapshot(): Promise<Snapshot> {
    await delay(300)
    return {
      routers: this.routers.map((r) => ({ ...r })),
      users: this.users.map((u) => ({ ...u })),
      profiles: this.profiles.map((p) => ({ ...p })),
      events: [...this.events],
      metas: structuredClone(this.metas),
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
    await delay(900)
    if (!input.username || !input.password) {
      throw new Error("Username dan password API wajib diisi")
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

  private info(a: MockAccount, sid: string): SessionInfo {
    return { id: sid, userId: a.id, username: a.username, displayName: a.displayName, twoFactor: a.totp }
  }

  private openSession(a: MockAccount, touch = true): SessionInfo {
    const now = Date.now()
    const s: MockSession = {
      id: `s${now.toString(36)}`, accountId: a.id, device: "Browser ini", ip: "10.10.3.193", createdAt: now, lastSeenAt: now,
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
    if (!s || !a) throw new Error("Belum login")
    return { a, s }
  }

  private fail(key: string) {
    const f = this.failures.get(key) ?? { n: 0, until: 0 }
    f.n++
    if (f.n >= 5) {
      f.until = Date.now() + 5 * 60_000
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
    if (!c || !a || c.expires < Date.now()) throw new Error("Sesi login kedaluwarsa. Masuk ulang")
    return a
  }

  private otpauth(a: MockAccount, secret: string) {
    return `otpauth://totp/Telemon:${a.username}?secret=${secret}&issuer=Telemon`
  }

  private checkPassword(a: MockAccount, password: string) {
    if (a.password !== password) throw new Error("Password salah")
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
      throw new Error(`Terlalu banyak percobaan. Coba lagi dalam ${Math.ceil((f.until - Date.now()) / 60_000)} menit`)
    }
    const a = this.accounts.find((x) => x.username === key)
    if (!a || a.password !== password) {
      this.fail(key)
      throw new Error("Username atau password salah")
    }
    if (a.disabled) throw new Error("Akun dinonaktifkan. Hubungi pemilik Telemon")
    this.failures.delete(key)
    if (a.totp) return { status: "2fa_required", challengeId: this.newChallenge(a) }
    if (this.share.auth.require2fa || this.share.publik.status === "on") {
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
      throw new Error(opts?.recovery ? "Kode pemulihan salah atau sudah dipakai" : "Kode salah")
    }
    this.challenges.delete(challengeId)
    return this.openSession(a)
  }

  async complete2faSetup(challengeId: string, code: string) {
    await delay(500)
    const a = this.takeChallenge(challengeId)
    if (code.replace(/\s/g, "") !== DEMO_TOTP) throw new Error("Kode salah")
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
    if (!name || name.length > 60) throw new Error("Nama 1-60 karakter")
    a.displayName = name
    return this.info(a, s.id)
  }

  async changeMyPassword(current: string, next: string) {
    await delay(500)
    const { a } = this.current()
    this.checkPassword(a, current)
    if (next.length < 8) throw new Error("Password baru minimal 8 karakter")
    if (next === current) throw new Error("Password baru harus berbeda")
    a.password = next
  }

  async start2faSetup(): Promise<TwoFactorSetup> {
    await delay(300)
    const { a } = this.current()
    if (a.totp) throw new Error("2FA sudah aktif")
    const secret = newSecret()
    a.pendingSecret = secret
    return { secret, otpauthUrl: this.otpauth(a, secret) }
  }

  async confirm2faSetup(code: string) {
    await delay(500)
    const { a } = this.current()
    if (!a.pendingSecret) throw new Error("Mulai pendaftaran 2FA terlebih dulu")
    if (code.replace(/\s/g, "") !== DEMO_TOTP) throw new Error("Kode salah")
    a.totp = true
    a.pendingSecret = undefined
    a.recovery = newRecoveryCodes()
    return { recoveryCodes: [...a.recovery] }
  }

  async disable2fa(password: string) {
    await delay(500)
    const { a } = this.current()
    this.checkPassword(a, password)
    if (this.share.auth.require2fa) throw new Error("2FA diwajibkan oleh pemilik Telemon")
    a.totp = false
    a.recovery = []
  }

  async regenerateRecoveryCodes(password: string) {
    await delay(500)
    const { a } = this.current()
    this.checkPassword(a, password)
    if (!a.totp) throw new Error("Aktifkan 2FA terlebih dulu")
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
    if (id === this.currentSessionId) throw new Error("Gunakan Keluar untuk sesi ini")
    this.sessions = this.sessions.filter((x) => x.id !== id)
  }

  // ---- Kelola akun (aplikasi desktop) ----

  private summary(a: MockAccount): AccountSummary {
    return { id: a.id, username: a.username, displayName: a.displayName, twoFactor: a.totp, disabled: a.disabled, createdAt: a.createdAt, lastLoginAt: a.lastLoginAt }
  }

  private accountOrThrow(id: string) {
    const a = this.accounts.find((x) => x.id === id)
    if (!a) throw new Error("Akun tidak ditemukan")
    return a
  }

  async listAccounts() {
    await delay(250)
    return this.accounts.map((a) => this.summary(a))
  }

  async createAccount(input: { username: string; displayName: string; password: string }) {
    await delay(500)
    const username = input.username.trim().toLowerCase()
    if (!/^[a-z0-9._-]{3,30}$/.test(username)) throw new Error("Username 3-30 karakter: huruf kecil, angka, titik, garis bawah, atau strip")
    if (this.accounts.some((a) => a.username === username)) throw new Error("Username sudah dipakai")
    if (!input.displayName.trim()) throw new Error("Nama wajib diisi")
    if (input.password.length < 8) throw new Error("Password minimal 8 karakter")
    const a: MockAccount = {
      id: `a${Date.now().toString(36)}`, username, displayName: input.displayName.trim(), password: input.password,
      totp: false, recovery: [], disabled: false, createdAt: Date.now(),
    }
    this.accounts.push(a)
    return this.summary(a)
  }

  async resetAccountPassword(id: string, password: string) {
    await delay(400)
    if (password.length < 8) throw new Error("Password minimal 8 karakter")
    const a = this.accountOrThrow(id)
    a.password = password
    this.sessions = this.sessions.filter((x) => x.accountId !== id)
  }

  async setAccountDisabled(id: string, disabled: boolean) {
    await delay(300)
    const a = this.accountOrThrow(id)
    a.disabled = disabled
    if (disabled) this.sessions = this.sessions.filter((x) => x.accountId !== id)
    return this.summary(a)
  }

  async resetAccount2fa(id: string) {
    await delay(300)
    const a = this.accountOrThrow(id)
    a.totp = false
    a.recovery = []
    this.sessions = this.sessions.filter((x) => x.accountId !== id)
    return this.summary(a)
  }

  async deleteAccount(id: string) {
    await delay(300)
    const a = this.accountOrThrow(id)
    const rest = this.accounts.filter((x) => x.id !== a.id && !x.disabled)
    if (rest.length === 0 && this.share.publik.status === "on") {
      throw new Error("Akses publik sedang aktif; matikan dulu sebelum menghapus akun terakhir")
    }
    this.accounts = this.accounts.filter((x) => x.id !== id)
    this.sessions = this.sessions.filter((x) => x.accountId !== id)
  }

  async setRequire2fa(required: boolean) {
    await delay(300)
    if (!required && this.share.publik.status === "on") throw new Error("Wajib selama akses publik aktif")
    this.share.auth.require2fa = required
    return this.shareChanged()
  }

  // ---- Tulis ke router (data contoh: router "menjawab" setelah jeda) ----

  private writableRouter(routerId: string): RouterInfo {
    const r = this.routers.find((x) => x.id === routerId)
    if (!r) throw new Error("Router tidak ditemukan")
    if (!r.canWrite) throw new Error(`Akun API ${r.name} tidak punya hak write`)
    if (r.status !== "online") {
      throw new Error(`Router ${r.name} sedang ${r.status === "offline" ? "offline" : "error"}; tidak bisa menulis`)
    }
    return r
  }

  private userOrThrow(id: string) {
    const u = this.users.find((x) => x.id === id)
    if (!u) throw new Error("User tidak ditemukan")
    return u
  }

  private profileOrThrow(id: string) {
    const p = this.profiles.find((x) => x.id === id)
    if (!p) throw new Error("Profile tidak ditemukan")
    return p
  }

  private checkProfileExists(routerId: string, name: string) {
    const p = this.profiles.find((x) => x.routerId === routerId && x.name === name)
    if (!p) throw new Error(`Profile "${name}" tidak ada di router`)
    if (p.deletedAt) throw new Error(`Profile "${name}" sudah dihapus; pulihkan dulu`)
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
      throw new Error("Rate limit tidak valid. Contoh: 10M/10M atau 5M")
    }
  }

  async createUser(input: UserInput): Promise<PppUser> {
    await delay(800)
    const r = this.writableRouter(input.routerId)
    const username = input.username.trim()
    if (!/^[\w.@-]{2,40}$/.test(username)) {
      throw new Error("Username 2-40 karakter: huruf, angka, titik, garis bawah, strip, atau @")
    }
    if (this.users.some((u) => u.routerId === r.id && u.username === username)) {
      throw new Error("Username sudah ada di router ini (cek juga yang sudah dihapus)")
    }
    if (!input.password || input.password.length < 6) throw new Error("Password minimal 6 karakter")
    this.checkProfileExists(r.id, input.profile)
    const u: PppUser = {
      id: `${r.id}:${username}`,
      routerId: r.id,
      username,
      profile: input.profile,
      status: input.disabled ? "disabled" : "inactive",
      staticAddress: input.staticAddress?.trim() || undefined,
      comment: input.comment?.trim() || undefined,
    }
    this.users.push(u)
    return this.emitUser(u)
  }

  async updateUser(id: string, patch: UserPatch): Promise<PppUser> {
    await delay(800)
    const u = this.userOrThrow(id)
    if (u.deletedAt) throw new Error("User sudah dihapus; pulihkan dulu")
    const r = this.writableRouter(u.routerId)
    if (patch.password && patch.password.length < 6) throw new Error("Password minimal 6 karakter")
    if (patch.profile !== undefined) {
      this.checkProfileExists(r.id, patch.profile)
      u.profile = patch.profile
    }
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
    await delay(700)
    const u = this.userOrThrow(id)
    this.writableRouter(u.routerId)
    u.deletedAt = undefined
    u.deletedReason = undefined
    u.status = "inactive"
    return this.emitUser(u)
  }

  async createProfile(input: ProfileInput): Promise<PppProfile> {
    await delay(800)
    const r = this.writableRouter(input.routerId)
    const name = input.name.trim()
    if (!/^[\w.-]{1,40}$/.test(name)) {
      throw new Error("Nama profile 1-40 karakter: huruf, angka, titik, garis bawah, atau strip")
    }
    if (this.profiles.some((p) => p.routerId === r.id && p.name === name)) {
      throw new Error("Nama profile sudah ada di router ini (cek juga yang sudah dihapus)")
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
    await delay(800)
    const p = this.profileOrThrow(id)
    if (p.deletedAt) throw new Error("Profile sudah dihapus; pulihkan dulu")
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
    await delay(400)
    const p = this.profileOrThrow(id)
    const used = this.users.filter((u) => u.routerId === p.routerId && u.profile === p.name && !u.deletedAt).length
    if (used > 0) throw new Error(`Profile masih dipakai ${used} user; pindahkan user-nya dulu`)
    p.deletedAt = Date.now()
    p.deletedReason = "manual"
    return this.emitProfile(p)
  }

  async restoreProfile(id: string): Promise<PppProfile> {
    await delay(400)
    const p = this.profileOrThrow(id)
    p.deletedAt = undefined
    p.deletedReason = undefined
    return this.emitProfile(p)
  }

  async importFromRouter(routerId: string): Promise<ImportResult> {
    await delay(1400)
    const r = this.routers.find((x) => x.id === routerId)
    if (!r) throw new Error("Router tidak ditemukan")
    if (r.status !== "online") {
      throw new Error(`Router ${r.name} sedang ${r.status === "offline" ? "offline" : "error"}; tidak bisa import`)
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

  async removeRouter(id: string) {
    await delay(300)
    this.routers = this.routers.filter((r) => r.id !== id)
    this.users = this.users.filter((u) => u.routerId !== id)
    this.profiles = this.profiles.filter((p) => p.routerId !== id)
    this.emit({ kind: "router-removed", routerId: id })
  }

  async saveUserMeta(userId: string, input: UserMetaInput): Promise<UserMeta> {
    await delay(300)
    if (!this.users.some((u) => u.id === userId)) throw new Error("User tidak ditemukan")
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
    const p = this.share.publik
    p.durationHours = opts.durationHours
    if (!opts.enabled) {
      await delay(300)
      this.stopPublic()
      return this.shareChanged()
    }
    if (!this.share.lan.enabled) throw new Error("Nyalakan Serve ke browser terlebih dulu")
    if (this.accounts.every((a) => a.disabled)) throw new Error("Buat minimal satu akun jarak jauh terlebih dulu")
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
