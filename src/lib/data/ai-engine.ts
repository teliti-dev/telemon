/**
 * Mesin jawaban AI chat untuk mode data contoh. Menjawab pertanyaan tentang data Telemon
 * (hanya membaca) dalam bahasa yang sedang aktif, lengkap dengan widget, tombol pintasan,
 * saran lanjutan, sumber data, dan langkah kerja. Di aplikasi sungguhan, bagian ini diganti
 * model bahasa yang memakai fungsi baca data yang sama dengan izin peran penanya.
 */
import { formatDuration, formatRelative } from "../format"
import { t, tk } from "../i18n"
import { formatBps } from "../format"
import type {
  AiAction,
  AiTone,
  AiUiContext,
  AiWidget,
  LayoutOp,
  PppEvent,
  PppEventType,
  PppProfile,
  PppUser,
  Resource,
  RouterInfo,
  UserMeta,
} from "../types"

export interface AiContext {
  routers: RouterInfo[]
  /** Hanya user yang belum dihapus; kosong bila penanya tidak boleh membaca Users. */
  users: PppUser[]
  profiles: PppProfile[]
  events: PppEvent[]
  metas: Record<string, UserMeta>
  now: number
  /** Samarkan IP dan MAC dalam jawaban. */
  mask: boolean
  can: (r: Resource) => boolean
  /** Tampilan Ringkasan dari klien (untuk usulan tata letak). */
  ui?: AiUiContext
  /** Peran penanya boleh mengubah tata letak Ringkasan. */
  canEditLayout?: boolean
  /** Deret bandwidth total pelanggan (24 jam) bila penanya boleh melihat Router; unduh = txBps. */
  series?: { at: number; rxBps: number; txBps: number; sessions?: number }[]
}

/** Pertanyaan yang butuh deret bandwidth; dipakai pemanggil untuk menyiapkannya. */
export const wantsBandwidth = (q: string) => /bandwidth|trafik|traffic|kecepatan|speed|unduh|unggah|download|upload|pemakaian internet/i.test(q)

export interface AiAnswer {
  /** Markdown sederhana: paragraf, daftar "- ", **tebal**, `kode`. */
  text: string
  widgets?: AiWidget[]
  actions?: AiAction[]
  followUps?: string[]
  sources?: string[]
  steps?: string[]
}

/** Contoh pertanyaan; dipakai panel chat dan bantuan. Diterjemahkan saat dirender. */
export const AI_SUGGESTIONS = [
  tk("Berapa user aktif sekarang?"),
  tk("Router mana yang bermasalah?"),
  tk("Siapa yang sering gagal auth?"),
  tk("Tampilkan user tidak aktif"),
  tk("Profil PPP mana yang paling banyak dipakai?"),
  tk("Siapa yang sering putus?"),
  tk("Tampilkan event terbaru"),
  tk("Statistik event 24 jam terakhir"),
  tk("Berapa bandwidth pelanggan sekarang?"),
  tk("Bagaimana kesehatan router?"),
]

const STOP = new Set([
  "user", "users", "berapa", "siapa", "yang", "router", "aktif", "event", "detail", "cari", "find", "show", "tampilkan",
  "status", "about", "untuk", "dengan", "apa", "what", "who", "how", "many", "tidak", "gagal", "auth", "profil", "profile",
  "paket", "terbaru", "latest", "check", "cek", "sering", "putus", "mana", "bermasalah", "sekarang", "now", "the", "ada",
  "dari", "ini", "itu", "saya", "ppp", "online", "offline", "login", "disconnect", "failed", "recent", "events", "inactive",
  "list", "daftar", "csv",
])

