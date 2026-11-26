import type {
  AccountSummary,
  AiBudgetInput,
  AiChat,
  AiChatSummary,
  AiDashboard,
  AiUiContext,
  DashboardLayout,
  TrafficInterfaces,
  TrafficQuery,
  TrafficSeries,
  AiMessage,
  AiProvider,
  AiProviderInput,
  AiStreamEvent,
  AppSettings,
  ImportResult,
  LoginResult,
  NewRouter,
  PppEvent,
  PppProfile,
  PppUser,
  ProfileInput,
  ProfilePatch,
  PurgeCounts,
  RouterEdit,
  RouterInfo,
  Role,
  RoleInput,
  SessionDevice,
  SessionInfo,
  ShareState,
  StorageInfo,
  TwoFactorSetup,
  UserInput,
  UserMeta,
  UserMetaInput,
  UpdateChannel,
  UpdateStatus,
  UserPatch,
} from "../types"
import { getLang } from "@/lib/i18n"
import type { LiveUpdate, Snapshot, TelemonApi } from "./api"

/** Galat dari server: `code` stabil (`forbidden`, `locked`, ...) dan pesan siap tampil. */
export class ApiError extends Error {
  readonly status: number
  readonly code: string
  constructor(message: string, status: number, code: string) {
    super(message)
    this.name = "ApiError"
    this.status = status
    this.code = code
  }
}

/** Dikirim saat server menjawab 401 di luar alur login: sesi berakhir atau dicabut. */
export const UNAUTHENTICATED_EVENT = "telemon:unauthenticated"

export interface HttpApiOptions {
  /** Awalan URL server (bawaan: asal yang sama). */
  baseUrl?: string
  fetch?: typeof fetch
  /** Selang sinkronisasi ulang (ms) untuk status router dan inventaris. 0 = mati. */
  pollMs?: number
  WebSocketCtor?: typeof WebSocket
}

/** JSON server memakai `null` untuk kolom kosong; tipe UI memakai kolom yang tidak ada. */
export function dropNulls<T>(v: T): T {
  if (Array.isArray(v)) return v.map(dropNulls) as unknown as T
  if (v && typeof v === "object") {
    const out: Record<string, unknown> = {}
    for (const [k, x] of Object.entries(v as Record<string, unknown>)) if (x !== null) out[k] = dropNulls(x)
    return out as T
  }
  return v
}

const enc = encodeURIComponent
const blank = (s?: string) => (s?.trim() ? s.trim() : null)
/** Kolom opsional yang bisa dikosongkan: tidak ada = tidak diubah, null = kosongkan. */
function clearable(patch: object, key: string, value?: string) {
  return key in patch ? { [key]: blank(value) } : {}
}

interface PlanResponse {
  planId: string
}

/**
 * Adapter browser: JSON + cookie sesi + WebSocket ke server Telemon (mode Serve/Publik).
 * Tulis ke router memakai rencana di server: buat rencana lalu terapkan; klien tidak mengirim
 * langkah ke router. Fitur yang tidak dipasang di server menolak dengan 501.
 */
export class HttpApi implements TelemonApi {
  readonly kind = "http" as const
  private readonly base: string
  private readonly f: typeof fetch
  private readonly pollMs: number
  private readonly WS: typeof WebSocket | undefined
  private routers: RouterInfo[] = []
  private listeners = new Set<(u: LiveUpdate) => void>()
  private lastInventory = new Map<string, string>()
  private lastRouters = new Map<string, string>()
  private seenEvents = new Set<string>()
  private lastShare = ""
  private ws: WebSocket | null = null
  private timer: ReturnType<typeof setInterval> | null = null
  private debounce: ReturnType<typeof setTimeout> | null = null
  private closed = true

  constructor(opts: HttpApiOptions = {}) {
    this.base = opts.baseUrl ?? ""
    this.f = opts.fetch ?? ((...a) => fetch(...a))
    this.pollMs = opts.pollMs ?? 15_000
    this.WS = opts.WebSocketCtor ?? (typeof WebSocket === "undefined" ? undefined : WebSocket)
  }

