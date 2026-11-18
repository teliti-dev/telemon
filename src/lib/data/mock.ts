/**
 * Adapter data contoh. Meniru perilaku router Mikrotik agar UI bisa dibangun
 * sebelum poller Rust ada: user PPPoE di beberapa router, log event, dan
 * event live acak setiap beberapa detik.
 */
import type {
  NewRouter,
  PppEvent,
  PppEventType,
  PppUser,
  RouterInfo,
  ShareState,
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
    { id: "r1", name: "CCR-Pusat", host: "10.10.1.1", port: 8729, tls: true, model: "CCR2004-1G-12S+2XS", version: "7.16.2", status: "online", lastPollAt: now },
    { id: "r2", name: "RB-Timur", host: "10.10.2.1", port: 8729, tls: true, model: "RB4011iGS+", version: "7.15.3", status: "online", lastPollAt: now },
    { id: "r3", name: "hEX-Barat", host: "10.10.3.1", port: 8728, tls: false, model: "hEX S", version: "6.49.17", status: "online", lastPollAt: now },
    { id: "r4", name: "RB-Selatan", host: "10.10.4.1", port: 8728, tls: false, model: "RB1100AHx4", version: "7.14.3", status: "offline", lastPollAt: now - 47 * 60_000, error: "Tidak bisa terhubung ke 10.10.4.1:8728 (timeout 10 dtk)" },
  ]
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

const TUNNEL_WORDS = ["quiet", "river", "amber", "solar", "maple", "harbor", "nickel", "violet", "canyon", "pixel", "meadow", "tango"]

export class MockApi implements TelemonApi {
  readonly kind = "mock" as const
  private routers: RouterInfo[]
  private users: PppUser[]
  private events: PppEvent[]
  private share: ShareState = {
    lan: { enabled: false, bind: "lan", port: 7878 },
    publik: { status: "off", pinSet: false, durationHours: 8 },
  }
  private listeners = new Set<(u: LiveUpdate) => void>()
  private timer?: ReturnType<typeof setInterval>
  private expiryTimer?: ReturnType<typeof setTimeout>

  constructor() {
    const now = Date.now()
    this.routers = makeRouters(now)
    this.users = makeUsers(this.routers, now)
    this.events = makeHistory(this.routers, this.users, now)
  }

  async snapshot(): Promise<Snapshot> {
    await delay(300)
    return {
      routers: this.routers.map((r) => ({ ...r })),
      users: this.users.map((u) => ({ ...u })),
      events: [...this.events],
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
    const candidates = this.users.filter((u) => online.has(u.routerId))
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
      error: "Data contoh: router baru tidak benar-benar dihubungi",
    }
    this.routers.push(router)
    this.emit({ kind: "router", router: { ...router } })
    return router
  }

  async removeRouter(id: string) {
    await delay(300)
    this.routers = this.routers.filter((r) => r.id !== id)
    this.users = this.users.filter((u) => u.routerId !== id)
    this.emit({ kind: "router-removed", routerId: id })
  }

  private shareChanged() {
    const s = structuredClone(this.share)
    this.emit({ kind: "share", share: s })
    return s
  }

  private newToken() {
    return Array.from({ length: 4 }, () => Math.floor(rand() * 0xffffff).toString(36)).join("")
  }

  async setLanShare(opts: { enabled: boolean; bind: "lan" | "localhost"; port: number }) {
    await delay(400)
    const lan = this.share.lan
    lan.bind = opts.bind
    lan.port = opts.port
    lan.enabled = opts.enabled
    if (opts.enabled) {
      lan.token = this.newToken()
      const host = opts.bind === "lan" ? "192.168.1.23" : "127.0.0.1"
      lan.url = `http://${host}:${opts.port}/?t=${lan.token}`
    } else {
      lan.url = undefined
      lan.token = undefined
      this.stopPublic()
    }
    return this.shareChanged()
  }

  async regenerateToken() {
    await delay(200)
    const lan = this.share.lan
    if (lan.enabled && lan.url) {
      lan.token = this.newToken()
      lan.url = lan.url.replace(/\?t=.*/, `?t=${lan.token}`)
    }
    return this.shareChanged()
  }

  async setPublicPin(pin: string) {
    await delay(300)
    if (!/^\d{6,12}$/.test(pin)) throw new Error("PIN harus 6-12 digit angka")
    this.share.publik.pinSet = true
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
    if (!p.pinSet) throw new Error("Atur PIN terlebih dulu")
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