const maskIp = (s?: string) => (s ? s.replace(/\.\d+$/, ".•••") : "-")
const maskMac = (s?: string) => (s ? `${s.split(":").slice(0, 3).join(":")}:••:••:••` : "-")
const plain = (s: string) => s.replace(/\*\*/g, "").replace(/`/g, "")

function eventWord(type: PppEventType) {
  switch (type) {
    case "connected":
      return t("terhubung")
    case "disconnected":
      return t("terputus")
    case "auth_failed":
      return t("gagal auth")
    case "authenticated":
      return t("terautentikasi")
    case "router_down":
      return t("router mati")
    case "router_up":
      return t("router hidup")
  }
}

const eventTone = (type: PppEventType): AiTone =>
  type === "auth_failed" || type === "router_down" ? "bad" : type === "disconnected" ? "warn" : type === "connected" || type === "router_up" ? "good" : "muted"

function statusWord(u: PppUser) {
  if (u.deletedAt) return t("dihapus")
  if (u.status === "active") return t("aktif")
  if (u.status === "inactive") return t("tidak aktif")
  return t("dinonaktifkan")
}

const noAccess = (resource: Resource): AiAnswer => {
  const name =
    resource === "user" ? t("Users") : resource === "profile" ? t("Profil PPP") : resource === "event" ? t("Event") : t("Router")
  return { text: t("Peran Anda tidak punya akses ke data {name}, jadi saya tidak bisa menjawab itu.", { name }) }
}

const routerName = (c: AiContext, id: string) => c.routers.find((r) => r.id === id)?.name ?? id

function help(): AiAnswer {
  return {
    text: [
      t("Saya bisa membantu membaca data Telemon. Saya hanya membaca; tidak mengubah apa pun."),
      t("Untuk widget di Ringkasan, saya hanya mengusulkan perubahan (mis. \"tambahkan widget total bandwidth\"); baru berlaku setelah Anda menekan Terapkan."),
      t("Coba tanyakan:"),
      ...AI_SUGGESTIONS.map((s) => `- ${t(s /* i18n-ignore: kunci tk */)}`),
      t("Atau sebut username untuk melihat detailnya, misalnya {example}.", { example: "`budi.rt01`" }),
      t("Ketik / untuk melihat perintah cepat."),
    ].join("\n"),
    followUps: [t("Berapa user aktif sekarang?"), t("Router mana yang bermasalah?")],
  }
}

function summary(c: AiContext): AiAnswer {
  const online = c.routers.filter((r) => r.status === "online").length
  const lines = [t("**Ringkasan saat ini**"), t("- Router online: **{online}** dari {total}", { online, total: c.routers.length })]
  const items: Extract<AiWidget, { kind: "stats" }>["items"] = [
    { label: t("Router online"), value: `${online}/${c.routers.length}`, tone: online === c.routers.length ? "good" : "bad" },
  ]
  const sources = [t("Router")]
  if (c.can("user")) {
    const count = (s: PppUser["status"]) => c.users.filter((u) => u.status === s).length
    const active = count("active")
    lines.push(
      t("- User aktif: **{n}** dari {total} user", { n: active, total: c.users.length }),
      t("- Tidak aktif: **{n}**, dinonaktifkan: **{m}**", { n: count("inactive"), m: count("disabled") })
    )
    items.push(
      { label: t("User aktif"), value: String(active), tone: "good" },
      { label: t("Tidak aktif"), value: String(count("inactive")), tone: count("inactive") > 0 ? "warn" : "muted" }
    )
    sources.push(t("Users"))
  }
  if (c.can("event")) {
    const n = c.events.filter((e) => e.type === "auth_failed" && c.now - e.at < 3600_000).length
    lines.push(t("- Gagal auth dalam 1 jam: **{n}**", { n }))
    items.push({ label: t("Gagal auth (1 jam)"), value: String(n), tone: n > 0 ? "bad" : "good" })
    sources.push(t("Event"))
  }
  const text = lines.join("\n")
  return {
    text,
    widgets: [{ kind: "stats", items }],
    actions: [
      { id: "open-users", label: t("Buka Users"), to: { page: "user" } },
      { id: "copy", label: t("Salin ringkasan"), copy: plain(text) },
    ],
    followUps: [t("Router mana yang bermasalah?"), t("Siapa yang sering gagal auth?"), t("Tampilkan user tidak aktif")],
    sources,
    steps: [t("Memahami pertanyaan"), t("Membaca data router"), t("Menghitung hasil")],
  }
}

function routerProblems(c: AiContext): AiAnswer {
  if (!c.can("router")) return noAccess("router")
  if (c.routers.length === 0) return { text: t("Tidak ada data router.") }
  const bad = c.routers.filter((r) => r.status !== "online")
  if (bad.length === 0) {
    return { text: t("Semua {n} router online.", { n: c.routers.length }), sources: [t("Router")], followUps: [t("Berapa user aktif sekarang?")] }
  }
  return {
    text: [
      t("Router yang bermasalah:"),
      ...bad.map((r) =>
        t("- **{name}** ({host}): {status}; data terakhir {when}", {
          name: r.name,
          host: r.host,
          status: r.status === "offline" ? t("offline") : t("error"),
          when: r.lastPollAt ? formatRelative(r.lastPollAt, c.now) : "-",
        })
      ),
    ].join("\n"),
    widgets: [
      {
        kind: "list",
        title: t("Router bermasalah"),
        rows: bad.map((r) => ({
          primary: r.name,
          secondary: r.error ?? `${r.host}:${r.port}`,
          trailing: r.lastPollAt ? formatRelative(r.lastPollAt, c.now) : "-",
          tone: r.status === "offline" ? ("bad" as const) : ("warn" as const),
          to: { page: "router" },
        })),
      },
    ],
    actions: [{ id: "open-router", label: t("Buka Router"), to: { page: "router" } }],
    followUps: [t("Berapa user aktif sekarang?"), t("Tampilkan event terbaru")],
    sources: [t("Router")],
    steps: [t("Memahami pertanyaan"), t("Membaca data router")],
  }
}

function top(c: AiContext, type: PppEventType) {
  const m = new Map<string, { n: number; last: number; routerId: string; username: string }>()
  for (const e of c.events) {
    if (e.type !== type || !e.username || c.now - e.at > 86400_000) continue
    const k = `${e.routerId}:${e.username}`
    const cur = m.get(k)
    if (cur) cur.n++
    else m.set(k, { n: 1, last: e.at, routerId: e.routerId, username: e.username })
  }
  return [...m.values()].sort((a, b) => b.n - a.n || b.last - a.last).slice(0, 5)
}

function perHour(c: AiContext, type: PppEventType, hours = 12) {
  const buckets = Array.from({ length: hours }, () => 0)
  for (const e of c.events) {
    const age = c.now - e.at
    if (e.type === type && age >= 0 && age < hours * 3600_000) buckets[hours - 1 - Math.floor(age / 3600_000)]++
  }
  return buckets.map((value, i) => ({ label: `${String(new Date(c.now - (hours - 1 - i) * 3600_000).getHours()).padStart(2, "0")}:00`, value }))
}

function authFailures(c: AiContext): AiAnswer {
  if (!c.can("event")) return noAccess("event")
  const h = c.events.filter((e) => e.type === "auth_failed" && c.now - e.at < 3600_000).length
  const d = c.events.filter((e) => e.type === "auth_failed" && c.now - e.at < 86400_000).length
  if (d === 0) return { text: t("Tidak ada gagal autentikasi dalam 24 jam terakhir."), sources: [t("Event")] }
  const list = top(c, "auth_failed")
  return {
    text: [
      t("Gagal autentikasi: **{h}** dalam 1 jam, **{d}** dalam 24 jam.", { h, d }),
      t("Paling sering:"),
      ...list.map((x) =>
        t("- **{user}** ({router}): {n}×, terakhir {when}", { user: x.username, router: routerName(c, x.routerId), n: x.n, when: formatRelative(x.last, c.now) })
      ),
    ].join("\n"),
    widgets: [
      { kind: "bars", title: t("Gagal auth per jam (12 jam terakhir)"), data: perHour(c, "auth_failed") },
      {
        kind: "list",
        title: t("Gagal auth teratas"),
        rows: list.map((x) => ({
          primary: x.username,
          secondary: routerName(c, x.routerId),
          trailing: `${x.n}×`,
          tone: "bad" as const,
          to: { page: "user", params: { q: x.username } },
        })),
      },
    ],
    actions: [{ id: "open-events", label: t("Buka Event gagal auth"), to: { page: "event", params: { type: "auth_failed" } } }],
    followUps: [t("Siapa yang sering putus?"), t("Tampilkan event terbaru")],
    sources: [t("Event")],
    steps: [t("Memahami pertanyaan"), t("Membaca event 24 jam terakhir"), t("Menghitung hasil")],
  }
}

function flapping(c: AiContext): AiAnswer {
  if (!c.can("event")) return noAccess("event")
  const list = top(c, "disconnected").filter((x) => x.n >= 2)
  if (list.length === 0) return { text: t("Tidak ada user yang sering terputus dalam 24 jam terakhir."), sources: [t("Event")] }
  return {
    text: [
      t("Paling sering terputus (24 jam):"),
      ...list.map((x) => t("- **{user}** ({router}): terputus {n}×", { user: x.username, router: routerName(c, x.routerId), n: x.n })),
    ].join("\n"),
    widgets: [
      {
        kind: "list",
        title: t("Paling sering terputus"),
        rows: list.map((x) => ({
          primary: x.username,
          secondary: routerName(c, x.routerId),
          trailing: `${x.n}×`,
          tone: "warn" as const,
          to: { page: "user", params: { q: x.username } },
        })),
      },
    ],
    actions: [{ id: "open-events", label: t("Buka Event"), to: { page: "event", params: { type: "disconnected" } } }],
    followUps: [t("Siapa yang sering gagal auth?")],
    sources: [t("Event")],
    steps: [t("Memahami pertanyaan"), t("Membaca event 24 jam terakhir"), t("Menghitung hasil")],
  }
}

function profileUsage(c: AiContext): AiAnswer {
  if (!c.can("profile") || !c.can("user")) return noAccess(c.can("user") ? "profile" : "user")
  const m = new Map<string, { n: number; active: number }>()
  for (const u of c.users) {
    const cur = m.get(u.profile) ?? { n: 0, active: 0 }
    cur.n++
    if (u.status === "active") cur.active++
    m.set(u.profile, cur)
  }
  if (m.size === 0) return { text: t("Belum ada profil PPP yang dipakai.") }
  const sorted = [...m.entries()].sort((a, b) => b[1].n - a[1].n).slice(0, 8)
  return {
    text: [t("Pemakaian profil PPP:"), ...sorted.map(([name, v]) => t("- **{name}**: {n} user, {active} aktif", { name, n: v.n, active: v.active }))].join("\n"),
    widgets: [{ kind: "bars", title: t("User per profil PPP"), data: sorted.map(([label, v]) => ({ label, value: v.n })) }],
    actions: [{ id: "open-profile", label: t("Buka Profil PPP"), to: { page: "profile" } }],
    followUps: [t("Berapa user aktif sekarang?")],
    sources: [t("Users"), t("Profil PPP")],
    steps: [t("Memahami pertanyaan"), t("Membaca data user"), t("Menghitung hasil")],
  }
}

function inactiveUsers(c: AiContext): AiAnswer {
  if (!c.can("user")) return noAccess("user")
  const list = c.users.filter((u) => u.status === "inactive").sort((a, b) => (b.lastSeenAt ?? 0) - (a.lastSeenAt ?? 0))
  if (list.length === 0) return { text: t("Tidak ada user tidak aktif."), sources: [t("Users")] }
  const csv = ["username,router,profile,last_seen", ...list.map((u) => `${u.username},${routerName(c, u.routerId)},${u.profile},${u.lastSeenAt ? new Date(u.lastSeenAt).toISOString() : ""}`)].join("\n")
  return {
    text: t("User tidak aktif: **{n}**. Menampilkan {m} yang terakhir terlihat.", { n: list.length, m: Math.min(8, list.length) }),
    widgets: [
      {
        kind: "list",
        title: t("User tidak aktif"),
        rows: list.slice(0, 8).map((u) => ({
          primary: u.username,
          secondary: `${routerName(c, u.routerId)} · ${u.profile}`,
          trailing: u.lastSeenAt ? formatRelative(u.lastSeenAt, c.now) : "-",
          tone: "warn" as const,
          to: { page: "user", params: { q: u.username } },
        })),
      },
    ],
    actions: [
      { id: "open-inactive", label: t("Buka Users tidak aktif"), to: { page: "user", params: { status: "inactive" } } },
      { id: "copy-csv", label: t("Salin sebagai CSV"), copy: csv },
    ],
    followUps: [t("Siapa yang sering gagal auth?"), t("Berapa user aktif sekarang?")],
    sources: [t("Users")],
    steps: [t("Memahami pertanyaan"), t("Membaca data user"), t("Menghitung hasil")],
  }
}

function latestEvents(c: AiContext): AiAnswer {
  if (!c.can("event")) return noAccess("event")
  const list = c.events.slice(0, 8)
  if (list.length === 0) return { text: t("Belum ada event.") }
  return {
    text: [
      t("Event terbaru:"),
      ...list.map((e) =>
        t("- {when} **{user}** {event} ({router})", {
          when: formatRelative(e.at, c.now),
          user: e.username ?? routerName(c, e.routerId),
          event: eventWord(e.type),
          router: routerName(c, e.routerId),
        })
      ),
    ].join("\n"),
    widgets: [
      {
        kind: "list",
        title: t("Event terbaru"),
        rows: list.map((e) => ({
          primary: e.username ?? routerName(c, e.routerId),
          secondary: `${eventWord(e.type)} · ${routerName(c, e.routerId)}`,
          trailing: formatRelative(e.at, c.now),
          tone: eventTone(e.type),
          to: e.username ? { page: "user", params: { q: e.username } } : { page: "router" },
        })),
      },
    ],
    actions: [{ id: "open-events", label: t("Buka Event"), to: { page: "event" } }],
    followUps: [t("Siapa yang sering gagal auth?"), t("Router mana yang bermasalah?")],
    sources: [t("Event")],
    steps: [t("Memahami pertanyaan"), t("Membaca event 24 jam terakhir")],
  }
}

function findUsers(q: string, c: AiContext): PppUser[] {
  const tokens = (q.match(/[a-z0-9][a-z0-9._-]{2,}/g) ?? []).filter((x) => !STOP.has(x))
  const found: PppUser[] = []
  for (const u of c.users) {
    const name = u.username.toLowerCase()
    if (tokens.some((tok) => name === tok || (tok.length >= 4 && (name.includes(tok) || tok.includes(name))))) found.push(u)
  }
  return found.slice(0, 3)
}

function userDetail(u: PppUser, c: AiContext): AiAnswer {
  const meta = c.metas[u.id]
  const last = c.events.find((e) => e.routerId === u.routerId && e.username === u.username)
  const ip = c.mask ? maskIp(u.address) : (u.address ?? "-")
  const mac = c.mask ? maskMac(u.callerId) : (u.callerId ?? "-")
  const uptime = u.connectedAt ? formatDuration(c.now - u.connectedAt) : "-"
  const seen = u.lastSeenAt ? formatRelative(u.lastSeenAt, c.now) : "-"
  const lines = [
    `**${u.username}** (${routerName(c, u.routerId)})`,
    t("- Status: {status}", { status: statusWord(u) }),
    t("- Profil PPP: {profile}", { profile: u.profile }),
    t("- IP: {ip}", { ip }),
    t("- MAC: {mac}", { mac }),
    t("- Uptime: {uptime}", { uptime }),
    t("- Terakhir terlihat: {seen}", { seen }),
  ]
  const fields = [
    { label: t("Profil PPP"), value: u.profile },
    { label: t("IP"), value: ip },
    { label: t("MAC"), value: mac },
    { label: t("Uptime"), value: uptime },
    { label: t("Terakhir terlihat"), value: seen },
  ]
  if (last) {
    const v = `${eventWord(last.type)} ${formatRelative(last.at, c.now)}`
    lines.push(t("- Event terakhir: {event} {when}", { event: eventWord(last.type), when: formatRelative(last.at, c.now) }))
    fields.push({ label: t("Event terakhir"), value: v })
  }
  if (meta?.customerName) {
    lines.push(t("- Pelanggan: {name}", { name: meta.customerName }))
    fields.push({ label: t("Pelanggan"), value: meta.customerName })
  }
  if (meta?.note) lines.push(t("- Catatan: {note}", { note: meta.note }))
  const tone: AiTone = u.deletedAt || u.status === "disabled" ? "muted" : u.status === "active" ? "good" : last?.type === "auth_failed" ? "bad" : "warn"
  const text = lines.join("\n")
  return {
    text,
    widgets: [{ kind: "user", username: u.username, router: routerName(c, u.routerId), status: statusWord(u), tone, fields, to: { page: "user", params: { q: u.username } } }],
    actions: [
      { id: "open-user", label: t("Buka di Users"), to: { page: "user", params: { q: u.username } } },
      { id: "copy-user", label: t("Salin detail"), copy: plain(text) },
    ],
    followUps: [t("Siapa yang sering gagal auth?"), t("Tampilkan event terbaru")],
    sources: [t("Users"), ...(last ? [t("Event")] : [])],
    steps: [t("Memahami pertanyaan"), t("Mencari user"), t("Membaca data user")],
  }
}

// ---- analitik dan usulan tata letak (ADR 0027) ----

function eventStats(c: AiContext): AiAnswer {
  if (!c.can("event")) return noAccess("event")
  const day = c.events.filter((e) => c.now - e.at < 86400_000)
  const n = (type: PppEventType) => day.filter((e) => e.type === type).length
  const auth = top(c, "auth_failed")
  const drops = top(c, "disconnected")
  const hours = Array.from({ length: 24 }, () => 0)
  for (const e of day) hours[23 - Math.floor((c.now - e.at) / 3600_000)]++
  const bars = hours.map((value, i) => ({ label: `${String(new Date(c.now - (23 - i) * 3600_000).getHours()).padStart(2, "0")}:00`, value }))
  const peak = bars.reduce((m, b) => (b.value > m.value ? b : m), bars[0])
  return {
    text: [
      t("Dalam 24 jam terakhir ada **{n}** event: {a} gagal auth, {d} terputus, {r} router down.", { n: day.length, a: n("auth_failed"), d: n("disconnected"), r: n("router_down") }),
      day.length ? t("Jam tersibuk: **{label}** dengan {value} event.", { label: peak.label, value: peak.value }) : "",
    ]
      .filter(Boolean)
      .join("\n"),
    widgets: [
      {
        kind: "stats",
        items: [
          { label: t("Event"), value: String(day.length) },
          { label: t("Gagal auth"), value: String(n("auth_failed")), tone: n("auth_failed") ? "bad" : "good" },
          { label: t("Terputus"), value: String(n("disconnected")), tone: n("disconnected") ? "warn" : "good" },
          { label: t("Router down"), value: String(n("router_down")), tone: n("router_down") ? "bad" : "good" },
        ],
      },
      { kind: "bars", title: t("Event per jam"), data: bars },
      ...(auth.length ? [{ kind: "list" as const, title: t("Paling sering gagal auth"), rows: auth.map((x) => ({ primary: x.username, secondary: routerName(c, x.routerId), trailing: `${x.n}×`, tone: "bad" as const, to: { page: "event", params: { type: "auth_failed" } } })) }] : []),
      ...(drops.length ? [{ kind: "list" as const, title: t("Paling sering terputus"), rows: drops.map((x) => ({ primary: x.username, secondary: routerName(c, x.routerId), trailing: `${x.n}×`, tone: "warn" as const, to: { page: "event", params: { type: "disconnected" } } })) }] : []),
    ],
    followUps: [t("Bagaimana kesehatan router?"), t("Berapa bandwidth pelanggan sekarang?")],
    sources: [t("Event")],
    steps: [t("Memahami pertanyaan"), t("Menghitung statistik event"), t("Menyusun hasil")],
  }
}

function bandwidth(c: AiContext): AiAnswer {
  if (!c.can("router")) return noAccess("router")
  const s = c.series ?? []
  if (s.length === 0) return { text: t("Belum ada data bandwidth. Nyalakan pengumpulan di Pengaturan > Bandwidth."), sources: [t("Router")] }
  const down = s.map((p) => p.txBps)
  const avg = (a: number[]) => a.reduce((x, y) => x + y, 0) / Math.max(1, a.length)
  const last = s[s.length - 1]
  const sessions = s.filter((p) => (p.sessions ?? 0) > 0)
  const perUser = sessions.length ? avg(sessions.map((p) => p.txBps / (p.sessions as number))) : null
  const peak = s.reduce((m, p) => (p.txBps > m.txBps ? p : m), s[0])
  return {
    text: [
      t("Bandwidth pelanggan (24 jam): unduh sekarang **{now}**, rata-rata **{avg}**, puncak **{peak}** pada {when}.", { now: formatBps(last.txBps), avg: formatBps(avg(down)), peak: formatBps(peak.txBps), when: new Date(peak.at).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }) }),
      perUser !== null ? t("Rata-rata per user aktif: **{v}** unduh.", { v: formatBps(perUser) }) : "",
    ]
      .filter(Boolean)
      .join("\n"),
    widgets: [
      {
        kind: "stats",
        items: [
          { label: t("Unduh sekarang"), value: formatBps(last.txBps) },
          { label: t("Rata-rata"), value: formatBps(avg(down)) },
          { label: t("Puncak"), value: formatBps(peak.txBps) },
          { label: t("Unggah sekarang"), value: formatBps(last.rxBps) },
          ...(last.sessions !== undefined ? [{ label: t("Sesi"), value: String(Math.round(last.sessions)) }] : []),
          ...(perUser !== null ? [{ label: t("Per user (unduh)"), value: formatBps(perUser) }] : []),
        ],
      },
    ],
    followUps: [t("Tambahkan widget bandwidth total ke Ringkasan"), t("Statistik event 24 jam terakhir")],
    sources: [t("Bandwidth (24 jam)")],
    steps: [t("Memahami pertanyaan"), t("Menghitung ringkasan bandwidth"), t("Menyusun hasil")],
  }
}

function routerHealth(c: AiContext): AiAnswer {
  if (!c.can("router")) return noAccess("router")
  const day = c.events.filter((e) => c.now - e.at < 86400_000 && (e.type === "router_down" || e.type === "router_up"))
  const rows = c.routers.map((r) => {
    const downs = c.can("event") ? day.filter((e) => e.routerId === r.id && e.type === "router_down").length : null
    return { name: r.name, status: r.status, downs }
  })
  return {
    text: [t("Kesehatan router (24 jam):"), ...rows.map((r) => `- **${r.name}**: ${r.status}${r.downs === null ? "" : `, ${t("{n}× down", { n: r.downs })}`}`)].join("\n"),
    widgets: [
      {
        kind: "list",
        title: t("Kesehatan router"),
        rows: rows.map((r) => ({ primary: r.name, secondary: r.downs === null ? undefined : t("{n}× down", { n: r.downs }), trailing: r.status, tone: r.status === "online" ? ("good" as const) : ("bad" as const), to: { page: "router" } })),
      },
    ],
    sources: [t("Router")],
    steps: [t("Memahami pertanyaan"), t("Memeriksa kesehatan router"), t("Menyusun hasil")],
  }
}

/** Jenis widget yang disebut di kalimat (kata kunci), untuk usulan tata letak. */
function widgetTypeIn(q: string): string | null {
  const has = (...w: string[]) => w.some((x) => q.includes(x))
  if (has("event per jam", "events per hour")) return "events-per-hour"
  if (has("per user", "per pelanggan")) return "bandwidth-per-user"
  if (has("total bandwidth", "bandwidth total", "total pelanggan", "total customer")) return "bandwidth-total"
  if (has("bandwidth", "grafik", "chart", "trafik")) return "bandwidth-chart"
  if (has("komposisi", "user mix")) return "user-status-mix"
  if (has("profil per", "user per profil", "users per profile")) return "users-per-profile"
  return null
}

export const isLayoutRequest = (q: string) => {
  const has = (...w: string[]) => w.some((x) => q.includes(x))
  return has("tambah", "hapus", "buang", "ubah", "ganti", "add ", "remove", "delete", "change ") && (has("widget", "grafik", "chart") || widgetTypeIn(q) !== null)
}

function layoutProposal(q: string, c: AiContext): AiAnswer {
  if (!c.canEditLayout) return { text: t("Peran Anda tidak boleh mengubah tata letak Ringkasan, jadi saya tidak bisa mengusulkan perubahan widget.") }
  if (!c.ui) return { text: t("Saya belum bisa melihat tampilan Ringkasan. Buka halaman Ringkasan lalu coba lagi.") }
  const kind = widgetTypeIn(q)
  const range = (["1 jam", "6 jam", "24 jam", "7 hari", "30 hari"] as const).map((w, i) => ({ w, r: (["1h", "6h", "24h", "7d", "30d"] as const)[i] })).find((x) => q.includes(x.w) || q.includes(x.r))?.r
  const existing = c.ui.widgets.find((w) => w.type === kind)
  let op: LayoutOp
  if (/hapus|buang|remove|delete/.test(q)) {
    if (!existing) return { text: t("Tidak ada widget itu di Ringkasan, jadi tidak ada yang bisa dihapus."), steps: [t("Membaca tata letak Ringkasan")] }
    op = { op: "remove", id: existing.id }
  } else if (/ubah|ganti|change/.test(q)) {
    if (!existing) return { text: t("Tidak ada widget itu di Ringkasan, jadi tidak ada yang bisa diubah."), steps: [t("Membaca tata letak Ringkasan")] }
    op = { op: "configure", id: existing.id, config: { range: range ?? "24h" } }
  } else {
    op = { op: "add", type: kind ?? "bandwidth-chart", ...(range ? { config: { range } } : {}) }
  }
  return {
    text: t("Saya usulkan satu perubahan tata letak. Belum ada yang berubah: periksa kartu di bawah lalu tekan Terapkan bila setuju."),
    widgets: [{ kind: "proposal", ops: [op] }],
    sources: [t("Tata letak Ringkasan")],
    steps: [t("Membaca tata letak Ringkasan"), t("Menyusun usulan perubahan widget")],
  }
}

export function answerQuestion(question: string, c: AiContext): AiAnswer {
  const q = question.toLowerCase()
  const has = (...w: string[]) => w.some((x) => q.includes(x))

  if (has("bantuan", "help", "bisa apa", "what can you", "bisa apa saja")) return help()
  if (isLayoutRequest(q)) return layoutProposal(q, c)
  if (has("statistik event", "analisis event", "tren event", "event stats", "event statistics", "event per jam", "pola event")) return eventStats(c)
  if (has("kesehatan router", "router health", "downtime", "ketersediaan", "uptime")) return routerHealth(c)
  if (wantsBandwidth(q)) return bandwidth(c)

  if (!has("router bermasalah", "problem router", "router problem")) {
    if (c.can("user")) {
      const users = findUsers(q, c)
      if (users.length > 0) {
        const first = userDetail(users[0], c)
        if (users.length === 1) return first
        const more = users.slice(1).map((u) => userDetail(u, c))
        return { ...first, text: [first.text, ...more.map((m) => m.text)].join("\n\n"), widgets: [...(first.widgets ?? []), ...more.flatMap((m) => m.widgets ?? [])] }
      }
    }
  }
  if (has("router") && has("bermasalah", "masalah", "offline", "down", "problem", "issue", "error", "mati")) return routerProblems(c)
  if (has("gagal auth", "auth fail", "failed auth", "gagal login", "login gagal", "unauthorized", "authentication fail", "password salah", "wrong password"))
    return authFailures(c)
  if (has("putus", "disconnect", "unstable", "tidak stabil", "flap")) return flapping(c)
  if (has("profil", "profile", "paket", "package", "plan")) return profileUsage(c)
  if (has("tidak aktif", "inactive", "nonaktif", "tidak terhubung", "not connected", "daftar user", "list users")) return inactiveUsers(c)
  if (has("event terbaru", "latest event", "recent event", "event terakhir", "aktivitas", "activity", "latest events")) return latestEvents(c)
  if (has("berapa", "how many", "jumlah", "count", "ringkasan", "summary", "overview", "aktif", "active", "status", "total")) return summary(c)

  return { text: [t("Maaf, saya belum paham pertanyaan itu."), "", help().text].join("\n"), followUps: [t("Berapa user aktif sekarang?"), t("Router mana yang bermasalah?")] }
}