  // ---------------- dasar ----------------

  private async req<T>(method: string, path: string, body?: unknown, opts: { auth?: boolean; keepNulls?: boolean } = {}): Promise<T> {
    const init: RequestInit = { method, headers: body === undefined ? {} : { "Content-Type": "application/json" }, credentials: "same-origin" }
    if (body !== undefined) init.body = JSON.stringify(body)
    const res = await this.f(this.base + path, init)
    if (res.status === 204) return undefined as T
    let data: unknown = null
    try {
      data = await res.json()
    } catch {
      /* badan kosong atau bukan JSON */
    }
    if (!res.ok) {
      const e = (data as { error?: { code?: string; message?: string } } | null)?.error
      if (res.status === 401 && opts.auth !== false) {
        try {
          window.dispatchEvent(new Event(UNAUTHENTICATED_EVENT))
        } catch {
          /* bukan di browser */
        }
      }
      throw new ApiError(e?.message ?? `HTTP ${res.status}`, res.status, e?.code ?? "http")
    }
    return (opts.keepNulls ? data : dropNulls(data)) as T
  }

  // ---------------- snapshot dan live ----------------

  async snapshot(): Promise<Snapshot> {
    const d = await this.req<Snapshot>("GET", "/api/snapshot")
    this.routers = d.routers
    for (const e of d.events) this.seenEvents.add(e.id)
    this.remember(d)
    return { ...d, share: this.shareOf(d.share) }
  }

  /** Status berbagi dari server; bila server lama tidak mengirimnya, nilai aman untuk browser. */
  private shareOf(s?: ShareState): ShareState {
    if (s) return s
    return this.share()
  }

  /** Berbagi (serve/publik) dikelola dari aplikasi desktop; browser hanya perlu tahu fitur yang ada. */
  private share(): ShareState {
    const loc = typeof location === "undefined" ? undefined : location
    return {
      lan: { enabled: true, bind: "lan", port: Number(loc?.port) || 80, url: loc?.origin },
      publik: { status: "off", durationHours: 8 },
      auth: { require2fa: false, allowPublicWrite: false },
      features: { ai: false, aiMask: true },
    }
  }

  private remember(d: Pick<Snapshot, "routers" | "users" | "profiles">) {
    for (const r of d.routers) this.lastRouters.set(r.id, JSON.stringify(r))
    for (const r of d.routers) this.lastInventory.set(r.id, this.inventoryKey(d, r.id))
  }

  private inventoryKey(d: Pick<Snapshot, "users" | "profiles">, routerId: string) {
    return JSON.stringify([d.users.filter((u) => u.routerId === routerId), d.profiles.filter((p) => p.routerId === routerId)])
  }

  private emit(u: LiveUpdate) {
    for (const l of this.listeners) l(u)
  }

  /** Sinkron ulang dengan server dan kirim yang berubah (router, inventaris user/profil). */
  async poll(): Promise<Snapshot> {
    const d = await this.req<Snapshot>("GET", "/api/snapshot")
    const ids = new Set(d.routers.map((r) => r.id))
    for (const r of d.routers) {
      const key = JSON.stringify(r)
      if (this.lastRouters.get(r.id) !== key) {
        this.lastRouters.set(r.id, key)
        this.emit({ kind: "router", router: r })
      }
      const inv = this.inventoryKey(d, r.id)
      if (this.lastInventory.get(r.id) !== inv) {
        this.lastInventory.set(r.id, inv)
        this.emit({ kind: "inventory", routerId: r.id, users: d.users.filter((u) => u.routerId === r.id), profiles: d.profiles.filter((p) => p.routerId === r.id) })
      }
    }
    for (const id of [...this.lastRouters.keys()]) {
      if (!ids.has(id)) {
        this.lastRouters.delete(id)
        this.lastInventory.delete(id)
        this.emit({ kind: "router-removed", routerId: id })
      }
    }
    // Cadangan bila WebSocket putus: event baru yang belum pernah terlihat.
    for (const e of [...d.events].reverse()) {
      if (!this.seenEvents.has(e.id)) {
        this.seenEvents.add(e.id)
        this.emit({ kind: "event", event: e })
      }
    }
    this.routers = d.routers
    const share = this.shareOf(d.share)
    if (JSON.stringify(share) !== this.lastShare) {
      this.lastShare = JSON.stringify(share)
      this.emit({ kind: "share", share })
    }
    return { ...d, share }
  }

  subscribe(onUpdate: (u: LiveUpdate) => void): () => void {
    this.listeners.add(onUpdate)
    if (this.listeners.size === 1) this.start()
    return () => {
      this.listeners.delete(onUpdate)
      if (this.listeners.size === 0) this.stop()
    }
  }

  private start() {
    this.closed = false
    if (this.pollMs > 0) this.timer = setInterval(() => void this.poll().catch(() => undefined), this.pollMs)
    this.connect()
  }

  private stop() {
    this.closed = true
    if (this.timer) clearInterval(this.timer)
    if (this.debounce) clearTimeout(this.debounce)
    this.timer = this.debounce = null
    this.ws?.close()
    this.ws = null
  }

  private connect(delay = 1000) {
    const origin = this.base || (typeof location === "undefined" ? "" : location.origin)
    if (this.closed || !this.WS || !origin) return
    const url = origin.replace(/^http/, "ws") + "/api/ws"
    const ws = new this.WS(url)
    this.ws = ws
    ws.onmessage = (m) => {
      let msg: { kind?: string; event?: PppEvent }
      try {
        msg = JSON.parse(String(m.data))
      } catch {
        return
      }
      if (msg.kind === "resync") {
        void this.poll().catch(() => undefined)
      } else if (msg.kind === "event" && msg.event) {
        const event = dropNulls(msg.event)
        if (!this.seenEvents.has(event.id)) {
          this.seenEvents.add(event.id)
          this.emit({ kind: "event", event })
        }
        // Status user dan router berubah bersama event: sinkron ulang sebentar lagi (digabung).
        if (this.debounce) clearTimeout(this.debounce)
        this.debounce = setTimeout(() => void this.poll().catch(() => undefined), 400)
      }
    }
    ws.onclose = () => {
      if (this.ws === ws) this.ws = null
      if (!this.closed) setTimeout(() => this.connect(Math.min(delay * 2, 30_000)), delay)
    }
  }

  // ---------------- router ----------------

  async addRouter(input: NewRouter): Promise<RouterInfo> {
    const r = await this.req<RouterInfo>("POST", "/api/routers", input)
    this.routers = [...this.routers.filter((x) => x.id !== r.id), r]
    this.lastRouters.set(r.id, JSON.stringify(r))
    this.emit({ kind: "router", router: r })
    return r
  }

  async removeRouter(id: string): Promise<void> {
    await this.req("DELETE", `/api/routers/${enc(id)}`)
    this.routers = this.routers.filter((r) => r.id !== id)
    this.lastRouters.delete(id)
    this.lastInventory.delete(id)
    this.emit({ kind: "router-removed", routerId: id })
  }

  async updateRouter(id: string, input: RouterEdit): Promise<RouterInfo> {
    const cur = this.routers.find((r) => r.id === id)
    // TLS dikirim hanya bila berubah atau ada sidik jari baru; selebihnya pin lama dipertahankan.
    const body: Record<string, unknown> = { ...input }
    if (cur && input.tls === cur.tls && !input.fingerprint) delete body.tls
    const r = await this.req<RouterInfo>("PATCH", `/api/routers/${enc(id)}`, body)
    this.routers = this.routers.map((x) => (x.id === id ? r : x))
    this.lastRouters.set(r.id, JSON.stringify(r))
    this.emit({ kind: "router", router: r })
    return r
  }

  async fetchFingerprint(host: string, port: number): Promise<string> {
    const r = await this.req<{ fingerprint: string }>("POST", "/api/routers/fingerprint", { host, port })
    return r.fingerprint
  }

  async importFromRouter(routerId: string): Promise<ImportResult> {
    const r = await this.req<ImportResult>("POST", `/api/routers/${enc(routerId)}/import`, {})
    await this.poll()
    return r
  }

  // ---------------- tulis dengan rencana ----------------

  private async write(routerId: string, plan: Record<string, unknown>): Promise<void> {
    const p = await this.req<PlanResponse>("POST", `/api/routers/${enc(routerId)}/plans`, plan)
    await this.req("POST", `/api/plans/${enc(p.planId)}/apply`, {})
  }

  private async fresh<T>(pick: (s: Snapshot) => T | undefined): Promise<T> {
    const s = await this.poll()
    const v = pick(s)
    if (v === undefined) throw new ApiError("Data tidak ditemukan setelah disimpan", 404, "not_found")
    return v
  }

  private user = (id: string) => this.fresh<PppUser>((s) => s.users.find((u) => u.id === id))
  private profile = (id: string) => this.fresh<PppProfile>((s) => s.profiles.find((p) => p.id === id))

  async createUser(input: UserInput): Promise<PppUser> {
    await this.write(input.routerId, {
      action: "user.add",
      name: input.username,
      password: input.password ?? "",
      profile: input.profile,
      remoteAddress: blank(input.staticAddress),
      comment: blank(input.comment),
      disabled: input.disabled,
    })
    return this.user(`${input.routerId}:${input.username}`)
  }

  async updateUser(id: string, patch: UserPatch): Promise<PppUser> {
    const [routerId, ...rest] = id.split(":")
    const name = rest.join(":")
    try {
      await this.write(routerId, {
        action: "user.edit",
        name,
        password: patch.password || undefined,
        profile: patch.profile,
        disabled: patch.disabled,
        ...this.userClearables(patch),
      })
    } catch (e) {
      // Tidak ada yang berbeda di router: bukan galat bagi pengguna.
      if (!(e instanceof ApiError && /tidak ada perubahan/i.test(e.message))) throw e
    }
    return this.user(id)
  }

  private userClearables(patch: UserPatch) {
    return { ...(("staticAddress" in patch) ? { remoteAddress: blank(patch.staticAddress) } : {}), ...clearable(patch, "comment", patch.comment) }
  }

  async softDeleteUser(id: string): Promise<PppUser> {
    const [routerId, ...rest] = id.split(":")
    await this.write(routerId, { action: "user.soft_delete", name: rest.join(":") })
    return this.user(id)
  }

  async restoreUser(id: string): Promise<PppUser> {
    const [routerId, ...rest] = id.split(":")
    await this.write(routerId, { action: "user.restore", name: rest.join(":") })
    return this.user(id)
  }

  async createProfile(input: ProfileInput): Promise<PppProfile> {
    await this.write(input.routerId, {
      action: "profile.add",
      name: input.name,
      rateLimit: blank(input.rateLimit),
      localAddress: blank(input.localAddress),
      remoteAddress: blank(input.remoteAddress),
      onlyOne: input.onlyOne,
      comment: blank(input.comment),
    })
    return this.profile(`${input.routerId}:${input.name}`)
  }

  async updateProfile(id: string, patch: ProfilePatch): Promise<PppProfile> {
    const [routerId, ...rest] = id.split(":")
    try {
      await this.write(routerId, {
        action: "profile.edit",
        name: rest.join(":"),
        onlyOne: patch.onlyOne,
        ...clearable(patch, "rateLimit", patch.rateLimit),
        ...clearable(patch, "localAddress", patch.localAddress),
        ...clearable(patch, "remoteAddress", patch.remoteAddress),
        ...clearable(patch, "comment", patch.comment),
      })
    } catch (e) {
      if (!(e instanceof ApiError && /tidak ada perubahan/i.test(e.message))) throw e
    }
    return this.profile(id)
  }

  async softDeleteProfile(id: string): Promise<PppProfile> {
    await this.req("POST", `/api/profiles/${enc(id)}/soft-delete`, {})
    return this.profile(id)
  }

  async restoreProfile(id: string): Promise<PppProfile> {
    await this.req("POST", `/api/profiles/${enc(id)}/restore`, {})
    return this.profile(id)
  }

  async saveUserMeta(userId: string, meta: UserMetaInput): Promise<UserMeta> {
    const m = await this.req<UserMeta>("PUT", `/api/users/${enc(userId)}/meta`, meta)
    const full = { ...m, tags: m.tags ?? [] }
    this.emit({ kind: "user-meta", userId, meta: full })
    return full
  }

  /** Password PPPoE untuk detail user; hanya izin Ubah, dan setiap tampil tercatat di riwayat server. */
  async revealUserPassword(userId: string): Promise<string> {
    const r = await this.req<{ password: string }>("GET", `/api/users/${enc(userId)}/password`)
    return r.password
  }

  // ---------------- login dan akun sendiri ----------------

  async session(): Promise<SessionInfo | null> {
    try {
      return await this.req<SessionInfo>("GET", "/api/session", undefined, { auth: false })
    } catch (e) {
      if (e instanceof ApiError && e.status === 401) return null
      throw e
    }
  }

  async login(username: string, password: string): Promise<LoginResult> {
    return this.req<LoginResult>("POST", "/api/login", { username, password }, { auth: false })
  }

  async verify2fa(challengeId: string, code: string, opts?: { recovery?: boolean }): Promise<SessionInfo> {
    const r = await this.req<{ session: SessionInfo }>("POST", "/api/login/2fa", { challengeId, code, recovery: !!opts?.recovery }, { auth: false })
    return r.session
  }

  complete2faSetup(challengeId: string, code: string) {
    return this.req<{ session: SessionInfo; recoveryCodes: string[] }>("POST", "/api/login/2fa-setup", { challengeId, code }, { auth: false })
  }

  async logout(): Promise<void> {
    await this.req("POST", "/api/logout", {}, { auth: false })
  }

  updateMyProfile(input: { displayName: string }) {
    return this.req<SessionInfo>("PATCH", "/api/me/profile", input)
  }
  changeMyPassword(current: string, next: string) {
    return this.req<void>("POST", "/api/me/password", { current, next })
  }
  start2faSetup() {
    return this.req<TwoFactorSetup>("POST", "/api/me/2fa/start", {})
  }
  confirm2faSetup(code: string) {
    return this.req<{ recoveryCodes: string[] }>("POST", "/api/me/2fa/confirm", { code })
  }
  disable2fa(password: string) {
    return this.req<void>("POST", "/api/me/2fa/disable", { password })
  }
  async regenerateRecoveryCodes(password: string): Promise<string[]> {
    return (await this.req<{ recoveryCodes: string[] }>("POST", "/api/me/recovery-codes", { password })).recoveryCodes
  }
  listMySessions() {
    return this.req<SessionDevice[]>("GET", "/api/me/sessions")
  }
  revokeSession(id: string) {
    return this.req<void>("DELETE", `/api/me/sessions/${enc(id)}`)
  }

  // ---------------- akun dan peran ----------------

  listAccounts() {
    return this.req<AccountSummary[]>("GET", "/api/accounts")
  }
  createAccount(input: { username: string; displayName: string; password: string; roleId: string }) {
    return this.req<AccountSummary>("POST", "/api/accounts", input)
  }
  setAccountRole(id: string, roleId: string) {
    return this.req<AccountSummary>("POST", `/api/accounts/${enc(id)}/role`, { roleId })
  }
  resetAccountPassword(id: string, password: string) {
    return this.req<void>("POST", `/api/accounts/${enc(id)}/password`, { password })
  }
  setAccountDisabled(id: string, disabled: boolean) {
    return this.req<AccountSummary>("POST", `/api/accounts/${enc(id)}/disabled`, { disabled })
  }
  resetAccount2fa(id: string) {
    return this.req<AccountSummary>("POST", `/api/accounts/${enc(id)}/reset-2fa`, {})
  }
  deleteAccount(id: string) {
    return this.req<void>("DELETE", `/api/accounts/${enc(id)}`)
  }
  listRoles() {
    return this.req<Role[]>("GET", "/api/roles")
  }
  createRole(input: RoleInput) {
    return this.req<Role>("POST", "/api/roles", input)
  }
  updateRole(id: string, input: RoleInput) {
    return this.req<Role>("PUT", `/api/roles/${enc(id)}`, input)
  }
  deleteRole(id: string) {
    return this.req<void>("DELETE", `/api/roles/${enc(id)}`)
  }

  // ---------------- pengaturan dan penyimpanan ----------------

  getTrafficInterfaces(routerId: string) {
    return this.req<TrafficInterfaces>("GET", `/api/traffic/interfaces?router=${encodeURIComponent(routerId)}`)
  }
  getTrafficSeries(q: TrafficQuery) {
    const qs = new URLSearchParams({ router: q.router, iface: q.iface, range: q.range })
    return this.req<TrafficSeries>("GET", `/api/traffic/series?${qs}`)
  }
  async getDashboard() {
    const r = await this.req<{ layout?: DashboardLayout | null }>("GET", "/api/dashboard", undefined, { keepNulls: true })
    return r.layout ?? null
  }
  async saveDashboard(layout: DashboardLayout | null) {
    const r = await this.req<{ layout?: DashboardLayout | null }>("PUT", "/api/dashboard", { layout }, { keepNulls: true })
    return r.layout ?? null
  }
  getSettings() {
    // null berarti "selamanya": jangan dibuang.
    return this.req<AppSettings>("GET", "/api/settings", undefined, { keepNulls: true })
  }
  saveSettings(settings: AppSettings) {
    return this.req<AppSettings>("PUT", "/api/settings", settings, { keepNulls: true })
  }
  getStorageInfo() {
    return this.req<StorageInfo>("GET", "/api/storage")
  }
  purgeNow() {
    return this.req<PurgeCounts>("POST", "/api/storage/purge", {})
  }

  // ---------------- pembaruan aplikasi (hanya desktop) ----------------

  getUpdateStatus() {
    return this.req<UpdateStatus>("GET", "/api/update")
  }
  checkUpdate() {
    return this.req<UpdateStatus>("POST", "/api/update/check", {})
  }
  setUpdateChannel(channel: UpdateChannel) {
    return this.req<UpdateStatus>("POST", "/api/update/channel", { channel })
  }
  async installUpdate(): Promise<{ installed: string; backup: string; restarting?: boolean }> {
    try {
      return await this.req<{ installed: string; backup: string }>("POST", "/api/update/install", {})
    } catch (e) {
      // Aplikasi menutup atau memulai ulang sendiri saat memasang: sambungan terputus bukan kegagalan.
      if (e instanceof TypeError) return { installed: "", backup: "", restarting: true }
      throw e
    }
  }

  // ---------------- AI (provider, anggaran, dasbor) ----------------

  getAiDashboard(days: 7 | 30): Promise<AiDashboard> {
    return this.req<AiDashboard>("GET", `/api/ai/dashboard?days=${days}`, undefined, { keepNulls: true })
  }
  saveAiProvider(id: string | null, input: AiProviderInput): Promise<AiProvider> {
    const body = { ...input, apiKey: input.apiKey?.trim() || undefined }
    return id
      ? this.req<AiProvider>("PUT", `/api/ai/providers/${encodeURIComponent(id)}`, body, { keepNulls: true })
      : this.req<AiProvider>("POST", "/api/ai/providers", body, { keepNulls: true })
  }
  async removeAiProvider(id: string): Promise<void> {
    await this.req("DELETE", `/api/ai/providers/${encodeURIComponent(id)}`)
  }
  async setDefaultAiProvider(id: string): Promise<void> {
    await this.req("POST", `/api/ai/providers/${encodeURIComponent(id)}/default`, {})
  }
  testAiProvider(id: string): Promise<AiProvider> {
    return this.req<AiProvider>("POST", `/api/ai/providers/${encodeURIComponent(id)}/test`, {}, { keepNulls: true })
  }
  async saveAiBudget(input: AiBudgetInput): Promise<void> {
    await this.req("PUT", "/api/ai/budget", input)
  }

  // ---------------- AI chat ----------------

  /**
   * Pertanyaan dialirkan server sebagai NDJSON (satu objek JSON per baris). Galat sebelum jawaban dimulai datang
   * sebagai respons HTTP biasa; galat sesudahnya sebagai event `error`.
   */
  async *askAi(messages: AiMessage[], opts?: { signal?: AbortSignal; ui?: AiUiContext }): AsyncGenerator<AiStreamEvent> {
    const slim = messages.map((m) => ({ id: m.id, role: m.role, text: m.text, at: m.at, ...(m.error ? { error: true } : {}) }))
    const res = await this.f(this.base + "/api/ai/ask", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      credentials: "same-origin",
      body: JSON.stringify({ messages: slim, lang: getLang(), ...(opts?.ui ? { ui: opts.ui } : {}) }),
      signal: opts?.signal,
    })
    if (!res.ok) {
      let e: { code?: string; message?: string } | undefined
      try {
        e = ((await res.json()) as { error?: { code?: string; message?: string } }).error
      } catch {
        /* bukan JSON */
      }
      if (res.status === 401) {
        try {
          window.dispatchEvent(new Event(UNAUTHENTICATED_EVENT))
        } catch {
          /* bukan di browser */
        }
      }
      throw new ApiError(e?.message ?? `HTTP ${res.status}`, res.status, e?.code ?? "http")
    }
    if (!res.body) throw new ApiError("Server tidak mengirim aliran jawaban", 502, "no_stream")
    const reader = res.body.getReader()
    const decoder = new TextDecoder()
    let buf = ""
    const parse = (line: string): AiStreamEvent | { type: "error"; message: string } | null => {
      if (!line.trim()) return null
      try {
        return JSON.parse(line) as AiStreamEvent | { type: "error"; message: string }
      } catch {
        return null
      }
    }
    try {
      for (;;) {
        const { value, done } = await reader.read()
        if (value) buf += decoder.decode(value, { stream: !done })
        let nl: number
        while ((nl = buf.indexOf("\n")) >= 0) {
          const ev = parse(buf.slice(0, nl))
          buf = buf.slice(nl + 1)
          if (!ev) continue
          if (ev.type === "error") throw new ApiError((ev as { message: string }).message, 502, "ai_upstream")
          yield ev as AiStreamEvent
        }
        if (done) break
      }
      const last = parse(buf)
      if (last) {
        if (last.type === "error") throw new ApiError((last as { message: string }).message, 502, "ai_upstream")
        yield last as AiStreamEvent
      }
    } finally {
      reader.cancel().catch(() => {})
    }
  }
  listAiChats(): Promise<AiChatSummary[]> {
    return this.req<AiChatSummary[]>("GET", "/api/ai/chats")
  }
  getAiChat(id: string): Promise<AiChat> {
    return this.req<AiChat>("GET", `/api/ai/chats/${encodeURIComponent(id)}`)
  }
  saveAiChat(chat: AiChat): Promise<AiChatSummary> {
    return this.req<AiChatSummary>("PUT", `/api/ai/chats/${encodeURIComponent(chat.id)}`, chat)
  }
  async deleteAiChat(id: string): Promise<void> {
    await this.req("DELETE", `/api/ai/chats/${encodeURIComponent(id)}`)
  }

  // ---------------- berbagi (hanya aplikasi desktop; server menolak akun jarak jauh) ----------------

  private async shareCall(path: string, body: unknown): Promise<ShareState> {
    const s = this.shareOf(await this.req<ShareState>("POST", path, body))
    this.lastShare = JSON.stringify(s)
    this.emit({ kind: "share", share: s })
    return s
  }
  setRequire2fa(required: boolean): Promise<ShareState> {
    return this.shareCall("/api/share/require-2fa", { required })
  }
  setLanShare(opts: { enabled: boolean; bind: "lan" | "localhost"; port: number }): Promise<ShareState> {
    return this.shareCall("/api/share/lan", opts)
  }
  setPublicShare(opts: { enabled: boolean; durationHours: number }): Promise<ShareState> {
    return this.shareCall("/api/share/publik", opts)
  }
}
